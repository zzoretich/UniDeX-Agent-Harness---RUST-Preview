//! A virtualised list of 5,000 messages of different heights, a reply streamed into its last row from another thread,
//! and the frame and token timing the chat trial used (decision 0008).
//!
//! Run it with `cargo run -p udx-cookbook --example streaming_list --release` (timings mean little in a debug build).
//! The reply streams for 30 seconds at 200 tokens a second; then the status line and stderr show the token delay
//! (sent to frame painted) and the paint intervals against the display's refresh interval.
//!
//! - GPUI's `list`: only rows in view plus `OVERDRAW` are laid out, rows may have any height, and `FollowMode::Tail`
//!   keeps the end in view while the user is at the bottom. `uniform_list` needs one fixed row height; measuring every
//!   row up front is the start-up cost the Electron app paid.
//! - A row's text and runs are prepared when it is first drawn and kept. The trial kept every prepared row, so memory
//!   grows with each message seen; the real chat needs eviction.
//! - Tokens come from another thread (a Tokio task in the application) over a channel. A foreground task awaits the
//!   first, drains the rest, applies the batch in one update and re-measures only the streaming row, so the UI thread
//!   never waits and a burst costs one frame. The view is notified once per drained batch, not once per token.
//! - Timing without the profiler: `render` marks the start of a draw and a zero-size `canvas` painted last marks the
//!   end of its paint phase (`udx_cookbook::stats`).
//!
//! The trap the trial measured: streaming into an existing row cost about 7 ms at the median and 13 ms at p99 from
//! send to paint, but scrolling into rows never drawn before gave slow frames, because GPUI lays out a row and shapes
//! its text in prepaint when it first enters the view, and keeps shaped text only from one frame to the next. Slow
//! frames come from text new to the view, not from the stream.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Bounds, Context, FollowMode, Font, FontWeight, ListAlignment, ListState, Role,
    SharedString, StyledText, TextRun, TitlebarOptions, Window, WindowBounds, WindowOptions,
    canvas, div, list, prelude::*, px, rgb, size, text,
};
use udx_cookbook::channel;
use udx_cookbook::stats::{FrameRecorder, TokenLatency};

const MESSAGES: usize = 5_000;
/// How far beyond the visible area rows are laid out, in pixels (the trial's value).
const OVERDRAW: f32 = 1_000.;
const TOKENS_PER_SECOND: u32 = 200;
const STREAM_TOKENS: usize = 6_000;

const SENTENCES: [&str; 6] = [
    "The build finished without warnings.",
    "Next, the test suite runs on all three platforms and reports each failure with its log.",
    "短い日本語の文も混ぜておく。",
    "A longer sentence wraps onto a second line in a narrow window, which changes the row's height.",
    "Done 🎉.",
    "Each row is laid out only when it comes into view.",
];

/// A token as the producer sends it, stamped so its delay to the screen can be measured.
struct Token {
    text: &'static str,
    sent: Instant,
}

struct Message {
    from_user: bool,
    text: SharedString,
    /// The body's style runs, built the first time the row is drawn and kept.
    runs: RefCell<Option<Rc<Vec<TextRun>>>>,
}

impl Message {
    fn body(&self, window: &Window) -> StyledText {
        let runs = self
            .runs
            .borrow_mut()
            .get_or_insert_with(|| Rc::new(runs_for(&self.text, window)))
            .clone();
        StyledText::new(self.text.clone()).with_runs(runs.to_vec())
    }
}

/// The first sentence in bold, the rest plain. Runs must cover the text exactly and end on character boundaries:
/// `StyledText::with_runs` panics otherwise.
fn runs_for(text: &str, window: &Window) -> Vec<TextRun> {
    let style = window.text_style();
    let plain = style.font();
    let bold = Font {
        weight: FontWeight::BOLD,
        ..plain.clone()
    };
    let run = |len, font| TextRun {
        len,
        font,
        color: style.color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    // Just after an ASCII full stop is always a character boundary.
    let lead = text.find(". ").map_or(text.len(), |dot| dot + 1);
    let mut runs = vec![run(lead, bold)];
    if lead < text.len() {
        runs.push(run(text.len() - lead, plain));
    }
    runs
}

fn message_text(index: usize) -> SharedString {
    let sentences = 1 + (index * 7 + index / 3) % 14;
    let text: Vec<&str> = (0..sentences)
        .map(|k| SENTENCES[(index + k) % SENTENCES.len()])
        .collect();
    text.join(" ").into()
}

/// The stand-in for the engine: a steady stream of short tokens from its own thread.
fn produce(tx: channel::Sender<Token>) {
    let words: Vec<&'static str> = SENTENCES
        .iter()
        .flat_map(|s| s.split_inclusive(' '))
        .collect();
    let period = Duration::from_secs(1) / TOKENS_PER_SECOND;
    let start = Instant::now();
    for (i, word) in words.iter().cycle().take(STREAM_TOKENS).enumerate() {
        let due = start + period * i as u32;
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        let sent = Instant::now();
        if tx.send(Token { text: word, sent }).is_err() {
            return; // The window is gone.
        }
    }
}

#[derive(Default)]
struct Timing {
    done: bool,
    frames: FrameRecorder,
    tokens: TokenLatency,
}

struct Chat {
    messages: Rc<Vec<Message>>,
    stream: String,
    list: ListState,
    timing: Rc<RefCell<Timing>>,
    status: SharedString,
}

impl Chat {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let messages = (0..MESSAGES)
            .map(|i| Message {
                from_user: i % 2 == 0,
                text: message_text(i),
                runs: RefCell::new(None),
            })
            .collect();
        // One row more than there are messages: the reply that streams in.
        let list = ListState::new(MESSAGES + 1, ListAlignment::Top, px(OVERDRAW));
        list.set_follow_mode(FollowMode::Tail);

        let (tx, rx) = channel::unbounded::<Token>();
        std::thread::spawn(move || produce(tx));
        cx.spawn_in(window, async move |this, cx| {
            while let Some(first) = rx.recv().await {
                let mut batch = vec![first];
                while let Some(more) = rx.try_recv() {
                    batch.push(more);
                }
                if this.update(cx, |chat, cx| chat.apply(batch, cx)).is_err() {
                    return;
                }
            }
            // Every sender is gone: the stream ended.
            let _ = this.update_in(cx, |chat, window, cx| chat.report(window, cx));
        })
        .detach();

        Chat {
            messages: Rc::new(messages),
            stream: String::new(),
            list,
            timing: Rc::default(),
            status: "Streaming a reply into the last row...".into(),
        }
    }

    fn apply(&mut self, batch: Vec<Token>, cx: &mut Context<Self>) {
        let mut timing = self.timing.borrow_mut();
        for token in &batch {
            self.stream.push_str(token.text);
            timing.tokens.applied(token.sent);
        }
        drop(timing);
        // Only the streaming row changed height: re-measure that one row, not the whole list.
        self.list.remeasure_items(MESSAGES..MESSAGES + 1);
        self.status = format!("Streaming: {} bytes received", self.stream.len()).into();
        cx.notify();
    }

    fn report(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let refresh = window
            .refresh_interval(cx)
            .unwrap_or(Duration::from_micros(16_667));
        let mut timing = self.timing.borrow_mut();
        timing.done = true;
        let line = match (timing.frames.summary(refresh), timing.tokens.summary()) {
            (Some(frames), Some(tokens)) => {
                format!(
                    "Stream done. {} tokens sent to painted: {tokens}. {frames}.",
                    tokens.count
                )
            }
            _ => "Stream done; too few frames to measure.".to_string(),
        };
        eprintln!("streaming_list: {line}");
        self.status = line.into();
        cx.notify();
    }
}

fn row(from_user: bool, body: impl IntoElement) -> AnyElement {
    let bubble = rgb(if from_user { 0xe8f0fe } else { 0xffffff });
    div()
        .w_full()
        .px_4()
        .py_1p5()
        .child(div().p_3().rounded_lg().bg(bubble).child(body))
        .into_any_element()
}

impl Render for Chat {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut timing = self.timing.borrow_mut();
        if !timing.done {
            timing.frames.render_started(Instant::now());
            timing.tokens.frame_rendered();
        }
        drop(timing);
        let messages = self.messages.clone();
        let stream = SharedString::from(self.stream.clone());
        let timing = self.timing.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf4f4f5))
            .text_color(rgb(0x18181b))
            .text_size(px(14.))
            .child(
                div()
                    .id("status")
                    .role(Role::Status)
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(0xd4d4d8))
                    .child(text!(self.status.clone())),
            )
            .child(
                // StyledText publishes nothing to screen readers (decision 0010): the real chat's message bodies
                // must publish accessibility nodes of their own.
                list(self.list.clone(), move |ix, window, _cx| {
                    match messages.get(ix) {
                        Some(message) => row(message.from_user, message.body(window)),
                        None => row(false, StyledText::new(stream.clone())),
                    }
                })
                .flex_1()
                .size_full(),
            )
            // Painted last, so this marks the end of the frame's paint phase.
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, _, _| {
                        let at = Instant::now();
                        let mut timing = timing.borrow_mut();
                        if !timing.done {
                            timing.frames.painted(at);
                            timing.tokens.frame_painted(at);
                        }
                    },
                )
                .absolute()
                .size_0(),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(720.), px(800.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("UniDeX cookbook: streaming list".into()),
                ..Default::default()
            }),
            // GPUI draws inactive windows at 30 frames a second by default, which is right for the product. Turned off
            // here, as in the trial, so a window that loses focus does not distort the measurement.
            inactive_frame_interval: None,
            ..Default::default()
        };
        if let Err(error) = cx.open_window(options, |window, cx| cx.new(|cx| Chat::new(window, cx)))
        {
            eprintln!("streaming_list: could not open the window: {error:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}
