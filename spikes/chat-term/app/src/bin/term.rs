//! `term`: a terminal pane. A PTY running a shell (either backend), `alacritty_terminal` for emulation, and our own
//! GPUI element that draws the cell grid with colours, bold, italic, underline, the cursor and the selection.
//!
//! ```sh
//! cargo run --release -p chat-term-app --bin term -- [--backend portable|alacritty] [--scenario NAME] [--out f.json]
//! ```
//!
//! Scenarios: `none` (an interactive pane running the default shell), `check` (correctness checks through the real
//! element: colours, wide and combining characters, scrollback, a full-screen program on the alternate screen, window
//! resize, bracketed paste, key mapping), `burst` (200,000 lines), `echo` (input-to-echo latency), `idle` and
//! `idle-blink` (30 s each). Scenarios run `/bin/sh` with a fixed prompt rather than the user's shell, so results do
//! not depend on personal shell configuration.
//!
//! Text is drawn per run of cells with one style, shaped with GPUI's `force_width` so every glyph lands on its cell.
//! A wide character is shaped on its own and drawn at its column. Selection and copy use `alacritty_terminal`'s own
//! selection model.

#[path = "../common.rs"]
mod common;

use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point as TermPoint, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Rgb};
use chat_term_core::keys;
use chat_term_core::pty::{self, Backend, Listener, Session, SpawnSpec, TermEvent};
use chat_term_core::stats;
use common::{Probe, Recorder, SharedRecorder, UsageWindow, arg, build_profile, ms};
use gpui::{
    App, Bounds, ClipboardItem, Context, Element, ElementId, FocusHandle, Focusable, Font,
    FontStyle, FontWeight, GlobalElementId, Hsla, InspectorElementId, KeyDownEvent, Keystroke,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, ShapedLine,
    Style, TextAlign, TextRun, TitlebarOptions, UnderlineStyle, Window, WindowBounds,
    WindowOptions, div, fill, point, prelude::*, px, relative, rgb, size,
};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------------------------------------------------
// Colours

fn rgb_hsla(c: Rgb) -> Hsla {
    rgb(((c.r as u32) << 16) | ((c.g as u32) << 8) | c.b as u32).into()
}

/// The xterm 256-colour palette.
fn indexed(i: u8) -> Rgb {
    const BASE: [(u8, u8, u8); 16] = [
        (0x1e, 0x1e, 0x1e),
        (0xcd, 0x31, 0x31),
        (0x0d, 0xbc, 0x79),
        (0xe5, 0xe5, 0x10),
        (0x24, 0x72, 0xc8),
        (0xbc, 0x3f, 0xbc),
        (0x11, 0xa8, 0xcd),
        (0xe5, 0xe5, 0xe5),
        (0x66, 0x66, 0x66),
        (0xf1, 0x4c, 0x4c),
        (0x23, 0xd1, 0x8b),
        (0xf5, 0xf5, 0x43),
        (0x3b, 0x8e, 0xea),
        (0xd6, 0x70, 0xd6),
        (0x29, 0xb8, 0xdb),
        (0xff, 0xff, 0xff),
    ];
    match i {
        0..=15 => {
            let (r, g, b) = BASE[i as usize];
            Rgb { r, g, b }
        }
        16..=231 => {
            let n = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Rgb {
                r: level(n / 36),
                g: level((n / 6) % 6),
                b: level(n % 6),
            }
        }
        _ => {
            let v = 8 + (i - 232) * 10;
            Rgb { r: v, g: v, b: v }
        }
    }
}

const FG: Rgb = Rgb {
    r: 0xd4,
    g: 0xd4,
    b: 0xd4,
};
const BG: Rgb = Rgb {
    r: 0x1e,
    g: 0x1e,
    b: 0x1e,
};

fn resolve(color: Color, colors: &alacritty_terminal::term::color::Colors) -> Rgb {
    match color {
        Color::Spec(rgb) => rgb,
        Color::Indexed(i) => colors[i as usize].unwrap_or_else(|| indexed(i)),
        Color::Named(n) => colors[n].unwrap_or(match n {
            NamedColor::Foreground | NamedColor::BrightForeground => FG,
            NamedColor::Background => BG,
            NamedColor::Cursor => FG,
            n if (n as usize) < 16 => indexed(n as u8),
            n if (n as usize) >= NamedColor::DimBlack as usize
                && (n as usize) <= NamedColor::DimWhite as usize =>
            {
                let c = indexed((n as usize - NamedColor::DimBlack as usize) as u8);
                Rgb {
                    r: c.r / 3 * 2,
                    g: c.g / 3 * 2,
                    b: c.b / 3 * 2,
                }
            }
            _ => FG,
        }),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// State shared between the view and the element.

#[derive(Default)]
struct Shared {
    cell: Option<(Pixels, Pixels)>,
    origin: Point<Pixels>,
    grid: (u16, u16),
    /// The grid before a scripted window resize, so the check can insist that it changed.
    grid_before_resize: Option<(u16, u16)>,
    /// Checks that the element evaluates while it holds the terminal lock in prepaint.
    expect_echo: Option<(String, Instant)>,
    echo_hit: Option<Instant>,
    echo_latencies: Vec<f64>,
    expect_final: Option<(String, Instant)>,
    final_hit: Option<Instant>,
    final_ms: Option<f64>,
    /// What the last frame drew, for the record.
    last_runs: usize,
    last_wide_glyphs: usize,
    last_bg_rects: usize,
    blink_on: bool,
}

struct TerminalView {
    session: Rc<Session>,
    focus: FocusHandle,
    shared: Rc<RefCell<Shared>>,
    rec: SharedRecorder,
    dirty: Arc<AtomicBool>,
    font: Font,
    font_size: Pixels,
    blink: bool,
    child_exit: Option<Option<i32>>,
    selecting: bool,
    scenario: String,
    steps: Vec<Step>,
    step_ix: usize,
    step_started: Option<Instant>,
    results: Vec<Value>,
    phase_usage: Option<UsageWindow>,
    out: Option<String>,
    finished: bool,
    started: bool,
    echo_plan: Option<EchoPlan>,
    resize_query_sent: std::cell::Cell<bool>,
}

struct EchoPlan {
    left: u32,
    typed: String,
    prompt: String,
    rng: chat_term_core::content::Rng,
}

type StepFn = Box<dyn Fn(&mut TerminalView, &mut Window, &mut Context<TerminalView>)>;
type DoneFn = Box<dyn Fn(&TerminalView, &Term<Listener>) -> bool>;

struct Step {
    name: String,
    start: StepFn,
    done: DoneFn,
    timeout: Duration,
}

fn step(
    name: &str,
    timeout_s: f64,
    start: impl Fn(&mut TerminalView, &mut Window, &mut Context<TerminalView>) + 'static,
    done: impl Fn(&TerminalView, &Term<Listener>) -> bool + 'static,
) -> Step {
    Step {
        name: name.into(),
        start: Box::new(start),
        done: Box::new(done),
        timeout: Duration::from_secs_f64(timeout_s),
    }
}

fn screen_has(t: &Term<Listener>, want: &str) -> bool {
    pty::screen_lines(t).iter().any(|l| l.contains(want))
}

fn write(
    text: &'static str,
) -> impl Fn(&mut TerminalView, &mut Window, &mut Context<TerminalView>) {
    move |v, _, _| v.session.write(text.as_bytes().to_vec())
}

impl TerminalView {
    fn key_down(&mut self, ev: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        if ks.modifiers.platform {
            match ks.key.as_str() {
                "c" => {
                    if let Some(text) = self.session.term.lock().selection_to_string() {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                        let bracketed = self
                            .session
                            .term
                            .lock()
                            .mode()
                            .contains(TermMode::BRACKETED_PASTE);
                        self.session.write(keys::paste(&text, bracketed));
                    }
                }
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        let app_cursor = self
            .session
            .term
            .lock()
            .mode()
            .contains(TermMode::APP_CURSOR);
        if let Some(bytes) = keys::encode(&key_from(ks), app_cursor) {
            {
                let mut term = self.session.term.lock();
                term.scroll_display(Scroll::Bottom);
                term.selection = None;
            }
            self.session.write(bytes);
            cx.stop_propagation();
        }
    }

    fn point_at(&self, position: Point<Pixels>) -> Option<(TermPoint, Side)> {
        let shared = self.shared.borrow();
        let (cw, ch) = shared.cell?;
        let x = (position.x - shared.origin.x) / cw;
        let y = (position.y - shared.origin.y) / ch;
        let col = (x.max(0.) as usize).min(shared.grid.0.saturating_sub(1) as usize);
        let row = (y.max(0.) as usize).min(shared.grid.1.saturating_sub(1) as usize);
        let side = if x.fract() < 0.5 {
            Side::Left
        } else {
            Side::Right
        };
        let offset = self.session.term.lock().grid().display_offset() as i32;
        Some((TermPoint::new(Line(row as i32 - offset), Column(col)), side))
    }

    fn start_scenario(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt_ready = |_: &TerminalView, t: &Term<Listener>| screen_has(t, "$");
        let mut steps = vec![step("shell prompt", 10., |_, _, _| {}, prompt_ready)];
        match self.scenario.as_str() {
            "check" => {
                steps.extend([
                    step("24-bit colour cells", 10., write("printf '\\033[38;2;255;100;0mTRUE\\033[48;2;0;80;160mCOLOR\\033[0m\\n'\r"), |_, t| {
                        find_cell(t, "TRUE").is_some_and(|(l, c)| t.grid()[l][c].fg == Color::Spec(Rgb { r: 255, g: 100, b: 0 }))
                            && find_cell(t, "COLOR").is_some_and(|(l, c)| t.grid()[l][c].bg == Color::Spec(Rgb { r: 0, g: 80, b: 160 }))
                    }),
                    step("256-colour table", 10., write("awk 'BEGIN{for(i=0;i<256;i++) printf \"\\033[38;5;%dm#\", i; print \"\\033[0mEND256\"}'\r"), |_, t| {
                        screen_has(t, "END256") && pty::screen_lines(t).iter().any(|l| l.contains(&"#".repeat(64)))
                    }),
                    step("bold, italic, underline", 10., write("printf '\\033[1mBOLD\\033[0m \\033[3mITAL\\033[0m \\033[4mUNDR\\033[0m\\n'\r"), |_, t| {
                        find_cell(t, "BOLD").is_some_and(|(l, c)| t.grid()[l][c].flags.contains(Flags::BOLD))
                            && find_cell(t, "ITAL").is_some_and(|(l, c)| t.grid()[l][c].flags.contains(Flags::ITALIC))
                            && find_cell(t, "UNDR").is_some_and(|(l, c)| t.grid()[l][c].flags.contains(Flags::UNDERLINE))
                    }),
                    step("wide and combining characters", 10., write("printf 'W:\\344\\275\\240\\345\\245\\275 C:e\\314\\201 E:\\360\\237\\232\\200\\n'\r"), |v, t| {
                        find_cell(t, "W:").is_some_and(|(l, c)| t.grid()[l][c + 2].flags.contains(Flags::WIDE_CHAR))
                            && find_cell(t, "C:").is_some_and(|(l, c)| t.grid()[l][c + 2].zerowidth().is_some_and(|z| z.contains(&'\u{301}')))
                            && v.shared.borrow().last_wide_glyphs >= 3
                    }),
                    step("scrollback keeps 10,000 lines", 20., write("seq 1 20000; echo SEQDONE\r"), |_, t| {
                        screen_has(t, "SEQDONE") && t.grid().history_size() == 10_000
                    }),
                    step("full-screen program: vi on the alternate screen", 10., write("vi -u NONE -i NONE -n\r"), |_, t| {
                        t.mode().contains(TermMode::ALT_SCREEN)
                    }),
                    step("vi: typed text appears", 5., write("ihello from vi\x1b"), |_, t| screen_has(t, "hello from vi")),
                    step("window resize reaches the PTY: vi reports the element's new grid", 10., |v, window, _| {
                        let before = v.shared.borrow().grid;
                        v.shared.borrow_mut().grid_before_resize = Some(before);
                        window.resize(size(px(700.), px(420.)));
                    }, |v, t| {
                        let (cols, rows) = v.shared.borrow().grid;
                        if Some((cols, rows)) == v.shared.borrow().grid_before_resize {
                            return false; // the window has not been laid out at its new size yet
                        }
                        if !v.resize_query_sent.get() {
                            v.resize_query_sent.set(true);
                            v.session.write(b":set columns? lines?\r".to_vec());
                            return false;
                        }
                        let screen = pty::screen_lines(t).join("\n");
                        t.columns() == cols as usize
                            && t.screen_lines() == rows as usize
                            && screen.contains(&format!("columns={cols}"))
                            && screen.contains(&format!("lines={rows}"))
                    }),
                    step("vi: quit leaves the alternate screen", 5., write("\r:q!\r"), |_, t| !t.mode().contains(TermMode::ALT_SCREEN)),
                    step("bracketed paste mode is seen and pastes are wrapped", 5., write("printf '\\033[?2004h'; echo BPON\r"), |_, t| {
                        screen_has(t, "BPON")
                            && t.mode().contains(TermMode::BRACKETED_PASTE)
                            && keys::paste("x\ny", true) == b"\x1b[200~x\ny\x1b[201~"
                    }),
                    step("GPUI keystrokes map to the expected bytes", 1., |_, _, _| {}, |_, _| keystroke_table_ok().0),
                    step("selection and copy text", 5., write("printf '\\033[?2004l'; echo SELECT-ME-PLEASE\r"), |_, t| screen_has(t, "SELECT-ME-PLEASE")),
                ]);
            }
            "burst" => {
                steps.push(step(
                    "200,000 lines drawn to the final state",
                    60.,
                    |v, _, _| {
                        v.rec.borrow_mut().begin_window();
                        v.phase_usage = Some(UsageWindow::begin());
                        v.shared.borrow_mut().expect_final =
                            Some(("200000".into(), Instant::now()));
                        v.session.write(b"seq 1 200000\r".to_vec());
                    },
                    |v, _| v.shared.borrow().final_ms.is_some(),
                ));
            }
            "echo" => {
                steps.push(step(
                    "100 keystrokes echoed",
                    60.,
                    |v, window, cx| {
                        // Keystrokes come from a timer at random gaps, so they land at random points of the frame cycle
                        // (as a person's typing does), not right at a frame signal.
                        cx.spawn_in(window, async move |this, cx| {
                            loop {
                                let wait = this.update(cx, |v, cx| v.drive_echo(cx)).ok().flatten();
                                match wait {
                                    Some(d) => cx.background_executor().timer(d).await,
                                    None => break,
                                }
                            }
                        })
                        .detach();
                        v.rec.borrow_mut().begin_window();
                        v.phase_usage = Some(UsageWindow::begin());
                        // The prompt is the cursor's line up to the cursor (screen lines are trimmed, the prompt's space is not).
                        let prompt = {
                            let t = v.session.term.lock();
                            let cursor = t.grid().cursor.point;
                            let line = pty::screen_lines(&t)
                                .get(cursor.line.0.max(0) as usize)
                                .cloned()
                                .unwrap_or_default();
                            format!("{line:<width$}", width = cursor.column.0)
                        };
                        v.echo_plan = Some(EchoPlan {
                            left: 100,
                            typed: String::new(),
                            prompt,
                            rng: chat_term_core::content::Rng::new(3),
                        });
                    },
                    |v, _| {
                        v.echo_plan.as_ref().is_some_and(|p| p.left == 0)
                            && v.shared.borrow().expect_echo.is_none()
                    },
                ));
            }
            "idle" | "idle-blink" => {
                steps.push(step(
                    "idle 30 s",
                    40.,
                    |v, _, _| {
                        v.rec.borrow_mut().begin_window();
                        v.phase_usage = Some(UsageWindow::begin());
                    },
                    |v, _| {
                        v.step_started
                            .is_some_and(|s| s.elapsed() >= Duration::from_secs(30))
                    },
                ));
            }
            _ => steps.clear(),
        }
        self.steps = steps;
        self.step_ix = 0;
        self.step_started = None;
        if !self.steps.is_empty() {
            self.schedule_drive(window, cx);
        }
    }

    fn schedule_drive(&self, window: &mut Window, cx: &mut Context<Self>) {
        let idle_step =
            self.scenario.starts_with("idle") && self.step_ix >= 1 && self.step_started.is_some();
        if idle_step {
            // While idle, one timer for the whole step: the app itself schedules nothing else.
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_secs(30))
                    .await;
                let _ = this.update_in(cx, |view, window, cx| view.drive(window, cx));
            })
            .detach();
        } else {
            let this = cx.entity();
            window.on_next_frame(move |window, cx| {
                this.update(cx, |view, cx| view.drive(window, cx));
            });
        }
    }

    fn drive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.finished {
            return;
        }
        let in_measured_step = !self.scenario.starts_with("idle") || self.step_ix == 0;
        if in_measured_step {
            self.rec.borrow_mut().signals.push(Instant::now());
        }
        let Some(current) = self.steps.get(self.step_ix) else {
            self.finish(window, cx);
            return;
        };
        match self.step_started {
            None => {
                self.step_started = Some(Instant::now());
                let start =
                    std::mem::replace(&mut self.steps[self.step_ix].start, Box::new(|_, _, _| {}));
                start(self, window, cx);
                cx.notify();
            }
            Some(started) => {
                let done = { (current.done)(self, &self.session.term.lock()) };
                let timed_out = started.elapsed() > current.timeout;
                if done || timed_out {
                    let name = current.name.clone();
                    let elapsed = started.elapsed();
                    let mut result = json!({"step": name, "ok": done, "ms": ms(elapsed)});
                    if !done {
                        let t = self.session.term.lock();
                        result["screen"] = json!(
                            pty::screen_lines(&t)
                                .into_iter()
                                .filter(|l| !l.is_empty())
                                .collect::<Vec<_>>()
                        );
                    }
                    if self.step_ix > 0
                        && matches!(
                            self.scenario.as_str(),
                            "burst" | "echo" | "idle" | "idle-blink"
                        )
                    {
                        self.add_measurements(&mut result, window, cx);
                    }
                    if name.starts_with("GPUI keystrokes") {
                        result["detail"] = json!(keystroke_table_ok().1);
                    }
                    if name.starts_with("window resize") {
                        result["grid_before"] = json!(self.shared.borrow().grid_before_resize);
                        result["grid_after"] = json!(self.shared.borrow().grid);
                    }
                    if name.starts_with("selection") && done {
                        result["copied"] = json!(self.select_line_containing("SELECT-ME-PLEASE"));
                    }
                    println!("term: {} {}", if done { "PASS" } else { "FAIL" }, name);
                    self.results.push(result);
                    self.step_ix += 1;
                    self.step_started = None;
                }
            }
        }
        if self.step_ix < self.steps.len() {
            self.schedule_drive(window, cx);
        } else {
            self.finish(window, cx);
        }
    }

    /// Select the visible line holding `needle` with alacritty's selection model and return the selected text.
    fn select_line_containing(&self, needle: &str) -> Option<String> {
        let mut term = self.session.term.lock();
        let lines = pty::screen_lines(&term);
        let row = lines.iter().rposition(|l| l.trim() == needle)?;
        let start = TermPoint::new(Line(row as i32), Column(0));
        let end = TermPoint::new(Line(row as i32), Column(needle.chars().count() - 1));
        let mut selection = Selection::new(SelectionType::Simple, start, Side::Left);
        selection.update(end, Side::Right);
        term.selection = Some(selection);
        term.selection_to_string()
    }

    /// Type the next character if the previous one has been seen; returns how long to wait before trying again,
    /// or `None` when the plan is finished.
    fn drive_echo(&mut self, _cx: &mut Context<Self>) -> Option<Duration> {
        let plan = self.echo_plan.as_mut()?;
        let mut shared = self.shared.borrow_mut();
        if plan.left == 0 {
            return None;
        }
        if shared.expect_echo.is_some() {
            return Some(Duration::from_millis(1));
        }
        if plan.typed.len() >= 20 {
            // Clear the line with ctrl-U and start again.
            self.session.write(vec![0x15]);
            plan.typed.clear();
            return Some(Duration::from_millis(50));
        }
        let c = (b'a' + (plan.rng.range(0, 25) as u8)) as char;
        plan.typed.push(c);
        let expected = format!("{}{}", plan.prompt, plan.typed);
        shared.expect_echo = Some((expected, Instant::now()));
        self.session.write(vec![c as u8]);
        plan.left -= 1;
        Some(Duration::from_millis(20 + plan.rng.range(0, 47) as u64))
    }

    fn add_measurements(
        &mut self,
        result: &mut Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let refresh = window
            .refresh_interval(cx)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(16.667);
        result["frames"] = self.rec.borrow().frame_stats(refresh);
        result["process"] = json!(self.phase_usage.take().map(|u| u.end()));
        let shared = self.shared.borrow();
        if self.scenario == "burst" {
            result["write_to_final_frame_painted_ms"] = json!(shared.final_ms);
        }
        if self.scenario == "echo" {
            result["keystroke_to_echo_painted"] = stats::summary_ms(&shared.echo_latencies);
        }
    }

    fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.finished {
            return;
        }
        self.finished = true;
        let shared = self.shared.borrow();
        let doc = json!({
            "scenario": self.scenario,
            "backend": self.session.backend.name(),
            "build_profile": build_profile(),
            "os": std::env::consts::OS,
            "blink": self.blink,
            "window": common::window_facts(window, cx),
            "grid": shared.grid,
            "cell_px": shared.cell.map(|(w, h)| (f32::from(w), f32::from(h))),
            "last_frame": {"text_runs": shared.last_runs, "wide_glyphs": shared.last_wide_glyphs, "background_rects": shared.last_bg_rects},
            "child_exit": self.child_exit,
            "steps": self.results,
            "process_at_exit": common::process_snapshot(),
        });
        drop(shared);
        common::write_json(self.out.as_deref(), &doc);
        cx.quit();
    }
}

fn key_from(ks: &Keystroke) -> keys::Key<'_> {
    keys::Key {
        key: &ks.key,
        key_char: ks.key_char.as_deref(),
        ctrl: ks.modifiers.control,
        alt: ks.modifiers.alt,
        shift: ks.modifiers.shift,
    }
}

/// GPUI keystrokes, parsed from GPUI's own notation, through `key_from` and the encoder.
fn keystroke_table_ok() -> (bool, String) {
    let cases: [(&'static str, &[u8]); 10] = [
        ("ctrl-c", b"\x03"),
        ("ctrl-d", b"\x04"),
        ("alt-b", b"\x1bb"),
        ("up", b"\x1b[A"),
        ("ctrl-right", b"\x1b[1;5C"),
        ("shift-tab", b"\x1b[Z"),
        ("enter", b"\r"),
        ("backspace", b"\x7f"),
        ("escape", b"\x1b"),
        ("pageup", b"\x1b[5~"),
    ];
    let mut wrong = Vec::new();
    for (source, want) in cases {
        match Keystroke::parse(source) {
            Ok(ks) => {
                let got = keys::encode(&key_from(&ks), false).unwrap_or_default();
                if got != want {
                    wrong.push(format!("{source}: got {got:?}"));
                }
            }
            Err(e) => wrong.push(format!("{source}: {e}")),
        }
    }
    (
        wrong.is_empty(),
        format!("{} keystrokes; {}", cases.len(), wrong.join("; ")),
    )
}

fn find_cell(t: &Term<Listener>, needle: &str) -> Option<(Line, Column)> {
    let lines = pty::screen_lines(t);
    for (row, text) in lines.iter().enumerate().rev() {
        if let Some(byte) = text.find(needle) {
            // Column = number of cells before `byte` (wide characters count twice).
            let col: usize = text[..byte]
                .chars()
                .map(|c| {
                    if (c as u32) >= 0x1100 && unicode_wide(c) {
                        2
                    } else {
                        1
                    }
                })
                .sum();
            return Some((Line(row as i32), Column(col)));
        }
    }
    None
}

fn unicode_wide(c: char) -> bool {
    matches!(c as u32, 0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x1F300..=0x1F64F | 0x1F900..=0x1F9FF | 0x20000..=0x3FFFD)
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        {
            let mut rec = self.rec.borrow_mut();
            rec.render_start = Some(now);
            rec.renders += 1;
        }
        self.dirty.store(false, Ordering::Release);
        if !self.started {
            self.started = true;
            window.focus(&self.focus, cx);
            let this = cx.entity();
            window.on_next_frame(move |window, cx| {
                this.update(cx, |view, cx| view.start_scenario(window, cx));
            });
        }
        let rec = self.rec.clone();
        let shared = self.shared.clone();
        let measuring = !self.scenario.starts_with("idle") || self.step_ix >= 1;
        div()
            .size_full()
            .bg(rgb_hsla(BG))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                    if let Some((p, side)) = this.point_at(ev.position) {
                        this.session.term.lock().selection =
                            Some(Selection::new(SelectionType::Simple, p, side));
                        this.selecting = true;
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if this.selecting && ev.pressed_button == Some(MouseButton::Left) {
                    if let Some((p, side)) = this.point_at(ev.position)
                        && let Some(sel) = this.session.term.lock().selection.as_mut()
                    {
                        sel.update(p, side);
                    }
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_scroll(cx.listener(|this, ev: &gpui::ScrollEvent, _, cx| {
                let cell_h = this.shared.borrow().cell.map(|c| c.1).unwrap_or(px(16.));
                let lines = (ev.delta.pixel_delta(cell_h).y / cell_h).round() as i32;
                if lines != 0 {
                    this.session
                        .term
                        .lock()
                        .scroll_display(Scroll::Delta(lines));
                    cx.notify();
                }
            }))
            .child(TermElement {
                term: self.session.clone(),
                shared: self.shared.clone(),
                font: self.font.clone(),
                font_size: self.font_size,
                show_cursor: !self.blink || self.shared.borrow().blink_on,
            })
            .child(Probe::new(move |_| {
                let paint_end = Instant::now();
                if measuring {
                    let mut rec = rec.borrow_mut();
                    let render_start = rec.render_start.unwrap_or(paint_end);
                    rec.frames.push(common::Frame {
                        render_start,
                        paint_end,
                    });
                }
                let mut s = shared.borrow_mut();
                if let Some(t0) = s.echo_hit.take() {
                    s.echo_latencies
                        .push((paint_end - t0).as_secs_f64() * 1000.0);
                    s.expect_echo = None;
                }
                if let Some(t0) = s.final_hit.take() {
                    s.final_ms = Some(ms(paint_end - t0));
                    s.expect_final = None;
                }
            }))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The grid element.

struct TermElement {
    term: Rc<Session>,
    shared: Rc<RefCell<Shared>>,
    font: Font,
    font_size: Pixels,
    show_cursor: bool,
}

struct Prepared {
    bgs: Vec<(Bounds<Pixels>, Hsla)>,
    lines: Vec<(Point<Pixels>, ShapedLine)>,
    cursor: Option<(Bounds<Pixels>, Hsla)>,
    selection: Vec<Bounds<Pixels>>,
    line_height: Pixels,
}

impl IntoElement for TermElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for TermElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepared;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) -> Prepared {
        let text_system = window.text_system().clone();
        let font_id = text_system.resolve_font(&self.font);
        let cell_w = text_system
            .advance(font_id, self.font_size, 'M')
            .map(|s| s.width)
            .unwrap_or(px(8.));
        let cell_h = (self.font_size * 1.35).round();
        let cols = ((bounds.size.width / cell_w).floor() as u16).max(2);
        let rows = ((bounds.size.height / cell_h).floor() as u16).max(1);
        {
            let mut shared = self.shared.borrow_mut();
            shared.cell = Some((cell_w, cell_h));
            shared.origin = bounds.origin;
            if shared.grid != (cols, rows) {
                shared.grid = (cols, rows);
                drop(shared);
                self.term.resize(
                    cols,
                    rows,
                    f32::from(cell_w) as u16,
                    f32::from(cell_h) as u16,
                );
            }
        }

        let term = self.term.term.lock();
        let content = term.renderable_content();
        let colors = content.colors;
        let offset = content.display_offset as i32;
        let default_bg = rgb_hsla(BG);
        let mut bgs: Vec<(Bounds<Pixels>, Hsla)> = Vec::new();
        let mut lines: Vec<(Point<Pixels>, ShapedLine)> = Vec::new();
        let mut wide_glyphs = 0;

        let mut run: Option<Run> = None;
        let flush = |run: &mut Option<Run>, lines: &mut Vec<(Point<Pixels>, ShapedLine)>| {
            if let Some(r) = run.take() {
                if r.text.trim().is_empty() && !r.flags.contains(Flags::UNDERLINE) {
                    return;
                }
                let mut font = self.font.clone();
                if r.flags.contains(Flags::BOLD) {
                    font.weight = FontWeight::BOLD;
                }
                if r.flags.contains(Flags::ITALIC) {
                    font.style = FontStyle::Italic;
                }
                let text_run = TextRun {
                    len: r.text.len(),
                    font,
                    color: r.fg,
                    background_color: None,
                    underline: r
                        .flags
                        .contains(Flags::UNDERLINE)
                        .then_some(UnderlineStyle {
                            color: Some(r.fg),
                            thickness: px(1.),
                            wavy: false,
                        }),
                    strikethrough: None,
                };
                let shaped = text_system.shape_line(
                    r.text.into(),
                    self.font_size,
                    &[text_run],
                    Some(cell_w),
                );
                let origin = point(
                    bounds.origin.x + cell_w * r.col as f32,
                    bounds.origin.y + cell_h * r.row as f32,
                );
                lines.push((origin, shaped));
            }
        };

        for indexed in content.display_iter {
            let cell = indexed.cell;
            let row = indexed.point.line.0 + offset;
            let col = indexed.point.column.0;
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                flush(&mut run, &mut lines);
                continue;
            }
            let (mut fg, mut bg) = (resolve(cell.fg, colors), resolve(cell.bg, colors));
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let mut fg = rgb_hsla(fg);
            if cell.flags.contains(Flags::DIM) {
                fg.a *= 0.7;
            }
            let bg = rgb_hsla(bg);
            let width = if cell.flags.contains(Flags::WIDE_CHAR) {
                2.
            } else {
                1.
            };
            if bg != default_bg {
                let b = Bounds::new(
                    point(
                        bounds.origin.x + cell_w * col as f32,
                        bounds.origin.y + cell_h * row as f32,
                    ),
                    size(cell_w * width, cell_h),
                );
                match bgs.last_mut() {
                    Some((last, color))
                        if *color == bg && last.top() == b.top() && last.right() == b.left() =>
                    {
                        last.size.width += b.size.width
                    }
                    _ => bgs.push((b, bg)),
                }
            }
            let style_flags = cell.flags & (Flags::BOLD | Flags::ITALIC | Flags::UNDERLINE);
            let mut ch = String::new();
            ch.push(if cell.flags.contains(Flags::HIDDEN) {
                ' '
            } else {
                cell.c
            });
            if let Some(zw) = cell.zerowidth() {
                ch.extend(zw.iter());
            }
            if cell.flags.contains(Flags::WIDE_CHAR) {
                // A wide glyph on its own, at its column.
                flush(&mut run, &mut lines);
                run = Some(Run {
                    row,
                    col,
                    text: ch,
                    fg,
                    flags: style_flags,
                });
                flush(&mut run, &mut lines);
                wide_glyphs += 1;
                continue;
            }
            let continues = run.as_ref().is_some_and(|r| {
                r.row == row && r.fg == fg && r.flags == style_flags && r.col + r.cells() == col
            });
            if continues {
                run.as_mut().unwrap().text.push_str(&ch);
            } else {
                flush(&mut run, &mut lines);
                run = Some(Run {
                    row,
                    col,
                    text: ch,
                    fg,
                    flags: style_flags,
                });
            }
        }
        flush(&mut run, &mut lines);

        let cursor = (self.show_cursor && content.cursor.shape != CursorShape::Hidden).then(|| {
            let p = content.cursor.point;
            let row = p.line.0 + offset;
            let origin = point(
                bounds.origin.x + cell_w * p.column.0 as f32,
                bounds.origin.y + cell_h * row as f32,
            );
            let color: Hsla = rgb_hsla(FG).opacity(0.6);
            let b = match content.cursor.shape {
                CursorShape::Beam => Bounds::new(origin, size(px(2.), cell_h)),
                CursorShape::Underline => Bounds::new(
                    point(origin.x, origin.y + cell_h - px(2.)),
                    size(cell_w, px(2.)),
                ),
                _ => Bounds::new(origin, size(cell_w, cell_h)),
            };
            (b, color)
        });

        let mut selection = Vec::new();
        if let Some(range) = content.selection {
            for line in range.start.line.0..=range.end.line.0 {
                let row = line + offset;
                if row < 0 || row >= rows as i32 {
                    continue;
                }
                let first = if line == range.start.line.0 {
                    range.start.column.0
                } else {
                    0
                };
                let last = if line == range.end.line.0 {
                    range.end.column.0
                } else {
                    cols as usize - 1
                };
                selection.push(Bounds::new(
                    point(
                        bounds.origin.x + cell_w * first as f32,
                        bounds.origin.y + cell_h * row as f32,
                    ),
                    size(cell_w * (last + 1 - first.min(last + 1)) as f32, cell_h),
                ));
            }
        }

        // Checks the scenarios evaluate on the exact state this frame draws.
        let mut shared = self.shared.borrow_mut();
        if let Some((expected, t0)) = shared.expect_echo.clone() {
            let cursor_row = content.cursor.point.line.0;
            if cursor_row >= 0 {
                let line = pty::screen_lines(&term)
                    .get(cursor_row as usize)
                    .cloned()
                    .unwrap_or_default();
                if line.trim_end() == expected.trim_end() {
                    shared.echo_hit = Some(t0);
                }
            }
        }
        if let Some((want, t0)) = shared.expect_final.clone() {
            let screen = pty::screen_lines(&term);
            if screen.iter().any(|l| l == &want)
                && screen
                    .iter()
                    .rev()
                    .find(|l| !l.is_empty())
                    .is_some_and(|l| l.starts_with('$'))
            {
                shared.final_hit = Some(t0);
            }
        }
        shared.last_runs = lines.len();
        shared.last_wide_glyphs = wide_glyphs;
        shared.last_bg_rects = bgs.len();
        Prepared {
            bgs,
            lines,
            cursor,
            selection,
            line_height: cell_h,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        prepared: &mut Prepared,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.paint_quad(fill(bounds, rgb_hsla(BG)));
        for (b, color) in &prepared.bgs {
            window.paint_quad(fill(*b, *color));
        }
        for b in &prepared.selection {
            window.paint_quad(fill(*b, gpui::hsla(0.6, 0.6, 0.5, 0.35)));
        }
        for (origin, line) in &prepared.lines {
            let _ = line.paint(
                *origin,
                prepared.line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
        if let Some((b, color)) = prepared.cursor {
            window.paint_quad(fill(b, color));
        }
    }
}

/// A run of same-styled cells on one row.
struct Run {
    row: i32,
    col: usize,
    text: String,
    fg: Hsla,
    flags: Flags,
}

impl Run {
    /// How many cells the run covers (combining marks share their base's cell).
    fn cells(&self) -> usize {
        self.text.chars().filter(|c| !is_zero_width(*c)).count()
    }
}

fn is_zero_width(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x200B..=0x200F | 0x20D0..=0x20FF | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--selftest") {
        // No window: GPUI keystrokes (parsed from GPUI's notation) through our mapping and encoder. CI runs this.
        let (ok, detail) = keystroke_table_ok();
        println!(
            "term --selftest: {} ({detail})",
            if ok { "PASS" } else { "FAIL" }
        );
        std::process::exit(if ok { 0 } else { 1 });
    }
    let backend = arg(&args, "--backend")
        .and_then(|s| Backend::parse(&s))
        .unwrap_or(Backend::Alacritty);
    let scenario = arg(&args, "--scenario").unwrap_or_else(|| "none".into());
    let out = arg(&args, "--out");
    let blink = scenario == "idle-blink" || args.iter().any(|a| a == "--blink");

    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(900.), px(600.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("chat-term trial: terminal".into()),
                ..Default::default()
            }),
            inactive_frame_interval: None,
            ..Default::default()
        };
        let scenario2 = scenario.clone();
        let opened = cx.open_window(options, move |window, cx| {
            cx.new(|cx: &mut Context<TerminalView>| {
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TermEvent>();
                let dirty = Arc::new(AtomicBool::new(false));
                let dirty2 = dirty.clone();
                // Terminal events arrive on the PTY threads. A wakeup is forwarded only when the view is not already
                // marked dirty, so a burst of output becomes one notification per frame, not one per read.
                let on_event: pty::EventFn = Arc::new(move |ev| match ev {
                    TermEvent::Wakeup => {
                        if !dirty2.swap(true, Ordering::AcqRel) {
                            let _ = tx.send(TermEvent::Wakeup);
                        }
                    }
                    other => {
                        let _ = tx.send(other);
                    }
                });
                let program = if scenario2 == "none" {
                    pty::default_shell()
                } else {
                    "/bin/sh".to_string()
                };
                let mut spec = SpawnSpec::new(program, &[]);
                spec.env.push(("PS1".into(), "$ ".into()));
                let session =
                    Rc::new(Session::spawn(backend, &spec, on_event).expect("spawn the shell"));
                cx.spawn(async move |this: gpui::WeakEntity<TerminalView>, cx| {
                    while let Some(ev) = rx.recv().await {
                        let alive = this.update(cx, |view, cx| {
                            if let TermEvent::ChildExit(code) = ev {
                                view.child_exit = Some(code);
                            }
                            cx.notify();
                        });
                        if alive.is_err() {
                            break;
                        }
                    }
                })
                .detach();
                let shared = Rc::new(RefCell::new(Shared {
                    blink_on: true,
                    ..Default::default()
                }));
                if blink {
                    let shared = shared.clone();
                    cx.spawn(async move |this: gpui::WeakEntity<TerminalView>, cx| {
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(500))
                                .await;
                            let alive = this.update(cx, |_, cx| {
                                let mut s = shared.borrow_mut();
                                s.blink_on = !s.blink_on;
                                cx.notify();
                            });
                            if alive.is_err() {
                                break;
                            }
                        }
                    })
                    .detach();
                }
                let _ = window;
                TerminalView {
                    session,
                    focus: cx.focus_handle(),
                    shared,
                    rec: Rc::new(RefCell::new(Recorder::default())),
                    dirty,
                    font: gpui::font(common::mono_family()),
                    font_size: px(13.),
                    blink,
                    child_exit: None,
                    selecting: false,
                    scenario: scenario2,
                    steps: Vec::new(),
                    step_ix: 0,
                    step_started: None,
                    results: Vec::new(),
                    phase_usage: None,
                    out,
                    finished: false,
                    started: false,
                    echo_plan: None,
                    resize_query_sent: std::cell::Cell::new(false),
                }
            })
        });
        if let Err(e) = opened {
            eprintln!("term: could not open the window: {e:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}
