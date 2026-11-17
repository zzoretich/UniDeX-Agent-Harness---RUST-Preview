//! A headless check of the helper protocol, for CI runners without a GPU or a display.
//!
//! ```sh
//! spike-smoke [--frames N] [--timeout-s S] [--hold-s S] [--expect-marker] [--expect-sandbox]
//!             -- <helper program> [helper args...]
//! ```
//!
//! `--hold-s` keeps the helper running that long after the checks, so a CI step can inspect its
//! sub-processes. Before shutting the helper down, the smoke check prints its process tree with
//! each process's sandbox state as the OS reports it (macOS: `sandbox_check`; Linux: the seccomp
//! mode, filters and namespaces; Windows: not read). `--expect-sandbox` fails the check unless
//! at least one renderer exists and every process the sandbox rule requires reports itself
//! sandboxed (macOS: every process but CEF's browser process; Linux: the same, less the
//! launchers and Chromium's own unsandboxed processes, see `tree::ProcSample::exemption`). The
//! helper must also exit by itself within 3 s of `Shutdown` with status 0.
//!
//! Starts the helper the way the app does (piped stdin and stdout, inherited shared memory),
//! asks for an 800 x 600 view, makes the two DevTools calls (navigate, evaluate), waits for
//! frames and both answers, checks the frames, and shuts the helper down. `--expect-marker`
//! checks the stand-in's green corner marker, which proves the pixels crossed intact. Exits 1 if
//! anything is missing.

use spike_protocol::helper::{HelperBrowser, HelperOptions};
use spike_protocol::view::{BrowserEvent, BrowserView as _, FrameKind};
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().collect();
    let split = raw.iter().position(|a| a == "--").unwrap_or(raw.len());
    let (own, rest) = raw.split_at(split);
    let value = |name: &str| {
        own.iter()
            .position(|a| a == name)
            .and_then(|i| own.get(i + 1))
            .and_then(|v| v.parse::<u64>().ok())
    };
    let want_frames = value("--frames").unwrap_or(30);
    let timeout = Duration::from_secs(value("--timeout-s").unwrap_or(30));
    let expect_marker = own.iter().any(|a| a == "--expect-marker");
    let hold = Duration::from_secs(value("--hold-s").unwrap_or(0));
    let expect_sandbox = own.iter().any(|a| a == "--expect-sandbox");
    let Some(program) = rest.get(1) else {
        eprintln!("smoke: usage: spike-smoke [options] -- <helper program> [helper args...]");
        return ExitCode::from(2);
    };

    let (tx, rx) = mpsc::channel();
    let options = HelperOptions {
        program: program.into(),
        args: rest[2..].to_vec(),
        max_width: 1920,
        max_height: 1200,
        slots: 3,
    };
    let mut browser = match HelperBrowser::spawn(options, move |event| {
        let _ = tx.send(event);
    }) {
        Ok(browser) => browser,
        Err(error) => {
            eprintln!("smoke: could not start the helper: {error:#}");
            return ExitCode::FAILURE;
        }
    };

    let started = Instant::now();
    browser.resize(800, 600, 1.0);
    let navigate = browser.navigate("data:text/html,<title>smoke</title><h1>smoke test</h1>");
    let evaluate = browser.evaluate("document.title");

    let mut engine = None;
    let mut frames = 0u64;
    let mut first_frame = None;
    let mut sizes_ok = true;
    let mut nonblank = false;
    let mut marker_ok = false;
    let mut replies = Vec::new();
    let mut exited = None;
    while started.elapsed() < timeout {
        let done = engine.is_some()
            && frames >= want_frames
            && replies.iter().any(|(id, _)| *id == evaluate);
        if done {
            break;
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(BrowserEvent::Ready { engine: name }) => engine = Some(name),
            Ok(BrowserEvent::Frame(frame)) if frame.kind == FrameKind::View => {
                frames += 1;
                first_frame.get_or_insert(started.elapsed());
                sizes_ok &= frame.width == 800 && frame.height == 600;
                nonblank |= frame.bgra.iter().any(|b| *b != 0);
                // The stand-in paints pure green (BGRA 00 FF 00 FF) in its top-left corner.
                let at = (4 * frame.width as usize + 4) * 4;
                marker_ok |= frame.bgra.get(at..at + 4) == Some(&[0x00, 0xFF, 0x00, 0xFF][..]);
            }
            Ok(BrowserEvent::DevTools(message)) => {
                let id = message["id"].as_u64().unwrap_or(0);
                eprintln!("smoke: devtools {message}");
                replies.push((id, message));
            }
            Ok(BrowserEvent::Exited { status }) => {
                exited = Some(status);
                break;
            }
            Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let elapsed = started.elapsed();
    if !hold.is_zero() {
        eprintln!("smoke: holding the helper for {} s", hold.as_secs());
        std::thread::sleep(hold);
    }
    let tree = spike_protocol::tree::ProcessTree::default().sample(browser.pid() as i32);
    for p in &tree {
        let rule = match p.exemption() {
            Some(reason) => format!("not required: {reason}"),
            None => "required".to_string(),
        };
        let sandbox_type = p
            .sandbox_type
            .as_deref()
            .map(|t| format!(" sandbox-type={t}"))
            .unwrap_or_default();
        eprintln!(
            "smoke: process {} {}{sandbox_type} sandboxed={:?} [{rule}] ({})",
            p.pid, p.kind, p.sandboxed, p.sandbox_detail
        );
    }
    let sandbox_ok = if tree.is_empty() {
        eprintln!("smoke: sandbox: not inspected (the process tree is not read on this platform)");
        !expect_sandbox
    } else {
        let required: Vec<_> = tree.iter().filter(|p| p.exemption().is_none()).collect();
        let sandboxed = required
            .iter()
            .filter(|p| p.sandboxed == Some(true))
            .count();
        let renderers = tree.iter().filter(|p| p.kind == "renderer").count();
        let ok = renderers > 0 && sandboxed == required.len();
        eprintln!(
            "smoke: sandbox: {} processes, {} required to be sandboxed, {sandboxed} of them \
             sandboxed, {renderers} renderers, {} not required; all required sandboxed: {ok}",
            tree.len(),
            required.len(),
            tree.len() - required.len()
        );
        ok
    };
    browser.shutdown();
    let (clean_exit, exit) = browser
        .exit()
        .map(|(ok, how)| (ok, how.to_string()))
        .unwrap_or((false, "unknown".into()));
    eprintln!("smoke: helper exit: {exit} (clean: {clean_exit})");

    let navigated = replies
        .iter()
        .any(|(id, m)| *id == navigate && m.get("result").is_some());
    let evaluated = replies
        .iter()
        .find(|(id, _)| *id == evaluate)
        .map(|(_, m)| m["result"]["result"]["value"].to_string());
    eprintln!(
        "smoke: engine={engine:?} frames={frames} in {:.1} s, first frame after {:?}, sizes_ok={sizes_ok} \
         nonblank={nonblank} marker_ok={marker_ok} navigate_ok={navigated} evaluate={evaluated:?} exited_early={exited:?}",
        elapsed.as_secs_f64(),
        first_frame,
    );
    let ok = engine.is_some()
        && frames >= want_frames
        && sizes_ok
        && nonblank
        && navigated
        && evaluated.is_some()
        && exited.is_none()
        && clean_exit
        && (!expect_sandbox || sandbox_ok)
        && (!expect_marker || marker_ok);
    if ok {
        eprintln!("smoke: ok");
        ExitCode::SUCCESS
    } else {
        eprintln!("smoke: FAILED");
        ExitCode::FAILURE
    }
}
