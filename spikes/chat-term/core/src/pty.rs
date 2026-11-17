//! A terminal session: a PTY with a child process, and `alacritty_terminal`'s emulator fed from it.
//!
//! Two PTY backends, chosen at spawn time:
//!
//! - [`Backend::Alacritty`]: `alacritty_terminal::tty` (openpty on Unix, ConPTY on Windows) driven by
//!   `alacritty_terminal`'s own `EventLoop`, a dedicated thread that polls the PTY, parses into the `Term` and handles
//!   writes, resizes and child exit.
//! - [`Backend::Portable`]: `portable-pty` (openpty on Unix, ConPTY on Windows). It offers blocking `Read`/`Write` and
//!   a blocking `wait`, so this module runs three threads: a reader that parses into the same `Term`, a control thread
//!   that owns the master (writes and resizes), and a waiter for the child's exit.
//!
//! Either way the caller gets the same [`Session`]: the `Term` behind `alacritty_terminal`'s `FairMutex`, and
//! `write`, `resize` and `shutdown`. Terminal events (new content, title, child exit) go to a callback, which the GPUI
//! app turns into a notification on its own thread and `headless-check` simply records.

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::Processor;
use std::borrow::Cow;
use std::io::{self, Read, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Portable,
    Alacritty,
}

impl Backend {
    pub fn name(self) -> &'static str {
        match self {
            Backend::Portable => "portable-pty",
            Backend::Alacritty => "alacritty_terminal::tty",
        }
    }

    pub fn parse(s: &str) -> Option<Backend> {
        match s {
            "portable" | "portable-pty" => Some(Backend::Portable),
            "alacritty" | "alacritty-tty" => Some(Backend::Alacritty),
            _ => None,
        }
    }
}

/// What the session reports to its owner.
#[derive(Clone, Debug)]
pub enum TermEvent {
    /// New content was parsed into the `Term`.
    Wakeup,
    Title(String),
    Bell,
    /// The child exited, with its exit code when known.
    ChildExit(Option<i32>),
    /// The emulator has shut down (after a child exit).
    Exit,
}

pub type EventFn = Arc<dyn Fn(TermEvent) + Send + Sync>;

/// The `Term`'s event listener. It forwards events to the owner and writes the emulator's replies (device status,
/// cursor position and the like) back into the PTY.
#[derive(Clone)]
pub struct Listener {
    on_event: EventFn,
    reply: Arc<OnceLock<Box<dyn Fn(Vec<u8>) + Send + Sync>>>,
    size: Arc<Mutex<WindowSize>>,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            Event::Wakeup => (self.on_event)(TermEvent::Wakeup),
            Event::Title(t) => (self.on_event)(TermEvent::Title(t)),
            Event::Bell => (self.on_event)(TermEvent::Bell),
            Event::ChildExit(status) => (self.on_event)(TermEvent::ChildExit(status.code())),
            Event::Exit => (self.on_event)(TermEvent::Exit),
            Event::PtyWrite(text) => {
                if let Some(reply) = self.reply.get() {
                    reply(text.into_bytes());
                }
            }
            Event::TextAreaSizeRequest(format) => {
                let size = *self.size.lock().unwrap();
                if let Some(reply) = self.reply.get() {
                    reply(format(size).into_bytes());
                }
            }
            _ => {}
        }
    }
}

pub type SharedTerm = Arc<FairMutex<Term<Listener>>>;

/// What to run in the PTY and at what size.
#[derive(Clone, Debug)]
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cols: u16,
    pub rows: u16,
    pub cell_width: u16,
    pub cell_height: u16,
    pub scrollback: usize,
    pub cwd: Option<std::path::PathBuf>,
}

impl SpawnSpec {
    pub fn new(program: impl Into<String>, args: &[&str]) -> Self {
        Self {
            program: program.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            env: vec![
                ("TERM".into(), "xterm-256color".into()),
                ("COLORTERM".into(), "truecolor".into()),
            ],
            cols: 80,
            rows: 24,
            cell_width: 8,
            cell_height: 16,
            scrollback: 10_000,
            cwd: None,
        }
    }

    fn window_size(&self) -> WindowSize {
        WindowSize {
            num_lines: self.rows,
            num_cols: self.cols,
            cell_width: self.cell_width,
            cell_height: self.cell_height,
        }
    }
}

/// The user's default shell: `$SHELL` on Unix (falling back to /bin/sh), `%COMSPEC%` on Windows (falling back to cmd.exe).
pub fn default_shell() -> String {
    if cfg!(windows) {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
    }
}

enum Control {
    Alacritty {
        sender: EventLoopSender,
        thread: Option<JoinHandle<()>>,
    },
    Portable {
        tx: mpsc::Sender<Cmd>,
        threads: Vec<JoinHandle<()>>,
    },
}

enum Cmd {
    Input(Vec<u8>),
    Resize(WindowSize),
    ChildExited,
    Shutdown,
}

pub struct Session {
    pub backend: Backend,
    pub term: SharedTerm,
    size: Arc<Mutex<WindowSize>>,
    control: Control,
    pub spawned_at: Instant,
}

impl Session {
    pub fn spawn(backend: Backend, spec: &SpawnSpec, on_event: EventFn) -> io::Result<Session> {
        let size = Arc::new(Mutex::new(spec.window_size()));
        let listener = Listener {
            on_event: on_event.clone(),
            reply: Arc::new(OnceLock::new()),
            size: size.clone(),
        };
        let config = Config {
            scrolling_history: spec.scrollback,
            ..Config::default()
        };
        let term_size = TermSize::new(spec.cols as usize, spec.rows as usize);
        let term: SharedTerm = Arc::new(FairMutex::new(Term::new(
            config,
            &term_size,
            listener.clone(),
        )));
        let spawned_at = Instant::now();
        let control = match backend {
            Backend::Alacritty => spawn_alacritty(spec, term.clone(), listener)?,
            Backend::Portable => spawn_portable(spec, term.clone(), listener, on_event)?,
        };
        Ok(Session {
            backend,
            term,
            size,
            control,
            spawned_at,
        })
    }

    pub fn write(&self, bytes: impl Into<Vec<u8>>) {
        let bytes = bytes.into();
        match &self.control {
            Control::Alacritty { sender, .. } => {
                let _ = sender.send(Msg::Input(Cow::Owned(bytes)));
            }
            Control::Portable { tx, .. } => {
                let _ = tx.send(Cmd::Input(bytes));
            }
        }
    }

    /// Resize both the PTY (so the child gets SIGWINCH or the ConPTY resize) and the emulator's grid.
    pub fn resize(&self, cols: u16, rows: u16, cell_width: u16, cell_height: u16) {
        let ws = WindowSize {
            num_lines: rows,
            num_cols: cols,
            cell_width,
            cell_height,
        };
        *self.size.lock().unwrap() = ws;
        self.term
            .lock()
            .resize(TermSize::new(cols as usize, rows as usize));
        match &self.control {
            Control::Alacritty { sender, .. } => {
                let _ = sender.send(Msg::Resize(ws));
            }
            Control::Portable { tx, .. } => {
                let _ = tx.send(Cmd::Resize(ws));
            }
        }
    }

    pub fn size(&self) -> WindowSize {
        *self.size.lock().unwrap()
    }

    /// Stop the I/O threads and the child. Waits up to `timeout` for the threads.
    pub fn shutdown(mut self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        let threads: Vec<JoinHandle<()>> = match &mut self.control {
            Control::Alacritty { sender, thread } => {
                let _ = sender.send(Msg::Shutdown);
                thread.take().into_iter().collect()
            }
            Control::Portable { tx, threads } => {
                let _ = tx.send(Cmd::Shutdown);
                std::mem::take(threads)
            }
        };
        for t in threads {
            while !t.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            if t.is_finished() {
                let _ = t.join();
            }
        }
    }
}

fn spawn_alacritty(spec: &SpawnSpec, term: SharedTerm, listener: Listener) -> io::Result<Control> {
    let options = tty::Options {
        shell: Some(tty::Shell::new(spec.program.clone(), spec.args.clone())),
        working_directory: spec.cwd.clone(),
        drain_on_exit: true,
        env: spec.env.iter().cloned().collect(),
        #[cfg(windows)]
        escape_args: true,
    };
    let pty = tty::new(&options, spec.window_size(), 0)?;
    let event_loop = EventLoop::new(term, listener.clone(), pty, true, false)?;
    let sender = event_loop.channel();
    let reply_sender = sender.clone();
    let _ = listener.reply.set(Box::new(move |bytes| {
        let _ = reply_sender.send(Msg::Input(Cow::Owned(bytes)));
    }));
    let handle = event_loop.spawn();
    // Join the event loop's thread through a small adapter so `shutdown` can wait on a `JoinHandle<()>`.
    let thread = std::thread::Builder::new()
        .name("alacritty-loop-join".into())
        .spawn(move || {
            let _ = handle.join();
        })?;
    Ok(Control::Alacritty {
        sender,
        thread: Some(thread),
    })
}

fn spawn_portable(
    spec: &SpawnSpec,
    term: SharedTerm,
    listener: Listener,
    on_event: EventFn,
) -> io::Result<Control> {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: spec.rows,
            cols: spec.cols,
            pixel_width: spec.cell_width * spec.cols,
            pixel_height: spec.cell_height * spec.rows,
        })
        .map_err(|e| io::Error::other(e.to_string()))?;
    let mut cmd = CommandBuilder::new(&spec.program);
    cmd.args(&spec.args);
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    if let Some(dir) = spec.cwd.clone().or_else(|| std::env::current_dir().ok()) {
        cmd.cwd(dir);
    }
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| io::Error::other(e.to_string()))?;
    // The parent must not keep the child's side open, or the reader never sees end of file on Unix.
    drop(pair.slave);
    let master = pair.master;
    let mut reader = master
        .try_clone_reader()
        .map_err(|e| io::Error::other(e.to_string()))?;
    let mut writer = master
        .take_writer()
        .map_err(|e| io::Error::other(e.to_string()))?;
    let mut killer = child.clone_killer();

    let (tx, rx) = mpsc::channel::<Cmd>();
    let reply_tx = tx.clone();
    let _ = listener.reply.set(Box::new(move |bytes| {
        let _ = reply_tx.send(Cmd::Input(bytes));
    }));

    // Reader: blocking reads, parsed into the Term under its lock, then a wakeup.
    let reader_term = term.clone();
    let reader_events = on_event.clone();
    let reader = std::thread::Builder::new()
        .name("portable-pty-reader".into())
        .spawn(move || {
            let mut parser: Processor = Processor::new();
            let mut buf = vec![0u8; 0x10000];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        {
                            let _lease = reader_term.lease();
                            let mut term = reader_term.lock_unfair();
                            parser.advance(&mut *term, &buf[..n]);
                        }
                        reader_events(TermEvent::Wakeup);
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            reader_events(TermEvent::Exit);
        })?;

    // Waiter: the child's exit status. On Windows the reader only sees end of file once the pseudoconsole is closed,
    // so the waiter tells the control thread to drop the master.
    let wait_events = on_event.clone();
    let wait_tx = tx.clone();
    let waiter = std::thread::Builder::new()
        .name("portable-pty-waiter".into())
        .spawn(move || {
            let code = child.wait().ok().map(|s| s.exit_code() as i32);
            wait_events(TermEvent::ChildExit(code));
            let _ = wait_tx.send(Cmd::ChildExited);
        })?;

    // Control: owns the master and the writer.
    let control = std::thread::Builder::new()
        .name("portable-pty-control".into())
        .spawn(move || {
            let mut master = Some(master);
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    Cmd::Input(bytes) => {
                        let _ = writer.write_all(&bytes);
                        let _ = writer.flush();
                    }
                    Cmd::Resize(ws) => {
                        if let Some(m) = master.as_ref() {
                            let _ = m.resize(PtySize {
                                rows: ws.num_lines,
                                cols: ws.num_cols,
                                pixel_width: ws.cell_width * ws.num_cols,
                                pixel_height: ws.cell_height * ws.num_lines,
                            });
                        }
                    }
                    Cmd::ChildExited => {
                        // Give the reader a moment to drain what the child wrote last, then close the PTY.
                        std::thread::sleep(Duration::from_millis(100));
                        master.take();
                    }
                    Cmd::Shutdown => {
                        let _ = killer.kill();
                        master.take();
                        break;
                    }
                }
            }
        })?;
    Ok(Control::Portable {
        tx,
        threads: vec![control, reader, waiter],
    })
}

fn line_text<T>(term: &Term<T>, line: i32) -> String {
    use alacritty_terminal::term::cell::Flags;
    let row = &term.grid()[Line(line)];
    let mut s = String::new();
    for c in 0..term.columns() {
        let cell = &row[Column(c)];
        if cell
            .flags
            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }
        s.push(cell.c);
        if let Some(zw) = cell.zerowidth() {
            s.extend(zw.iter());
        }
    }
    s.trim_end().to_string()
}

/// Every line of the grid (scrollback and screen) as text, oldest first, trailing spaces trimmed.
pub fn grid_lines<T>(term: &Term<T>) -> Vec<String> {
    let grid = term.grid();
    (grid.topmost_line().0..=grid.bottommost_line().0)
        .map(|l| line_text(term, l))
        .collect()
}

/// The visible screen lines only, top to bottom (ignores any scrollback display offset).
pub fn screen_lines<T>(term: &Term<T>) -> Vec<String> {
    (0..term.screen_lines() as i32)
        .map(|l| line_text(term, l))
        .collect()
}
