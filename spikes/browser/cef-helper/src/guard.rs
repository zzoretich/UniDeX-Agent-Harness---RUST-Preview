//! A hard stop for a CEF program that re-launches itself in a loop.
//!
//! The classic failure (the precedent crate's issue 9): a sub-process that does not recognise
//! itself as one runs the browser-process path again, starts more sub-processes, and so on until
//! the machine falls over. This helper decides its role from Chromium's `--type=` switch (every
//! sub-process Chromium starts carries one), but the trial adds two independent guards before
//! anything else runs, including Chromium's sandbox:
//!
//! 1. Depth: the browser-process role marks the environment; a second browser-process start
//!    that inherits the mark is a re-entry and exits at once.
//! 2. Rate: every start, in either role, appends a line to a launch log; more than
//!    [`MAX_STARTS`] starts within [`WINDOW`] make the program exit.
//!
//! The log is the file named by `UDX_SPIKE_GUARD_FILE`, or `spike-cef-helper.launches` beside
//! the helper's app bundle (macOS) or executable.

use std::io::Write as _;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ENV_BROWSER: &str = "UDX_SPIKE_CEF_BROWSER_PID";
const ENV_FILE: &str = "UDX_SPIKE_GUARD_FILE";
/// A browser with three tabs starts about ten sub-processes; a loop starts hundreds.
const MAX_STARTS: usize = 40;
const WINDOW: Duration = Duration::from_secs(10);

/// Returns an error when this start looks like part of a re-launch loop.
pub fn check(is_subprocess: bool) -> Result<(), String> {
    if !is_subprocess {
        if let Ok(pid) = std::env::var(ENV_BROWSER) {
            return Err(format!(
                "browser-process start inside the tree of browser process {pid}: re-entry"
            ));
        }
        // SAFETY: first thing in `main`, before any other thread exists.
        unsafe { std::env::set_var(ENV_BROWSER, std::process::id().to_string()) };
    }
    let Some(file) = log_file() else {
        return Ok(());
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let role = if is_subprocess { "sub" } else { "browser" };
    if let Ok(mut log) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
    {
        // One write per line, so concurrent starts do not interleave their lines.
        let line = format!("{} {} {role}\n", now.as_millis(), std::process::id());
        let _ = log.write_all(line.as_bytes());
    }
    let recent = std::fs::read_to_string(&file)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_whitespace().next()?.parse::<u128>().ok())
        .filter(|ms| now.as_millis().saturating_sub(*ms) < WINDOW.as_millis())
        .count();
    if recent > MAX_STARTS {
        return Err(format!(
            "{recent} starts within {} s (limit {MAX_STARTS}), see {}",
            WINDOW.as_secs(),
            file.display()
        ));
    }
    Ok(())
}

fn log_file() -> Option<PathBuf> {
    if let Some(file) = std::env::var_os(ENV_FILE) {
        return Some(file.into());
    }
    let exe = std::env::current_exe().ok()?;
    let beside = exe
        .ancestors()
        .find(|dir| dir.file_name().is_some_and(|n| n == "spike-cef-helper.app"))
        .and_then(|bundle| bundle.parent())
        .or_else(|| exe.parent())?;
    Some(beside.join("spike-cef-helper.launches"))
}
