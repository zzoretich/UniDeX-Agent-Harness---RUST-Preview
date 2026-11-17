//! `chat`: a virtualised thread of 5,000 generated markdown messages plus a reply that streams in at 200 tokens a
//! second from a Tokio task, with scripted scenarios that measure themselves and write JSON.
//!
//! ```sh
//! cargo run --release -p chat-term-app --bin chat -- --scenario follow --out follow-1.json
//! ```
//!
//! Scenarios (`--scenario`): `none` (just open the window), `coldstart`, `idle`, `scroll`, `follow`, `stream-up`,
//! `resize`. Other flags: `--messages 5000`, `--seed 42`, `--rate 200`, `--seconds 30`, `--out file.json`,
//! `--renderer plain|kit` (`kit` needs the `kit` cargo feature).
//!
//! Architecture, as the design asks (section 5.10): the token producer runs on a Tokio runtime and sends tokens over a
//! `tokio::sync::mpsc` channel; a GPUI foreground task awaits the channel and applies each batch to the view, so the UI
//! thread never waits on I/O. The thread is GPUI's `list` element (variable heights, only visible rows are laid out).
//! Each message's markdown is parsed on first render and the result is kept; the streaming reply is parsed
//! incrementally (`chat_term_core::md::StreamDoc`).

#[path = "../common.rs"]
mod common;

use chat_term_core::content;
use chat_term_core::md::{self, Block, Inline, StreamDoc};
use chat_term_core::stats::{self, round3};
use common::{
    Probe, Recorder, SharedRecorder, UsageWindow, arg, build_profile, ms,
    wall_ms_since_process_start,
};
use gpui::{
    AnyElement, App, Bounds, Context, FollowMode, Font, FontStyle, FontWeight, Hsla, ListAlignment,
    ListOffset, ListState, Render, SharedString, StyledText, TextRun, TitlebarOptions,
    UnderlineStyle, Window, WindowBounds, WindowOptions, div, list, prelude::*, px, rgb, size,
};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

// ---------------------------------------------------------------------------------------------------------------------
// Render-ready blocks: the parsed markdown turned into shared strings and text runs once, then reused every frame.

#[derive(Clone)]
struct Fonts {
    base: Font,
    mono: Font,
    text: Hsla,
    muted: Hsla,
    link: Hsla,
    code_bg: Hsla,
}

impl Fonts {
    fn styled(&self, style: u8) -> Font {
        let mut f = if style & md::CODE != 0 {
            self.mono.clone()
        } else {
            self.base.clone()
        };
        if style & md::BOLD != 0 {
            f.weight = FontWeight::BOLD;
        }
        if style & md::ITALIC != 0 {
            f.style = FontStyle::Italic;
        }
        f
    }

    fn run(&self, len: usize, style: u8) -> TextRun {
        TextRun {
            len,
            font: self.styled(style),
            color: if style & md::LINK != 0 {
                self.link
            } else {
                self.text
            },
            background_color: (style & md::CODE != 0).then_some(self.code_bg),
            underline: (style & md::LINK != 0).then_some(UnderlineStyle {
                color: Some(self.link),
                thickness: px(1.),
                wavy: false,
            }),
            strikethrough: None,
        }
    }
}

#[derive(Clone)]
enum RBlock {
    Text {
        text: SharedString,
        runs: Vec<TextRun>,
        kind: TextKind,
    },
    Code {
        text: SharedString,
    },
    Table {
        rows: Vec<Vec<(SharedString, Vec<TextRun>)>>,
    },
    Rule,
}

#[derive(Clone)]
enum TextKind {
    Para,
    Heading,
    Item { depth: u8, marker: SharedString },
    Quote,
}

fn runs_for(inline: &Inline, fonts: &Fonts) -> Vec<TextRun> {
    let mut runs = Vec::new();
    let mut at = 0;
    for span in &inline.spans {
        if span.start > at {
            runs.push(fonts.run(span.start - at, 0));
        }
        runs.push(fonts.run(span.end - span.start, span.style));
        at = span.end;
    }
    if inline.text.len() > at || runs.is_empty() {
        runs.push(fonts.run(inline.text.len() - at, 0));
    }
    runs
}

fn text_block(inline: &Inline, kind: TextKind, fonts: &Fonts) -> RBlock {
    RBlock::Text {
        text: SharedString::from(inline.text.clone()),
        runs: runs_for(inline, fonts),
        kind,
    }
}

fn prepare(blocks: &[Block], fonts: &Fonts) -> Vec<RBlock> {
    blocks
        .iter()
        .map(|b| match b {
            Block::Heading(_, i) => text_block(i, TextKind::Heading, fonts),
            Block::Paragraph(i) => text_block(i, TextKind::Para, fonts),
            Block::Quote(i) => text_block(i, TextKind::Quote, fonts),
            Block::Item {
                depth,
                marker,
                body,
            } => text_block(
                body,
                TextKind::Item {
                    depth: *depth,
                    marker: marker.clone().into(),
                },
                fonts,
            ),
            Block::Code { text, .. } => RBlock::Code {
                text: text.clone().into(),
            },
            Block::Table(rows) => RBlock::Table {
                rows: rows
                    .iter()
                    .map(|r| {
                        r.iter()
                            .map(|c| (SharedString::from(c.text.clone()), runs_for(c, fonts)))
                            .collect()
                    })
                    .collect(),
            },
            Block::Rule => RBlock::Rule,
        })
        .collect()
}

fn render_block(block: &RBlock, fonts: &Fonts) -> AnyElement {
    match block {
        RBlock::Text { text, runs, kind } => {
            let body = StyledText::new(text.clone()).with_runs(runs.clone());
            match kind {
                TextKind::Para => div().child(body).into_any_element(),
                TextKind::Heading => div().text_size(px(18.)).child(body).into_any_element(),
                TextKind::Quote => div()
                    .border_l_2()
                    .border_color(fonts.muted)
                    .pl_2()
                    .child(body)
                    .into_any_element(),
                TextKind::Item { depth, marker } => div()
                    .flex()
                    .gap_2()
                    .pl(px(16. * *depth as f32))
                    .child(div().w(px(22.)).flex_none().child(marker.clone()))
                    .child(div().flex_1().min_w_0().child(body))
                    .into_any_element(),
            }
        }
        RBlock::Code { text } => div()
            .bg(fonts.code_bg)
            .rounded_md()
            .p_2()
            .font(fonts.mono.clone())
            .text_size(px(12.))
            .child(text.clone())
            .into_any_element(),
        RBlock::Table { rows } => div()
            .flex()
            .flex_col()
            .border_1()
            .border_color(fonts.muted)
            .children(rows.iter().enumerate().map(|(r, row)| {
                div()
                    .flex()
                    .when(r == 0, |d| d.bg(fonts.code_bg))
                    .children(row.iter().map(|(text, runs)| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .px_2()
                            .py_1()
                            .border_1()
                            .border_color(fonts.muted)
                            .child(StyledText::new(text.clone()).with_runs(runs.clone()))
                    }))
            }))
            .into_any_element(),
        RBlock::Rule => div().h(px(1.)).bg(fonts.muted).into_any_element(),
    }
}

fn render_message(from_user: bool, parts: &[&[RBlock]], fonts: &Fonts) -> AnyElement {
    div()
        .w_full()
        .px_4()
        .py_1p5()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .rounded_lg()
                .bg(if from_user {
                    rgb(0xe8f0fe)
                } else {
                    rgb(0xffffff)
                })
                .children(
                    parts
                        .iter()
                        .flat_map(|p| p.iter())
                        .map(|b| render_block(b, fonts)),
                ),
        )
        .into_any_element()
}

// ---------------------------------------------------------------------------------------------------------------------
// The thread and the stream.

struct ThreadMsg {
    from_user: bool,
    text: String,
    /// The same text as a shared string, handed to the kit's TextView every frame (it recognises the allocation).
    #[cfg_attr(not(feature = "kit"), allow(dead_code))]
    shared: SharedString,
    prepared: RefCell<Option<Rc<Vec<RBlock>>>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Renderer {
    Plain,
    #[cfg(feature = "kit")]
    Kit,
}

#[cfg(feature = "kit")]
fn render_message_kit(ix: usize, from_user: bool, body: gpui_base::text::TextView) -> AnyElement {
    let _ = ix;
    div()
        .w_full()
        .px_4()
        .py_1p5()
        .child(
            div()
                .p_3()
                .rounded_lg()
                .bg(if from_user {
                    rgb(0xe8f0fe)
                } else {
                    rgb(0xffffff)
                })
                .child(body),
        )
        .into_any_element()
}

struct Token {
    seq: u64,
    text: String,
    sent: Instant,
}

#[derive(Default)]
struct TokenLog {
    /// Tokens applied to the view but not yet seen in a painted frame: (seq, sent, received).
    pending: VecDeque<(u64, Instant, Instant)>,
    /// Byte offset in the streamed text where each pending token ends (same order as `pending`).
    ends: VecDeque<(u64, usize)>,
    applied_seq: Option<u64>,
    rendered_seq: Option<u64>,
    sent_to_recv: Vec<f64>,
    recv_to_paint: Vec<f64>,
    sent_to_paint: Vec<f64>,
    parse_ms: Vec<f64>,
    apply_ms: Vec<f64>,
    batches: u64,
    count: u64,
}

impl TokenLog {
    fn reset(&mut self) {
        let keep_seq = self.applied_seq;
        *self = TokenLog {
            applied_seq: keep_seq,
            rendered_seq: keep_seq,
            ..Default::default()
        };
    }

    fn summary(&self) -> Value {
        json!({
            "tokens": self.count,
            "batches": self.batches,
            "channel_delay_sent_to_received": stats::summary_ms(&self.sent_to_recv),
            "received_to_frame_painted": stats::summary_ms(&self.recv_to_paint),
            "sent_to_frame_painted": stats::summary_ms(&self.sent_to_paint),
            "incremental_parse_per_token": stats::summary_ms(&self.parse_ms),
            "apply_per_batch": stats::summary_ms(&self.apply_ms),
        })
    }
}

async fn produce(
    tx: tokio::sync::mpsc::UnboundedSender<Token>,
    tokens: Arc<Vec<String>>,
    rate: f64,
    stop: Arc<AtomicBool>,
) {
    let start = tokio::time::Instant::now();
    let period = Duration::from_secs_f64(1.0 / rate);
    for (i, text) in tokens.iter().cycle().enumerate() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        tokio::time::sleep_until(start + period.mul_f64(i as f64)).await;
        if tx
            .send(Token {
                seq: i as u64,
                text: text.clone(),
                sent: Instant::now(),
            })
            .is_err()
        {
            break;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Scenarios.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ScenarioKind {
    None,
    ColdStart,
    Idle,
    Scroll,
    Follow,
    StreamUp,
    Resize,
}

enum Phase {
    Settle {
        until: Instant,
    },
    Idle {
        seconds: u64,
    },
    ScrollSteady {
        until: Instant,
        speed: f32,
        start_fraction: f32,
        warm: bool,
    },
    ScrollToEnd {
        speed: f32,
        deadline: Instant,
        last_ix: usize,
        stalled: u32,
    },
    Jumps {
        left: u32,
        hold: u32,
        rng: content::Rng,
    },
    Stream {
        until: Instant,
        resize: bool,
        started: Instant,
    },
}

impl Phase {
    fn name(&self) -> &'static str {
        match self {
            Phase::Settle { .. } => "settle",
            Phase::Idle { .. } => "idle",
            Phase::ScrollSteady { warm: false, .. } => "scroll_2000px_per_s_from_top_first_visit",
            Phase::ScrollSteady { warm: true, .. } => "scroll_2000px_per_s_from_45pct_second_visit",
            Phase::ScrollToEnd { .. } => "scroll_top_to_bottom_30000px_per_s",
            Phase::Jumps { .. } => "random_jumps",
            Phase::Stream { resize: true, .. } => "stream_while_resizing",
            Phase::Stream { .. } => "stream",
        }
    }
}

struct Startup {
    t_main: Instant,
    wall_main: SystemTime,
    generated_ms: f64,
    app_running: Option<Instant>,
    view_created: Option<Instant>,
    window_opened: Option<Instant>,
    first_render: Option<Instant>,
    first_paint: Option<Instant>,
    signal_after_first_paint: Option<Instant>,
}

impl Startup {
    fn wall(&self, at: Instant) -> SystemTime {
        self.wall_main + (at - self.t_main)
    }

    fn json(&self) -> Value {
        let since_start =
            |at: Option<Instant>| at.and_then(|a| wall_ms_since_process_start(self.wall(a)));
        let since_main = |at: Option<Instant>| at.map(|a| ms(a - self.t_main));
        json!({
            "process_start_to_main_ms": wall_ms_since_process_start(self.wall_main),
            "thread_generated_ms_after_main": round3(self.generated_ms),
            "app_running": {"since_process_start_ms": since_start(self.app_running), "since_main_ms": since_main(self.app_running)},
            "view_created": {"since_process_start_ms": since_start(self.view_created), "since_main_ms": since_main(self.view_created)},
            "window_opened": {"since_process_start_ms": since_start(self.window_opened), "since_main_ms": since_main(self.window_opened)},
            "first_render": {"since_process_start_ms": since_start(self.first_render), "since_main_ms": since_main(self.first_render)},
            "first_frame_painted": {"since_process_start_ms": since_start(self.first_paint), "since_main_ms": since_main(self.first_paint)},
            "next_frame_signal_after_first_paint": {"since_process_start_ms": since_start(self.signal_after_first_paint), "since_main_ms": since_main(self.signal_after_first_paint)},
        })
    }
}

struct ChatView {
    msgs: Rc<Vec<ThreadMsg>>,
    stream: StreamDoc,
    stream_prepared_len: usize,
    stream_stable: Rc<Vec<RBlock>>,
    stream_tail: Rc<Vec<RBlock>>,
    list: ListState,
    fonts: Rc<Fonts>,
    rec: SharedRecorder,
    tokens: Rc<RefCell<TokenLog>>,
    startup: Rc<RefCell<Startup>>,
    kind: ScenarioKind,
    phases: VecDeque<Phase>,
    phase: Option<Phase>,
    phase_usage: Option<UsageWindow>,
    #[cfg(feature = "profiler")]
    collector: Option<gpui::FrameTimingCollector>,
    results: Vec<Value>,
    out: Option<String>,
    rt: tokio::runtime::Runtime,
    tx: tokio::sync::mpsc::UnboundedSender<Token>,
    stream_tokens: Arc<Vec<String>>,
    rate: f64,
    stop: Option<Arc<AtomicBool>>,
    driving: bool,
    started: bool,
    finished: bool,
    base_size: (f32, f32),
    renderer: Renderer,
    #[cfg(feature = "kit")]
    kit_stream: Option<gpui::Entity<gpui_base::text::TextViewState>>,
}

const OVERDRAW: f32 = 1000.;

impl ChatView {
    fn item_count(&self) -> usize {
        self.msgs.len() + 1
    }

    fn on_tokens(&mut self, batch: Vec<Token>, recv: Instant, cx: &mut Context<Self>) {
        let apply_start = Instant::now();
        {
            let mut log = self.tokens.borrow_mut();
            log.batches += 1;
            for t in &batch {
                self.stream.append(&t.text);
                log.parse_ms.push(self.stream.last_parse_ns as f64 / 1e6);
                log.sent_to_recv
                    .push((recv - t.sent).as_secs_f64() * 1000.0);
                log.pending.push_back((t.seq, t.sent, recv));
                log.ends.push_back((t.seq, self.stream.source().len()));
                log.applied_seq = Some(t.seq);
                log.count += 1;
            }
        }
        #[cfg(feature = "kit")]
        if let Some(state) = self.kit_stream.clone() {
            let text: String = batch.iter().map(|t| t.text.as_str()).collect();
            state.update(cx, |s, cx| s.push_str(&text, cx));
        }
        let stable = self.stream.stable();
        if stable.len() > self.stream_prepared_len {
            let mut v = (*self.stream_stable).clone();
            v.extend(prepare(&stable[self.stream_prepared_len..], &self.fonts));
            self.stream_prepared_len = stable.len();
            self.stream_stable = Rc::new(v);
        }
        self.stream_tail = Rc::new(prepare(self.stream.tail(), &self.fonts));
        let last = self.item_count() - 1;
        self.list.remeasure_items(last..last + 1);
        self.tokens
            .borrow_mut()
            .apply_ms
            .push(apply_start.elapsed().as_secs_f64() * 1000.0);
        cx.notify();
    }

    fn start_scenario(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        let secs = |s: f64| Duration::from_secs_f64(s);
        let stream_secs = self.rate_seconds();
        self.phases = match self.kind {
            ScenarioKind::None | ScenarioKind::ColdStart => VecDeque::new(),
            ScenarioKind::Idle => [
                Phase::Settle {
                    until: now + secs(2.),
                },
                Phase::Idle {
                    seconds: stream_secs as u64,
                },
            ]
            .into(),
            ScenarioKind::Scroll => {
                self.list.set_follow_mode(FollowMode::Normal);
                self.list.scroll_to(ListOffset {
                    item_ix: 0,
                    offset_in_item: px(0.),
                });
                [
                    Phase::Settle {
                        until: now + secs(1.),
                    },
                    Phase::ScrollSteady {
                        until: Instant::now(),
                        speed: 2000.,
                        start_fraction: 0.,
                        warm: false,
                    },
                    Phase::ScrollToEnd {
                        speed: 30000.,
                        deadline: Instant::now(),
                        last_ix: 0,
                        stalled: 0,
                    },
                    Phase::ScrollSteady {
                        until: Instant::now(),
                        speed: 2000.,
                        start_fraction: 0.45,
                        warm: true,
                    },
                    Phase::Jumps {
                        left: 60,
                        hold: 0,
                        rng: content::Rng::new(7),
                    },
                ]
                .into()
            }
            ScenarioKind::Follow => [
                Phase::Settle {
                    until: now + secs(1.),
                },
                Phase::Stream {
                    until: now,
                    resize: false,
                    started: now,
                },
            ]
            .into(),
            ScenarioKind::StreamUp => {
                self.list.set_follow_mode(FollowMode::Normal);
                let ix = self.item_count().saturating_sub(1001);
                self.list.scroll_to(ListOffset {
                    item_ix: ix,
                    offset_in_item: px(0.),
                });
                [
                    Phase::Settle {
                        until: now + secs(1.),
                    },
                    Phase::Stream {
                        until: now,
                        resize: false,
                        started: now,
                    },
                ]
                .into()
            }
            ScenarioKind::Resize => [
                Phase::Settle {
                    until: now + secs(1.),
                },
                Phase::Stream {
                    until: now,
                    resize: true,
                    started: now,
                },
            ]
            .into(),
        };
        self.next_phase(window, cx);
    }

    fn rate_seconds(&self) -> f64 {
        std::env::args()
            .collect::<Vec<_>>()
            .iter()
            .position(|a| a == "--seconds")
            .and_then(|i| std::env::args().nth(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(30.)
    }

    fn next_phase(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut phase) = self.phases.pop_front() else {
            self.finish(window, cx);
            return;
        };
        let now = Instant::now();
        let seconds = self.rate_seconds();
        match &mut phase {
            Phase::ScrollSteady {
                until,
                start_fraction,
                ..
            } => {
                *until = now + Duration::from_secs(10);
                let ix = (*start_fraction * self.item_count() as f32) as usize;
                self.list.scroll_to(ListOffset {
                    item_ix: ix,
                    offset_in_item: px(0.),
                });
                cx.notify();
            }
            Phase::ScrollToEnd { deadline, .. } => {
                *deadline = now + Duration::from_secs(120);
                self.list.scroll_to(ListOffset {
                    item_ix: 0,
                    offset_in_item: px(0.),
                });
                cx.notify();
            }
            Phase::Stream { until, started, .. } => {
                *until = now + Duration::from_secs_f64(seconds);
                *started = now;
                let stop = Arc::new(AtomicBool::new(false));
                self.rt.spawn(produce(
                    self.tx.clone(),
                    self.stream_tokens.clone(),
                    self.rate,
                    stop.clone(),
                ));
                self.stop = Some(stop);
            }
            _ => {}
        }
        self.rec.borrow_mut().begin_window();
        self.tokens.borrow_mut().reset();
        self.phase_usage = Some(UsageWindow::begin());
        #[cfg(feature = "profiler")]
        {
            gpui::set_trace_enabled(true);
            self.collector = Some(gpui::FrameTimingCollector::new());
        }
        let is_idle = matches!(phase, Phase::Idle { .. });
        if let Phase::Idle { seconds } = phase {
            // No driver while idle: a single timer ends the phase, so the app does nothing for `seconds`.
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_secs(seconds))
                    .await;
                let _ = this.update_in(cx, |view, window, cx| view.end_phase(window, cx));
            })
            .detach();
        }
        self.phase = Some(phase);
        if !is_idle && !self.driving {
            self.driving = true;
            self.schedule_drive(window, cx);
        }
    }

    fn schedule_drive(&self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity();
        window.on_next_frame(move |window, cx| {
            this.update(cx, |view, cx| view.drive(window, cx));
        });
    }

    /// Called once per frame signal while a driven phase runs.
    fn drive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        self.rec.borrow_mut().signals.push(now);
        let dt = {
            let rec = self.rec.borrow();
            let n = rec.signals.len();
            if n >= 2 {
                (rec.signals[n - 1] - rec.signals[n - 2]).as_secs_f32()
            } else {
                0.
            }
        };
        let mut done = false;
        let mut notify = false;
        let item_count = self.item_count();
        match self.phase.as_mut() {
            Some(Phase::Settle { until }) => done = now >= *until,
            Some(Phase::ScrollSteady { until, speed, .. }) => {
                self.list.scroll_by(px(*speed * dt.min(0.05)));
                notify = true;
                done = now >= *until;
            }
            Some(Phase::ScrollToEnd {
                speed,
                deadline,
                last_ix,
                stalled,
            }) => {
                self.list.scroll_by(px(*speed * dt.min(0.05)));
                notify = true;
                let ix = self.list.logical_scroll_top().item_ix;
                if ix == *last_ix {
                    *stalled += 1;
                } else {
                    *stalled = 0;
                    *last_ix = ix;
                }
                done = ix + 3 >= item_count || *stalled > 30 || now >= *deadline;
            }
            Some(Phase::Jumps { left, hold, rng }) => {
                // A jump every tenth frame; the frames in between are redrawn too (as during continuous
                // interaction), so the interval after a jump shows whether the jump frame was late.
                if *hold == 0 {
                    let ix = rng.range(0, item_count - 1);
                    self.list.scroll_to(ListOffset {
                        item_ix: ix,
                        offset_in_item: px(0.),
                    });
                    self.rec.borrow_mut().mark_next();
                    *left -= 1;
                    *hold = 10;
                } else {
                    *hold -= 1;
                }
                notify = true;
                done = *left == 0 && *hold == 0;
            }
            Some(Phase::Stream {
                until,
                resize,
                started,
            }) => {
                if *resize {
                    let t = (now - *started).as_secs_f32();
                    let w = self.base_size.0 + 300. * (t * std::f32::consts::PI).sin();
                    window.resize(size(px(w), px(self.base_size.1)));
                }
                done = now >= *until;
            }
            Some(Phase::Idle { .. }) | None => {}
        }
        if notify {
            cx.notify();
        }
        if done {
            self.end_phase(window, cx);
        }
        if self.phase.is_some() && !matches!(self.phase, Some(Phase::Idle { .. })) {
            self.schedule_drive(window, cx);
        } else {
            self.driving = false;
        }
    }

    fn end_phase(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(phase) = self.phase.take() else {
            return;
        };
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        let refresh = window
            .refresh_interval(cx)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(16.667);
        let rec = self.rec.borrow();
        let mut result = json!({
            "phase": phase.name(),
            "frames": rec.frame_stats(refresh),
            "process": self.phase_usage.take().map(|u| u.end()),
            "renders": rec.renders,
            "scroll_top_item": self.list.logical_scroll_top().item_ix,
        });
        if matches!(phase, Phase::Jumps { .. }) {
            result["jump_frames"] = rec.marked_stats(refresh);
        }
        drop(rec);
        if matches!(phase, Phase::Stream { .. }) {
            result["tokens"] = self.tokens.borrow().summary();
            result["stream_bytes"] = json!(self.stream.source().len());
            result["stream_blocks"] = json!(self.stream.block_count());
        }
        #[cfg(feature = "profiler")]
        if let Some(mut collector) = self.collector.take() {
            result["profiler"] = profiler_summary(collector.collect_unseen(), refresh);
        }
        if !matches!(phase, Phase::Settle { .. }) {
            println!("chat: phase {} done", phase.name());
            self.results.push(result);
        }
        self.next_phase(window, cx);
    }

    fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.finished {
            return;
        }
        self.finished = true;
        let doc = json!({
            "scenario": format!("{:?}", self.kind),
            "renderer": format!("{:?}", self.renderer),
            "build_profile": build_profile(),
            "os": std::env::consts::OS,
            "messages": self.msgs.len(),
            "stream_rate_tokens_per_s": self.rate,
            "window": common::window_facts(window, cx),
            "startup": self.startup.borrow().json(),
            "phases": self.results,
            "process_at_exit": common::process_snapshot(),
        });
        common::write_json(self.out.as_deref(), &doc);
        cx.quit();
    }
}

#[cfg(feature = "profiler")]
fn profiler_summary(events: Vec<gpui::FrameEvent>, refresh_ms: f64) -> Value {
    let mut layout = Vec::new();
    let mut prepaint = Vec::new();
    let mut paint = Vec::new();
    let mut draw = Vec::new();
    let mut present = Vec::new();
    let mut present_starts = Vec::new();
    for e in events {
        match e {
            gpui::FrameEvent::Draw(t) => {
                layout.push(t.phases.request_layout.as_secs_f64() * 1000.0);
                prepaint.push(t.phases.prepaint.as_secs_f64() * 1000.0);
                paint.push(t.phases.paint.as_secs_f64() * 1000.0);
                draw.push(t.draw_duration().as_secs_f64() * 1000.0);
            }
            gpui::FrameEvent::Present(p) => {
                present.push(p.present_duration().as_secs_f64() * 1000.0);
                present_starts.push(p.present_start);
            }
        }
    }
    let intervals: Vec<f64> = present_starts
        .windows(2)
        .map(|w| (w[1] - w[0]).as_secs_f64() * 1000.0)
        .collect();
    json!({
        "request_layout": stats::summary_ms(&layout),
        "prepaint": stats::summary_ms(&prepaint),
        "paint": stats::summary_ms(&paint),
        "draw": stats::summary_ms(&draw),
        "present_call": stats::summary_ms(&present),
        "present_intervals": stats::frame_summary(&intervals, refresh_ms),
    })
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        {
            let mut rec = self.rec.borrow_mut();
            rec.render_start = Some(now);
            rec.renders += 1;
        }
        {
            let mut startup = self.startup.borrow_mut();
            if startup.first_render.is_none() {
                startup.first_render = Some(now);
            }
        }
        {
            let mut log = self.tokens.borrow_mut();
            log.rendered_seq = log.applied_seq;
            // The kit parses on a background thread: a token is on screen only once the parse that covers it lands.
            #[cfg(feature = "kit")]
            if let Some(state) = self.kit_stream.as_ref() {
                let landed = state.read(cx).rendered_text().source().len();
                log.rendered_seq = log
                    .ends
                    .iter()
                    .take_while(|(_, end)| *end <= landed)
                    .last()
                    .map(|(seq, _)| *seq);
            }
        }
        if !self.started {
            self.started = true;
            let this = cx.entity();
            // Start the scenario after the first frame.
            window.on_next_frame(move |window, cx| {
                this.update(cx, |view, cx| view.after_first_frame(window, cx));
            });
        }

        let msgs = self.msgs.clone();
        let fonts = self.fonts.clone();
        let stable = self.stream_stable.clone();
        let tail = self.stream_tail.clone();
        let rec = self.rec.clone();
        let tokens = self.tokens.clone();
        let startup = self.startup.clone();
        let streaming_ix = msgs.len();
        let renderer = self.renderer;
        #[cfg(feature = "kit")]
        let kit_stream = self.kit_stream.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf4f4f5))
            .text_color(rgb(0x18181b))
            .text_size(px(14.))
            .child(
                list(self.list.clone(), move |ix, _window, _cx| {
                    #[cfg(feature = "kit")]
                    if renderer == Renderer::Kit {
                        use gpui_base::text::TextView;
                        if ix == streaming_ix {
                            let body = match kit_stream.as_ref() {
                                Some(state) => TextView::new(state),
                                None => TextView::markdown("stream", ""),
                            };
                            return render_message_kit(ix, false, body);
                        }
                        let m = &msgs[ix];
                        let body = TextView::markdown(
                            gpui::ElementId::NamedInteger("msg".into(), ix as u64),
                            m.shared.clone(),
                        );
                        return render_message_kit(ix, m.from_user, body);
                    }
                    let _ = renderer;
                    if ix == streaming_ix {
                        return render_message(false, &[&stable, &tail], &fonts);
                    }
                    let m = &msgs[ix];
                    let prepared = m
                        .prepared
                        .borrow_mut()
                        .get_or_insert_with(|| Rc::new(prepare(&md::parse(&m.text), &fonts)))
                        .clone();
                    render_message(m.from_user, &[&prepared], &fonts)
                })
                .flex_1()
                .size_full(),
            )
            .child(Probe::new(move |_window| {
                let paint_end = Instant::now();
                let mut rec = rec.borrow_mut();
                let render_start = rec.render_start.unwrap_or(paint_end);
                rec.frames.push(common::Frame {
                    render_start,
                    paint_end,
                });
                let mut startup = startup.borrow_mut();
                if startup.first_paint.is_none() {
                    startup.first_paint = Some(paint_end);
                }
                let mut log = tokens.borrow_mut();
                if let Some(seen) = log.rendered_seq {
                    while log.pending.front().is_some_and(|(seq, _, _)| *seq <= seen) {
                        let (_, sent, recv) = log.pending.pop_front().unwrap();
                        log.ends.pop_front();
                        log.recv_to_paint
                            .push((paint_end - recv).as_secs_f64() * 1000.0);
                        log.sent_to_paint
                            .push((paint_end - sent).as_secs_f64() * 1000.0);
                    }
                }
            }))
    }
}

impl ChatView {
    fn after_first_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.startup.borrow_mut().signal_after_first_paint = Some(Instant::now());
        if self.kind == ScenarioKind::ColdStart {
            self.finish(window, cx);
            return;
        }
        self.start_scenario(window, cx);
    }
}

/// No window: parse and prepare every message and the streamed reply, and check that each block's text runs cover its
/// text exactly on character boundaries (GPUI's `StyledText::with_runs` panics otherwise). CI runs this.
fn selftest(seed: u64, count: usize) {
    let fonts = Fonts {
        base: gpui::font("sans-serif"),
        mono: gpui::font(common::mono_family()),
        text: rgb(0x18181b).into(),
        muted: rgb(0xa1a1aa).into(),
        link: rgb(0x2563eb).into(),
        code_bg: rgb(0xf1f1f3).into(),
    };
    let started = Instant::now();
    let thread = content::thread(seed, count);
    let mut blocks = 0usize;
    let mut bad = Vec::new();
    let mut check = |text: &str, runs: &[TextRun], at: String| {
        let mut offset = 0;
        for r in runs {
            offset += r.len;
            if !text.is_char_boundary(offset.min(text.len())) {
                bad.push(format!("{at}: run ends inside a character"));
                return;
            }
        }
        if offset != text.len() {
            bad.push(format!("{at}: runs cover {offset} of {} bytes", text.len()));
        }
    };
    let mut all: Vec<(String, Vec<RBlock>)> = thread
        .iter()
        .enumerate()
        .map(|(i, m)| (format!("message {i}"), prepare(&md::parse(&m.text), &fonts)))
        .collect();
    let mut doc = StreamDoc::new();
    for t in content::stream_tokens(seed, 60_000) {
        doc.append(&t);
    }
    let streamed: Vec<Block> = doc.blocks().cloned().collect();
    all.push(("streamed reply".into(), prepare(&streamed, &fonts)));
    for (name, rblocks) in &all {
        for b in rblocks {
            blocks += 1;
            match b {
                RBlock::Text { text, runs, .. } => check(text, runs, name.clone()),
                RBlock::Table { rows } => rows
                    .iter()
                    .flatten()
                    .for_each(|(t, r)| check(t, r, name.clone())),
                _ => {}
            }
        }
    }
    let ok = bad.is_empty();
    println!(
        "chat --selftest: {} ({} messages, {} blocks prepared in {:.1} ms; {})",
        if ok { "PASS" } else { "FAIL" },
        all.len(),
        blocks,
        started.elapsed().as_secs_f64() * 1000.0,
        bad.iter().take(5).cloned().collect::<Vec<_>>().join("; ")
    );
    std::process::exit(if ok { 0 } else { 1 });
}

fn main() {
    let t_main = Instant::now();
    let wall_main = SystemTime::now();
    let args: Vec<String> = std::env::args().collect();
    let kind = match arg(&args, "--scenario").as_deref().unwrap_or("none") {
        "coldstart" => ScenarioKind::ColdStart,
        "idle" => ScenarioKind::Idle,
        "scroll" => ScenarioKind::Scroll,
        "follow" => ScenarioKind::Follow,
        "stream-up" => ScenarioKind::StreamUp,
        "resize" => ScenarioKind::Resize,
        _ => ScenarioKind::None,
    };
    let count: usize = arg(&args, "--messages")
        .and_then(|s| s.parse().ok())
        .unwrap_or(5000);
    let seed: u64 = arg(&args, "--seed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(42);
    let rate: f64 = arg(&args, "--rate")
        .and_then(|s| s.parse().ok())
        .unwrap_or(200.);
    let out = arg(&args, "--out");
    let renderer = match arg(&args, "--renderer").as_deref() {
        #[cfg(feature = "kit")]
        Some("kit") => Renderer::Kit,
        Some("plain") | None => Renderer::Plain,
        Some(other) => panic!("unknown renderer {other} (the kit renderer needs --features kit)"),
    };

    if args.iter().any(|a| a == "--selftest") {
        selftest(seed, count);
        return;
    }
    let thread = content::thread(seed, count);
    let stream_tokens = Arc::new(content::stream_tokens(seed, 60_000));
    let generated_ms = t_main.elapsed().as_secs_f64() * 1000.0;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()
        .expect("tokio");

    gpui_platform::application().run(move |cx: &mut App| {
        #[cfg(feature = "kit")]
        gpui_base::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let base_size = (1100f32, 900f32);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(base_size.0), px(base_size.1)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("chat-term trial: chat".into()),
                ..Default::default()
            }),
            // Measure at the display's rate even if the window loses focus (GPUI's default throttles inactive windows to 30 fps).
            inactive_frame_interval: None,
            ..Default::default()
        };
        let startup = Rc::new(RefCell::new(Startup {
            t_main,
            wall_main,
            generated_ms,
            app_running: Some(Instant::now()),
            view_created: None,
            window_opened: None,
            first_render: None,
            first_paint: None,
            signal_after_first_paint: None,
        }));
        let startup2 = startup.clone();
        let opened = cx.open_window(options, move |window, cx| {
            startup2.borrow_mut().view_created = Some(Instant::now());
            cx.new(|cx| {
                let base = window.text_style().font();
                let fonts = Rc::new(Fonts {
                    base,
                    mono: gpui::font(common::mono_family()),
                    text: rgb(0x18181b).into(),
                    muted: rgb(0xa1a1aa).into(),
                    link: rgb(0x2563eb).into(),
                    code_bg: rgb(0xf1f1f3).into(),
                });
                let msgs: Rc<Vec<ThreadMsg>> = Rc::new(
                    thread
                        .into_iter()
                        .map(|m| ThreadMsg {
                            from_user: m.from_user,
                            shared: SharedString::from(m.text.clone()),
                            text: m.text,
                            prepared: RefCell::new(None),
                        })
                        .collect(),
                );
                let list = ListState::new(msgs.len() + 1, ListAlignment::Top, px(OVERDRAW));
                list.set_follow_mode(FollowMode::Tail);
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Token>();
                cx.spawn(async move |this, cx| {
                    while let Some(first) = rx.recv().await {
                        let recv = Instant::now();
                        let mut batch = vec![first];
                        while let Ok(t) = rx.try_recv() {
                            batch.push(t);
                        }
                        if this
                            .update(cx, |view: &mut ChatView, cx| {
                                view.on_tokens(batch, recv, cx)
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .detach();
                ChatView {
                    msgs,
                    stream: StreamDoc::new(),
                    stream_prepared_len: 0,
                    stream_stable: Rc::new(Vec::new()),
                    stream_tail: Rc::new(Vec::new()),
                    list,
                    fonts,
                    rec: Rc::new(RefCell::new(Recorder::default())),
                    tokens: Rc::new(RefCell::new(TokenLog::default())),
                    startup: startup2,
                    kind,
                    phases: VecDeque::new(),
                    phase: None,
                    phase_usage: None,
                    #[cfg(feature = "profiler")]
                    collector: None,
                    results: Vec::new(),
                    out,
                    rt,
                    tx,
                    stream_tokens,
                    rate,
                    stop: None,
                    driving: false,
                    started: false,
                    finished: false,
                    base_size,
                    renderer,
                    #[cfg(feature = "kit")]
                    kit_stream: (renderer == Renderer::Kit)
                        .then(|| cx.new(|cx| gpui_base::text::TextViewState::markdown("", cx))),
                }
            })
        });
        match opened {
            Ok(_) => startup.borrow_mut().window_opened = Some(Instant::now()),
            Err(e) => {
                eprintln!("chat: could not open the window: {e:#}");
                cx.quit();
            }
        }
        cx.activate(true);
    });
}
