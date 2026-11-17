//! `headless-check`: everything in the trial that runs without a display. This is the part CI can really exercise on
//! macOS, Linux and Windows.
//!
//! ```sh
//! cargo run --release -p chat-term-core --bin headless-check -- [--out results.json] [--backend portable|alacritty]
//! ```
//!
//! Checks, in order: incremental markdown against full parses after every token; the key encoding table; the emulator
//! (`alacritty_terminal::Term`) fed escape sequences directly (24-bit and 256 colours, attributes, wide characters,
//! combining marks, alternate screen, bracketed paste mode, a 10,000-line scrollback, a 200,000-line burst); then, for
//! each PTY backend, a child process in a real PTY: this binary's own `child` helper (echo, size, resize, raw keystroke
//! round trip, a 200,000-line burst, exit code), the default system shell (a command, a resize, exit code) and, on Unix,
//! `vi` as a full-screen program on the alternate screen.
//!
//! A failed "hard" check makes the exit code 1. "Soft" checks are reported but do not fail the run: they are the ones
//! whose behaviour on a platform is the finding (for example the raw keystroke round trip through ConPTY).

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, Processor, Rgb};
use chat_term_core::content;
use chat_term_core::keys;
use chat_term_core::md::{self, StreamDoc};
use chat_term_core::pty::{self, Backend, Listener, Session, SpawnSpec, TermEvent};
use chat_term_core::stats::{self, round3};
use serde_json::{Value, json};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

struct Entry {
    group: String,
    name: String,
    ok: bool,
    hard: bool,
    detail: String,
}

#[derive(Default)]
struct Report {
    entries: Vec<Entry>,
    data: serde_json::Map<String, Value>,
}

impl Report {
    fn check(&mut self, group: &str, name: &str, ok: bool, hard: bool, detail: impl Into<String>) {
        let detail = detail.into();
        let tag = match (ok, hard) {
            (true, _) => "PASS",
            (false, true) => "FAIL",
            (false, false) => "WARN",
        };
        println!(
            "[{tag}] {group}: {name}{}",
            if detail.is_empty() {
                String::new()
            } else {
                format!(" ({detail})")
            }
        );
        self.entries.push(Entry {
            group: group.into(),
            name: name.into(),
            ok,
            hard,
            detail,
        });
    }

    fn data(&mut self, key: &str, value: Value) {
        println!("  data {key} = {value}");
        self.data.insert(key.into(), value);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("child") {
        child::run();
        return;
    }
    let out = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let only_backend = args
        .iter()
        .position(|a| a == "--backend")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| Backend::parse(s));
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!(
        "headless-check: os={} arch={} profile={profile}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    let mut report = Report::default();
    check_markdown(&mut report);
    check_keys(&mut report);
    check_emulator(&mut report);
    for backend in [Backend::Portable, Backend::Alacritty] {
        if only_backend.is_some_and(|b| b != backend) {
            continue;
        }
        check_pty_child(backend, &mut report);
        check_pty_shell(backend, &mut report);
        #[cfg(unix)]
        check_pty_vi(backend, &mut report);
    }

    let hard_failures = report.entries.iter().filter(|e| !e.ok && e.hard).count();
    let soft_failures = report.entries.iter().filter(|e| !e.ok && !e.hard).count();
    let passed = report.entries.iter().filter(|e| e.ok).count();
    println!("headless-check: {passed} passed, {hard_failures} failed, {soft_failures} warnings");
    if let Some(path) = out {
        let entries: Vec<Value> = report
            .entries
            .iter()
            .map(|e| json!({"group": e.group, "name": e.name, "ok": e.ok, "hard": e.hard, "detail": e.detail}))
            .collect();
        let doc = json!({
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "profile": profile,
            "passed": passed,
            "failed": hard_failures,
            "warnings": soft_failures,
            "checks": entries,
            "data": report.data,
        });
        std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).expect("write --out");
    }
    std::process::exit(if hard_failures > 0 { 1 } else { 0 });
}

// ---------------------------------------------------------------------------------------------------------------------
// Markdown

fn check_markdown(report: &mut Report) {
    let t = Instant::now();
    let thread = content::thread(42, 5000);
    let gen_ms = t.elapsed().as_secs_f64() * 1000.0;
    let bytes: usize = thread.iter().map(|m| m.text.len()).sum();
    let t = Instant::now();
    let parsed: Vec<Vec<md::Block>> = thread.iter().map(|m| md::parse(&m.text)).collect();
    let parse_ms = t.elapsed().as_secs_f64() * 1000.0;
    let blocks: usize = parsed.iter().map(Vec::len).sum();
    let code_blocks: Vec<usize> = parsed
        .iter()
        .flatten()
        .filter_map(|b| {
            if let md::Block::Code { text, .. } = b {
                Some(text.lines().count())
            } else {
                None
            }
        })
        .collect();
    let tables = parsed
        .iter()
        .flatten()
        .filter(|b| matches!(b, md::Block::Table(_)))
        .count();
    report.data(
        "thread",
        json!({
            "messages": thread.len(), "bytes": bytes, "blocks": blocks, "code_blocks": code_blocks.len(),
            "code_lines_min": code_blocks.iter().min(), "code_lines_max": code_blocks.iter().max(), "tables": tables,
            "generate_ms": round3(gen_ms), "parse_all_ms": round3(parse_ms),
        }),
    );
    report.check(
        "markdown",
        "thread has fenced code of 5 to 80 lines and tables",
        code_blocks.iter().any(|n| *n >= 60) && tables > 0,
        true,
        format!("{} code blocks, {} tables", code_blocks.len(), tables),
    );

    // Stream a long reply token by token; after every token compare the incremental parse with a full one.
    let tokens = content::stream_tokens(42, 40_000);
    let mut doc = StreamDoc::new();
    let mut inc = Vec::with_capacity(tokens.len());
    let mut full = Vec::with_capacity(tokens.len());
    let mut mismatches = 0usize;
    let mut first_mismatch = None;
    for (i, tok) in tokens.iter().enumerate() {
        doc.append(tok);
        inc.push(doc.last_parse_ns as f64 / 1e6);
        let t = Instant::now();
        let whole = md::parse(doc.source());
        full.push(t.elapsed().as_secs_f64() * 1000.0);
        let same =
            whole.len() == doc.block_count() && whole.iter().zip(doc.blocks()).all(|(a, b)| a == b);
        if !same {
            mismatches += 1;
            first_mismatch.get_or_insert(i);
        }
    }
    report.data(
        "stream_parse",
        json!({
            "tokens": tokens.len(), "final_bytes": doc.source().len(), "final_blocks": doc.block_count(),
            "incremental_per_token": stats::summary_ms(&inc), "full_reparse_per_token": stats::summary_ms(&full),
            "full_reparse_last_100": stats::summary_ms(&full[full.len().saturating_sub(100)..]),
        }),
    );
    report.check(
        "markdown",
        "incremental parse equals a full parse after every token",
        mismatches == 0,
        true,
        format!(
            "{mismatches} mismatches of {}, first at {:?}",
            tokens.len(),
            first_mismatch
        ),
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Keys

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn check_keys(report: &mut Report) {
    let table = keys::table();
    let mut wrong = Vec::new();
    for (key, app_cursor, expected) in &table {
        let got = keys::encode(key, *app_cursor).unwrap_or_default();
        if got != *expected {
            wrong.push(format!(
                "{:?} got {} want {}",
                key.key,
                hex(&got),
                hex(expected)
            ));
        }
    }
    report.check(
        "keys",
        "encoding table",
        wrong.is_empty(),
        true,
        format!("{} keys; {}", table.len(), wrong.join("; ")),
    );
    let bp = keys::paste("a\nb\x1b[201~c", true);
    report.check(
        "keys",
        "bracketed paste wraps and strips the end marker",
        bp == b"\x1b[200~a\nbc\x1b[201~",
        true,
        hex(&bp),
    );
    let plain = keys::paste("a\nb", false);
    report.check(
        "keys",
        "plain paste turns newlines into CR",
        plain == b"a\rb",
        true,
        hex(&plain),
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Emulator without a PTY

fn feed(term: &mut Term<VoidListener>, parser: &mut Processor, bytes: &[u8]) {
    parser.advance(term, bytes);
}

fn check_emulator(report: &mut Report) {
    let config = Config {
        scrolling_history: 10_000,
        ..Config::default()
    };
    let mut term = Term::new(config.clone(), &TermSize::new(80, 24), VoidListener);
    let mut p: Processor = Processor::new();
    feed(&mut term, &mut p, b"\x1b[38;2;255;100;0mX\x1b[0m\x1b[48;5;196mY\x1b[0m\x1b[1mB\x1b[0m\x1b[3mI\x1b[0m\x1b[4mU\x1b[0m");
    let row = &term.grid()[Line(0)];
    report.check(
        "emulator",
        "24-bit colour",
        row[Column(0)].fg
            == Color::Spec(Rgb {
                r: 255,
                g: 100,
                b: 0,
            }),
        true,
        format!("{:?}", row[Column(0)].fg),
    );
    report.check(
        "emulator",
        "256-colour background",
        row[Column(1)].bg == Color::Indexed(196),
        true,
        format!("{:?}", row[Column(1)].bg),
    );
    report.check(
        "emulator",
        "bold, italic, underline",
        row[Column(2)].flags.contains(Flags::BOLD)
            && row[Column(3)].flags.contains(Flags::ITALIC)
            && row[Column(4)].flags.contains(Flags::UNDERLINE),
        true,
        "",
    );

    feed(
        &mut term,
        &mut p,
        "\r\n\u{4F60}e\u{301}\u{1F468}\u{200D}\u{1F4BB}".as_bytes(),
    );
    let row = &term.grid()[Line(1)];
    report.check(
        "emulator",
        "wide character takes two cells",
        row[Column(0)].flags.contains(Flags::WIDE_CHAR)
            && row[Column(1)].flags.contains(Flags::WIDE_CHAR_SPACER),
        true,
        "",
    );
    let combining = row[Column(2)].c == 'e'
        && row[Column(2)]
            .zerowidth()
            .is_some_and(|z| z.contains(&'\u{301}'));
    report.check(
        "emulator",
        "combining mark stays on its base cell",
        combining,
        true,
        "",
    );
    let emoji_cells: Vec<String> = (3..8)
        .map(|c| {
            format!(
                "{:?}{}",
                row[Column(c)].c,
                if row[Column(c)].flags.contains(Flags::WIDE_CHAR) {
                    "W"
                } else {
                    ""
                }
            )
        })
        .collect();
    report.data("emulator_zwj_emoji_cells", json!(emoji_cells.join(" ")));

    feed(&mut term, &mut p, b"\x1b[?1049h");
    let alt_on = term.mode().contains(TermMode::ALT_SCREEN);
    feed(&mut term, &mut p, b"\x1b[?1049l");
    report.check(
        "emulator",
        "alternate screen on and off",
        alt_on && !term.mode().contains(TermMode::ALT_SCREEN),
        true,
        "",
    );
    feed(&mut term, &mut p, b"\x1b[?2004h\x1b[?1h");
    report.check(
        "emulator",
        "bracketed paste and application cursor modes",
        term.mode().contains(TermMode::BRACKETED_PASTE)
            && term.mode().contains(TermMode::APP_CURSOR),
        true,
        "",
    );

    let mut term = Term::new(config.clone(), &TermSize::new(80, 24), VoidListener);
    let mut lines = Vec::new();
    for i in 0..20_000 {
        lines.extend_from_slice(format!("line {i}\r\n").as_bytes());
    }
    feed(&mut term, &mut p, &lines);
    report.check(
        "emulator",
        "scrollback is capped at 10,000 lines",
        term.grid().history_size() == 10_000,
        true,
        format!("history {}", term.grid().history_size()),
    );
    let top = pty::grid_lines(&term).first().cloned().unwrap_or_default();
    report.check(
        "emulator",
        "oldest kept line is the expected one",
        top == "line 9977",
        true,
        format!("top line {top:?}"),
    );

    let mut term = Term::new(config, &TermSize::new(120, 40), VoidListener);
    let mut burst = Vec::new();
    for i in 1..=200_000 {
        burst.extend_from_slice(format!("{i}\r\n").as_bytes());
    }
    let t = Instant::now();
    for chunk in burst.chunks(0x10000) {
        feed(&mut term, &mut p, chunk);
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    report.data("emulator_burst_200k_lines", json!({"bytes": burst.len(), "ms": round3(ms), "mb_per_s": round3(burst.len() as f64 / 1e6 / (ms / 1000.0))}));
}

// ---------------------------------------------------------------------------------------------------------------------
// PTY

#[derive(Default)]
struct RecState {
    events: u64,
    wakeups: u64,
    child_exit: Option<Option<i32>>,
    exit: bool,
}

#[derive(Default)]
struct Recorder {
    state: Mutex<RecState>,
    cv: Condvar,
}

impl Recorder {
    fn callback(self: &Arc<Self>) -> pty::EventFn {
        let me = self.clone();
        Arc::new(move |ev: TermEvent| {
            let mut s = me.state.lock().unwrap();
            s.events += 1;
            match ev {
                TermEvent::Wakeup => s.wakeups += 1,
                TermEvent::ChildExit(code) => s.child_exit = Some(code),
                TermEvent::Exit => s.exit = true,
                _ => {}
            }
            me.cv.notify_all();
        })
    }
}

/// Wait until `pred` holds for the terminal, re-checking on every wakeup (and at least every 50 ms).
fn wait_term(
    session: &Session,
    rec: &Recorder,
    timeout: Duration,
    mut pred: impl FnMut(&Term<Listener>) -> bool,
) -> Option<Duration> {
    let start = Instant::now();
    loop {
        // Read the event counter before looking at the terminal, so an event that lands in between is not missed.
        let seen = rec.state.lock().unwrap().events;
        if pred(&session.term.lock()) {
            return Some(start.elapsed());
        }
        let mut guard = rec.state.lock().unwrap();
        while guard.events == seen {
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return None;
            }
            guard = rec
                .cv
                .wait_timeout(guard, (timeout - elapsed).min(Duration::from_millis(250)))
                .unwrap()
                .0;
        }
    }
}

fn wait_rec(
    rec: &Recorder,
    timeout: Duration,
    mut pred: impl FnMut(&RecState) -> bool,
) -> Option<Duration> {
    let start = Instant::now();
    let mut guard = rec.state.lock().unwrap();
    loop {
        if pred(&guard) {
            return Some(start.elapsed());
        }
        let elapsed = start.elapsed();
        if elapsed >= timeout {
            return None;
        }
        guard = rec.cv.wait_timeout(guard, timeout - elapsed).unwrap().0;
    }
}

fn has_line(term: &Term<Listener>, want: &str) -> bool {
    pty::screen_lines(term).iter().any(|l| l.trim() == want)
}

fn ms(d: Option<Duration>) -> String {
    d.map(|d| format!("{:.3} ms", d.as_secs_f64() * 1000.0))
        .unwrap_or_else(|| "timed out".into())
}

fn screen_dump(session: &Session) -> String {
    let t = session.term.lock();
    format!(
        "mode {:?}; screen {:?}",
        t.mode(),
        pty::screen_lines(&t)
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
    )
}

fn check_pty_child(backend: Backend, report: &mut Report) {
    let group = format!("pty {} / child helper", backend.name());
    let exe = std::env::current_exe()
        .expect("current_exe")
        .to_string_lossy()
        .into_owned();
    let rec = Arc::new(Recorder::default());
    let spec = SpawnSpec::new(exe, &["child"]);
    let session = match Session::spawn(backend, &spec, rec.callback()) {
        Ok(s) => s,
        Err(e) => {
            report.check(&group, "spawn", false, true, e.to_string());
            return;
        }
    };
    let ready = wait_term(&session, &rec, Duration::from_secs(15), |t| {
        has_line(t, "READY")
    });
    report.check(
        &group,
        "spawn and first output",
        ready.is_some(),
        true,
        ms(ready),
    );
    if ready.is_none() {
        session.shutdown(Duration::from_secs(2));
        return;
    }

    session.write(b"echo hello-pty\r".to_vec());
    let d = wait_term(&session, &rec, Duration::from_secs(5), |t| {
        has_line(t, "ECHO hello-pty")
    });
    report.check(
        &group,
        "write a command, read its output",
        d.is_some(),
        true,
        ms(d),
    );

    session.write(b"size\r".to_vec());
    let d = wait_term(&session, &rec, Duration::from_secs(5), |t| {
        has_line(t, "SIZE 80x24")
    });
    report.check(&group, "child sees 80x24", d.is_some(), true, ms(d));

    session.resize(100, 30, 8, 16);
    // Clear the screen first so an old SIZE line cannot match.
    session.write(b"size\r".to_vec());
    let d = wait_term(&session, &rec, Duration::from_secs(5), |t| {
        has_line(t, "SIZE 100x30")
    });
    let grid_ok = {
        let t = session.term.lock();
        t.columns() == 100 && t.screen_lines() == 30
    };
    report.check(
        &group,
        "resize: child sees 100x30 and the grid agrees",
        d.is_some() && grid_ok,
        true,
        format!("{}, grid ok {grid_ok}", ms(d)),
    );

    // Raw keystroke round trip: the child reads in raw mode and prints the hex of what it received.
    let table = keys::table();
    session.write(format!("keys {}\r", table.len()).into_bytes());
    let raw = wait_term(&session, &rec, Duration::from_secs(5), |t| {
        has_line(t, "RAW")
    });
    let mut wrong = Vec::new();
    let mut lat = Vec::new();
    if raw.is_some() {
        for (i, (key, app_cursor, expected)) in table.iter().enumerate() {
            let bytes = keys::encode(key, *app_cursor).unwrap_or_default();
            let prefix = format!("K{i} ");
            let t0 = Instant::now();
            session.write(bytes);
            let mut got = None;
            let d = wait_term(&session, &rec, Duration::from_secs(3), |t| {
                got = pty::screen_lines(t)
                    .iter()
                    .find_map(|l| l.strip_prefix(&prefix).map(|h| h.trim().to_string()));
                got.is_some()
            });
            if d.is_some() {
                lat.push(t0.elapsed().as_secs_f64() * 1000.0);
            }
            let want = hex(expected);
            if got.as_deref() != Some(want.as_str()) {
                wrong.push(format!("{} got {:?} want {want}", key.key, got));
            }
        }
    }
    let raw_ok = raw.is_some() && wrong.is_empty();
    report.check(
        &group,
        "keystrokes arrive byte for byte in raw mode",
        raw_ok,
        cfg!(unix),
        format!("{} keys; {}", table.len(), wrong.join("; ")),
    );
    report.data(
        &format!("{}_keystroke_roundtrip", backend.name()),
        stats::summary_ms(&lat),
    );
    let cooked = wait_term(&session, &rec, Duration::from_secs(3), |t| {
        has_line(t, "COOKED")
    });
    if cooked.is_none() {
        // Leave raw mode however we can before the next step.
        session.write(b"\r".to_vec());
    }

    // A burst of 200,000 lines, timed until the end marker is on screen.
    let wakeups_before = rec.state.lock().unwrap().wakeups;
    let t0 = Instant::now();
    session.write(b"burst 200000\r".to_vec());
    let d = wait_term(&session, &rec, Duration::from_secs(60), |t| {
        has_line(t, "BURST-END")
    });
    let wakeups = rec.state.lock().unwrap().wakeups - wakeups_before;
    let (history, last_line) = {
        let t = session.term.lock();
        let lines = pty::screen_lines(&t);
        let ix = lines.iter().position(|l| l == "BURST-END");
        (
            t.grid().history_size(),
            ix.and_then(|i| i.checked_sub(1))
                .map(|i| lines[i].clone())
                .unwrap_or_default(),
        )
    };
    report.check(
        &group,
        "200,000-line burst arrives complete",
        d.is_some() && last_line == "line 199999",
        true,
        format!("{}, last line {last_line:?}", ms(d)),
    );
    report.check(
        &group,
        "scrollback stays at 10,000 lines after the burst",
        history == 10_000,
        true,
        format!("history {history}"),
    );
    report.data(
        &format!("{}_burst_200k", backend.name()),
        json!({"ms": d.map(|d| round3(d.as_secs_f64() * 1000.0)), "wakeup_events": wakeups, "total_ms": round3(t0.elapsed().as_secs_f64() * 1000.0)}),
    );

    session.write(b"exit 7\r".to_vec());
    let d = wait_rec(&rec, Duration::from_secs(10), |s| s.child_exit.is_some());
    let code = rec.state.lock().unwrap().child_exit;
    report.check(
        &group,
        "child exit is reported with its code (7)",
        code == Some(Some(7)),
        true,
        format!("{} code {code:?}", ms(d)),
    );
    let d = wait_rec(&rec, Duration::from_secs(5), |s| s.exit);
    report.check(
        &group,
        "reader ends after the child exits",
        d.is_some(),
        false,
        ms(d),
    );
    session.shutdown(Duration::from_secs(3));
}

fn check_pty_shell(backend: Backend, report: &mut Report) {
    let group = format!("pty {} / system shell", backend.name());
    let rec = Arc::new(Recorder::default());
    let mut spec = if cfg!(windows) {
        SpawnSpec::new(
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into()),
            &[],
        )
    } else {
        SpawnSpec::new("/bin/sh", &[])
    };
    spec.env.push(("PS1".into(), "$ ".into()));
    let session = match Session::spawn(backend, &spec, rec.callback()) {
        Ok(s) => s,
        Err(e) => {
            report.check(&group, "spawn", false, true, e.to_string());
            return;
        }
    };
    let prompt = wait_term(&session, &rec, Duration::from_secs(15), |t| {
        pty::screen_lines(t).iter().any(|l| !l.trim().is_empty())
    });
    report.check(
        &group,
        format!("spawn {} and see a prompt", spec.program).as_str(),
        prompt.is_some(),
        true,
        ms(prompt),
    );
    let (cmd, want): (&[u8], &str) = if cfg!(windows) {
        (b"set /a 6*7\r", "42")
    } else {
        (b"echo marker-$((6*7))\r", "marker-42")
    };
    session.write(cmd.to_vec());
    let d = wait_term(&session, &rec, Duration::from_secs(10), |t| {
        has_line(t, want)
    });
    report.check(
        &group,
        "a command runs and its output appears",
        d.is_some(),
        true,
        ms(d),
    );

    session.resize(90, 20, 8, 16);
    if cfg!(windows) {
        session.write(b"mode con\r".to_vec());
        let d = wait_term(&session, &rec, Duration::from_secs(10), |t| {
            let lines = pty::screen_lines(t);
            let has = |label: &str, n: &str| {
                lines
                    .iter()
                    .any(|l| l.trim().starts_with(label) && l.trim().ends_with(n))
            };
            has("Lines:", "20") && has("Columns:", "90")
        });
        report.check(
            &group,
            "after a resize, `mode con` reports 90 columns and 20 lines",
            d.is_some(),
            false,
            ms(d),
        );
    } else {
        session.write(b"stty size\r".to_vec());
        let d = wait_term(&session, &rec, Duration::from_secs(10), |t| {
            has_line(t, "20 90")
        });
        report.check(
            &group,
            "after a resize, `stty size` reports 20 90",
            d.is_some(),
            true,
            ms(d),
        );
    }
    session.write(b"exit 3\r".to_vec());
    let d = wait_rec(&rec, Duration::from_secs(10), |s| s.child_exit.is_some());
    let code = rec.state.lock().unwrap().child_exit;
    report.check(
        &group,
        "shell exit code (3) is reported",
        code == Some(Some(3)),
        true,
        format!("{} code {code:?}", ms(d)),
    );
    session.shutdown(Duration::from_secs(3));
}

#[cfg(unix)]
fn check_pty_vi(backend: Backend, report: &mut Report) {
    let group = format!("pty {} / vi on the alternate screen", backend.name());
    // A short relative name in the temp folder, so vi's file message fits on one line (no "Press ENTER" prompt).
    let dir = std::env::temp_dir();
    let name = format!("ct{}.txt", std::process::id() % 100_000);
    let file = dir.join(&name);
    let _ = std::fs::write(&file, "first line\nsecond line\n");
    let rec = Arc::new(Recorder::default());
    let mut spec = SpawnSpec::new("vi", &["-u", "NONE", "-i", "NONE", "-n", &name]);
    spec.cwd = Some(dir);
    let session = match Session::spawn(backend, &spec, rec.callback()) {
        Ok(s) => s,
        Err(e) => {
            report.check(&group, "spawn vi", false, false, e.to_string());
            return;
        }
    };
    let alt = wait_term(&session, &rec, Duration::from_secs(10), |t| {
        t.mode().contains(TermMode::ALT_SCREEN) && has_line(t, "second line")
    });
    let detail = if alt.is_some() {
        ms(alt)
    } else {
        format!("timed out; {}", screen_dump(&session))
    };
    report.check(
        &group,
        "vi switches to the alternate screen and draws the file",
        alt.is_some(),
        false,
        detail,
    );
    if alt.is_some() {
        session.write(b"Ohello from vi\x1b".to_vec());
        let d = wait_term(&session, &rec, Duration::from_secs(5), |t| {
            has_line(t, "hello from vi")
        });
        report.check(&group, "typed text appears", d.is_some(), false, ms(d));
        session.resize(70, 18, 8, 16);
        session.write(b":set columns? lines?\r".to_vec());
        // vim prints the two values on their own lines above a "Press ENTER" prompt.
        let d = wait_term(&session, &rec, Duration::from_secs(5), |t| {
            let screen = pty::screen_lines(t).join("\n");
            screen.contains("columns=70") && screen.contains("lines=18")
        });
        let detail = if d.is_some() {
            ms(d)
        } else {
            format!("timed out; {}", screen_dump(&session))
        };
        report.check(
            &group,
            "after a resize vi reports columns=70 lines=18",
            d.is_some(),
            false,
            detail,
        );
        session.write(b"\r:q!\r".to_vec());
        let d = wait_rec(&rec, Duration::from_secs(5), |s| s.child_exit.is_some());
        let alt_off = !session.term.lock().mode().contains(TermMode::ALT_SCREEN);
        report.check(
            &group,
            "quitting leaves the alternate screen and exits",
            d.is_some() && alt_off,
            false,
            format!("{} alt off {alt_off}", ms(d)),
        );
    }
    session.shutdown(Duration::from_secs(3));
    let _ = std::fs::remove_file(&file);
}

// ---------------------------------------------------------------------------------------------------------------------
// The child helper that runs inside the PTY.

mod child {
    use std::io::{BufRead, Write};

    pub fn run() {
        let stdin = std::io::stdin();
        let mut out = std::io::stdout();
        let _ = writeln!(out, "READY");
        let _ = out.flush();
        let mut line = String::new();
        loop {
            line.clear();
            if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            let cmd = line.trim();
            let (word, rest) = cmd.split_once(' ').unwrap_or((cmd, ""));
            match word {
                "echo" => {
                    let _ = writeln!(out, "ECHO {rest}");
                }
                "size" => {
                    let (cols, rows) = size();
                    let _ = writeln!(out, "SIZE {cols}x{rows}");
                }
                "keys" => keys(rest.parse().unwrap_or(0), &mut out),
                "burst" => {
                    let n: usize = rest.parse().unwrap_or(0);
                    let mut buf = std::io::BufWriter::with_capacity(1 << 16, out.lock());
                    for i in 0..n {
                        let _ = writeln!(buf, "line {i}");
                    }
                    let _ = writeln!(buf, "BURST-END");
                }
                "exit" => std::process::exit(rest.parse().unwrap_or(0)),
                _ => {
                    let _ = writeln!(out, "UNKNOWN {cmd}");
                }
            }
            let _ = out.flush();
        }
    }

    #[cfg(unix)]
    fn size() -> (u16, u16) {
        // SAFETY: TIOCGWINSZ fills the winsize struct.
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) };
        (ws.ws_col, ws.ws_row)
    }

    #[cfg(windows)]
    fn size() -> (u16, u16) {
        use windows_sys::Win32::System::Console::{
            CONSOLE_SCREEN_BUFFER_INFO, GetConsoleScreenBufferInfo, GetStdHandle, STD_OUTPUT_HANDLE,
        };
        // SAFETY: plain Win32 calls on our own console handle.
        unsafe {
            let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            GetConsoleScreenBufferInfo(GetStdHandle(STD_OUTPUT_HANDLE), &mut info);
            let w = info.srWindow;
            ((w.Right - w.Left + 1) as u16, (w.Bottom - w.Top + 1) as u16)
        }
    }

    /// Raw mode: no line editing, no echo, no signals, no CR translation; read `n` keystrokes and print their bytes.
    #[cfg(unix)]
    fn keys(n: usize, out: &mut std::io::Stdout) {
        // SAFETY: termios calls on our own stdin.
        let mut saved: libc::termios = unsafe { std::mem::zeroed() };
        unsafe {
            libc::tcgetattr(0, &mut saved);
            let mut raw = saved;
            libc::cfmakeraw(&mut raw);
            libc::tcsetattr(0, libc::TCSANOW, &raw);
        }
        let _ = write!(out, "RAW\r\n");
        let _ = out.flush();
        let mut buf = [0u8; 64];
        for i in 0..n {
            // SAFETY: reading into a local buffer.
            let got = unsafe { libc::read(0, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            let bytes = if got > 0 {
                &buf[..got as usize]
            } else {
                &[][..]
            };
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            let _ = write!(out, "K{i} {hex}\r\n");
            let _ = out.flush();
        }
        unsafe { libc::tcsetattr(0, libc::TCSANOW, &saved) };
        let _ = writeln!(out, "COOKED");
    }

    /// Windows: virtual terminal input on, line input, echo and processed input off, then read through the console.
    #[cfg(windows)]
    fn keys(n: usize, out: &mut std::io::Stdout) {
        use std::io::Read;
        use windows_sys::Win32::System::Console::{
            ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
            ENABLE_VIRTUAL_TERMINAL_INPUT, GetConsoleMode, GetStdHandle, STD_INPUT_HANDLE,
            SetConsoleMode,
        };
        // SAFETY: plain Win32 calls on our own console handle.
        let (handle, saved) = unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            let mut mode = 0;
            GetConsoleMode(handle, &mut mode);
            let raw = (mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT))
                | ENABLE_VIRTUAL_TERMINAL_INPUT;
            SetConsoleMode(handle, raw);
            (handle, mode)
        };
        let _ = write!(out, "RAW\r\n");
        let _ = out.flush();
        let mut buf = [0u8; 64];
        let mut stdin = std::io::stdin();
        for i in 0..n {
            let got = stdin.read(&mut buf).unwrap_or(0);
            let hex: String = buf[..got].iter().map(|b| format!("{b:02x}")).collect();
            let _ = write!(out, "K{i} {hex}\r\n");
            let _ = out.flush();
        }
        unsafe { SetConsoleMode(handle, saved) };
        let _ = writeln!(out, "COOKED");
    }
}
