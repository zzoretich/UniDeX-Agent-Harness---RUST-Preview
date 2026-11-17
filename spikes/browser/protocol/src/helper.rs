//! `HelperBrowser`: a [`BrowserView`] whose engine runs in a helper process.
//!
//! The helper is started with piped stdin and stdout (the control channel) and an inherited
//! shared-memory region (the frames). Two threads per view keep the UI thread off the pipes:
//! a writer that sends [`ToHelper`] messages, and a reader that parses [`FromHelper`] messages,
//! copies frames out of shared memory, releases their slots and passes [`BrowserEvent`]s to the
//! sink. The sink runs on the reader thread.

use crate::shm::SharedFrames;
use crate::view::{BrowserEvent, BrowserView, Frame, InputEvent};
use crate::wire::{self, FromHelper, ToHelper};
use anyhow::{Context as _, Result};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct HelperOptions {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// The largest frame, in device pixels, the shared memory is sized for.
    pub max_width: u32,
    pub max_height: u32,
    pub slots: u32,
}

pub struct HelperBrowser {
    child: Option<Child>,
    to_helper: Option<mpsc::Sender<ToHelper>>,
    writer: Option<JoinHandle<()>>,
    reader: Option<JoinHandle<()>>,
    next_id: u64,
    pid: u32,
    exit: Option<(bool, String)>,
}

impl HelperBrowser {
    pub fn spawn(
        options: HelperOptions,
        mut sink: impl FnMut(BrowserEvent) + Send + 'static,
    ) -> Result<Self> {
        let shm = Arc::new(SharedFrames::create(
            options.max_width,
            options.max_height,
            options.slots,
        )?);
        let mut command = Command::new(&options.program);
        command
            .args(&options.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        shm.share_with(&mut command);
        let mut child = command
            .spawn()
            .with_context(|| format!("starting {}", options.program.display()))?;
        let pid = child.id();
        let stdin = child.stdin.take().context("helper stdin")?;
        let stdout = child.stdout.take().context("helper stdout")?;

        let (to_helper, outbox) = mpsc::channel::<ToHelper>();
        let writer = std::thread::Builder::new()
            .name("helper-writer".into())
            .spawn(move || {
                let mut stdin = BufWriter::new(stdin);
                while let Ok(message) = outbox.recv() {
                    if wire::write_line(&mut stdin, &message).is_err() {
                        break;
                    }
                }
            })?;

        let releases = to_helper.clone();
        let reader = std::thread::Builder::new()
            .name("helper-reader".into())
            .spawn(move || {
                let mut stdout = BufReader::with_capacity(1 << 16, stdout);
                let mut buffer = Vec::new();
                let status = loop {
                    let message = match wire::read_line::<FromHelper>(&mut stdout, &mut buffer) {
                        Ok(Some(message)) => message,
                        Ok(None) => break "helper closed its output".to_string(),
                        Err(error) => break format!("bad message from helper: {error}"),
                    };
                    match message {
                        FromHelper::Ready { engine } => sink(BrowserEvent::Ready { engine }),
                        FromHelper::Frame {
                            slot,
                            kind,
                            seq,
                            width,
                            height,
                            dirty,
                            produced_unix_ns,
                        } => {
                            let started = Instant::now();
                            let copied = shm.copy_out(slot, width, height);
                            let copy_ns = started.elapsed().as_nanos() as u64;
                            let _ = releases.send(ToHelper::Release { slot });
                            match copied {
                                Ok(bgra) => sink(BrowserEvent::Frame(Frame {
                                    kind,
                                    seq,
                                    width,
                                    height,
                                    bgra,
                                    dirty,
                                    produced_unix_ns,
                                    copy_ns,
                                })),
                                Err(error) => break format!("bad frame from helper: {error:#}"),
                            }
                        }
                        FromHelper::Popup { show, rect } => {
                            sink(BrowserEvent::Popup { show, rect })
                        }
                        FromHelper::Cursor { cursor } => sink(BrowserEvent::Cursor(cursor)),
                        FromHelper::ImeBounds {
                            range_start,
                            range_end,
                            characters,
                        } => sink(BrowserEvent::ImeCompositionBounds {
                            range_start,
                            range_end,
                            characters,
                        }),
                        FromHelper::DevTools { message } => sink(BrowserEvent::DevTools(message)),
                        FromHelper::Status { text } => sink(BrowserEvent::Status(text)),
                    }
                };
                sink(BrowserEvent::Exited { status });
            })?;

        Ok(Self {
            child: Some(child),
            to_helper: Some(to_helper),
            writer: Some(writer),
            reader: Some(reader),
            next_id: 1,
            pid,
            exit: None,
        })
    }

    /// The helper's process id, for measurements.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// After [`BrowserView::shutdown`]: whether the helper ended by itself with success, and how
    /// it ended (its exit status, or "killed after 3 s" when it did not stop by itself).
    pub fn exit(&self) -> Option<(bool, &str)> {
        self.exit.as_ref().map(|(ok, how)| (*ok, how.as_str()))
    }

    fn send(&self, message: ToHelper) {
        if let Some(to_helper) = &self.to_helper {
            let _ = to_helper.send(message);
        }
    }
}

impl BrowserView for HelperBrowser {
    fn resize(&mut self, width: u32, height: u32, scale_factor: f32) {
        self.send(ToHelper::Resize {
            width,
            height,
            scale_factor,
        });
    }

    fn input(&mut self, event: InputEvent) {
        self.send(ToHelper::Input { event });
    }

    fn devtools(&mut self, method: &str, params: serde_json::Value) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.send(ToHelper::DevTools {
            id,
            method: method.to_string(),
            params,
        });
        id
    }

    fn shutdown(&mut self) {
        self.send(ToHelper::Shutdown);
        // The reader holds the other sender (for slot releases) until the helper's output
        // closes, so wait for the helper first, then join both threads.
        self.to_helper = None;
        if let Some(mut child) = self.child.take() {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        self.exit = Some((status.success(), status.to_string()));
                        break;
                    }
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    _ => {
                        eprintln!("app: helper did not stop in time, killing it");
                        let _ = child.kill();
                        let _ = child.wait();
                        self.exit = Some((false, "killed after 3 s".into()));
                        break;
                    }
                }
            }
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

impl Drop for HelperBrowser {
    fn drop(&mut self) {
        self.shutdown();
    }
}
