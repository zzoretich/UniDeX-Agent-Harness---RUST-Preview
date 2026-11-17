//! The browser trial's GPUI window (Task 12, decision 0012). Throwaway.
//!
//! A window with a toolbar and a browser pane. The pane draws frames that a helper process
//! paints into shared memory, as an element; forwards pointer, wheel, key, text and
//! input-method events to the helper; shows the cursor and popups the helper asks for; and draws
//! a GPUI overlay card (a stand-in permission card) above the page. The helper is the CEF
//! helper (`--helper cef`) or the stand-in test pattern; both speak `spike-protocol`.
//!
//! ```sh
//! spike-app [options]
//!   --helper standin|cef   which helper to start (default standin)
//!   --page NAME            start on pages/NAME.html (CEF; a file URL)
//!   --start-url URL        start on URL (CEF)
//!   --tabs N               N - 1 extra hidden tabs in the helper (CEF), loading --hidden-page
//!   --hidden-page NAME     what the hidden tabs load (default: the start page)
//!   --scenario NAME        input | scroll | anim | idle | latency | memory (see script.rs)
//!   --measure-seconds N    length of a measurement window (default 10)
//!   --width W --height H   requested pane size in logical pixels (default 1280 x 800)
//!   --display N            open the window on the Nth display GPUI lists (logged at start)
//!   --fps N                frame rate the stand-in paints at (default 60)
//!   --seconds N            hard limit: quit after N seconds whatever happens
//!   --quiet-ui             do not redraw the toolbar's stats line every second (idle runs)
//!   --probe-shared-texture ask CEF for shared textures; frames are then only logged
//!   --engine-fps N         CEF's windowless frame rate (default 60)
//!   --demo-popup           the stand-in shows and hides a popup every 1.5 s
//!   --selfcheck            read the window back and check the overlay is drawn above the page
//!   --url URL              navigate there at start (a DevTools call), then evaluate document.title
//! ```
//!
//! Everything it measures goes to stderr: one `app: stats` line per second, `check:` lines from
//! the input scenario and one `app: summary {json}` line per measurement window. The helper's
//! lines start with its own name.

mod script;
mod stats;

#[cfg(target_os = "macos")]
mod mac_ime;

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::{
    App, AppContext as _, Bounds, Context, Corners, CursorStyle, DispatchPhase,
    ElementInputHandler, EntityInputHandler, FocusHandle, HitboxBehavior, InteractiveElement as _,
    IntoElement, KeyDownEvent, KeyUpEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, NavigationDirection, ParentElement as _, Pixels, Point, Render, RenderImage,
    ScrollDelta, ScrollEvent, StatefulInteractiveElement as _, Styled as _, Subscription, Task,
    TitlebarOptions, UTF16Selection, Window, WindowBounds, WindowOptions, canvas, div, fill, point,
    px, rgb, rgba, size,
};
use spike_protocol::helper::{HelperBrowser, HelperOptions};
use spike_protocol::usage;
use spike_protocol::view::{self as proto, BrowserEvent, BrowserView as _};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOOLBAR_HEIGHT: f32 = 44.0;
const SIDEBAR_WIDTH: f32 = 240.0;
/// Shared memory is sized for frames up to this many device pixels.
const MAX_FRAME: (u32, u32) = (3840, 2400);

#[derive(Clone)]
pub struct Args {
    helper: String,
    start_url: Option<String>,
    tabs: u32,
    hidden_url: Option<String>,
    pub scenario: Option<String>,
    pub measure_seconds: u64,
    width: f32,
    height: f32,
    fps: u32,
    seconds: Option<u64>,
    quiet_ui: bool,
    probe_shared_texture: bool,
    engine_fps: Option<u32>,
    demo_popup: bool,
    selfcheck: bool,
    url: Option<String>,
    display: Option<usize>,
}

impl Args {
    fn parse() -> Args {
        let raw: Vec<String> = std::env::args().collect();
        let value = |name: &str| {
            raw.iter()
                .position(|a| a == name)
                .and_then(|i| raw.get(i + 1))
                .cloned()
        };
        let flag = |name: &str| raw.iter().any(|a| a == name);
        let start_url = value("--start-url").or_else(|| value("--page").map(|p| page_url(&p)));
        Args {
            helper: value("--helper").unwrap_or_else(|| "standin".into()),
            hidden_url: value("--hidden-page").map(|p| page_url(&p)),
            start_url,
            tabs: value("--tabs").and_then(|v| v.parse().ok()).unwrap_or(1),
            scenario: value("--scenario"),
            measure_seconds: value("--measure-seconds")
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            width: value("--width")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1280.0),
            height: value("--height")
                .and_then(|v| v.parse().ok())
                .unwrap_or(800.0),
            fps: value("--fps").and_then(|v| v.parse().ok()).unwrap_or(60),
            seconds: value("--seconds").and_then(|v| v.parse().ok()),
            quiet_ui: flag("--quiet-ui"),
            probe_shared_texture: flag("--probe-shared-texture"),
            engine_fps: value("--engine-fps").and_then(|v| v.parse().ok()),
            demo_popup: flag("--demo-popup"),
            selfcheck: flag("--selfcheck"),
            url: value("--url"),
            display: value("--display").and_then(|v| v.parse().ok()),
        }
    }
}

/// `pages/NAME.html` beside the build (target/release/../../pages) as a file URL, or
/// `about:blank` for "blank".
pub fn page_url(name: &str) -> String {
    if name == "blank" {
        return "about:blank".into();
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let pages = exe
        .parent()
        .and_then(|d| d.parent())
        .and_then(|d| d.parent())
        .map(|d| d.join("pages"))
        .unwrap_or_default();
    let file = pages.join(format!("{name}.html"));
    let file = file.canonicalize().unwrap_or(file);
    format!("file://{}", file.display())
}

/// A frame ready to paint: the pixels already wrapped as a GPUI image off the UI thread.
#[derive(Clone)]
pub struct UiFrame {
    pub seq: u64,
    pub image: Arc<RenderImage>,
    produced_unix_ns: u64,
}

/// Counters the helper's reader thread updates.
#[derive(Default)]
pub struct ReaderCounters {
    pub received: AtomicU64,
    pub superseded: AtomicU64,
    copy_ns: AtomicU64,
    wrap_ns: AtomicU64,
    pub copy_ns_total: AtomicU64,
    received_total: AtomicU64,
}

impl ReaderCounters {
    pub fn received_total(&self) -> u64 {
        self.received_total.load(Ordering::Relaxed)
    }
}

/// The key-to-frame latency probe. The reader thread hashes one rectangle of every page frame;
/// once armed (just before a key is injected), the first frame whose rectangle differs from the
/// last one before the key is the frame that shows the typed character.
#[derive(Default)]
pub struct Watch {
    /// Device pixels: x, y, width, height.
    pub rect: Option<(u32, u32, u32, u32)>,
    pub last_hash: u64,
    /// When the key was injected, and the rectangle's hash before it.
    pub armed: Option<(Instant, u64)>,
    /// When the changed frame reached the app (reader thread), and its sequence number.
    pub hit: Option<(Instant, u64)>,
    /// When that frame was painted (UI thread, right after `paint_image`).
    pub painted: Option<Instant>,
}

/// What the reader thread tells the UI. Frames of the page do not travel here: the newest one
/// waits in a single slot, so a slow UI thread drops old frames instead of queueing them.
enum ToUi {
    FrameReady,
    PopupFrame(UiFrame),
    Popup {
        show: bool,
        rect: Option<proto::Rect>,
    },
    Cursor(proto::CursorKind),
    ImeBounds(Vec<proto::Rect>),
    DevTools(serde_json::Value),
    Status(String),
    Ready(String),
    Exited(String),
}

/// State the paint closures share with the view across frames.
#[derive(Default)]
pub struct PaintState {
    pub pane: Option<Bounds<Pixels>>,
    pub swatch: Option<Bounds<Pixels>>,
    sent_size: Option<(u32, u32, u32)>,
    painted_view: Option<(u64, Arc<RenderImage>)>,
    painted_popup: Option<Arc<RenderImage>>,
    pointer_inside: bool,
    pub popup_painted: u64,
    pub first_painted: Option<Instant>,
    // Per-second counters.
    paints: u64,
    new_frames_painted: u64,
    paint_ns: u64,
    paint_ns_max: u64,
    latency_ns: u64,
    latency_ns_max: u64,
    // The window frame being built: when `render` started.
    frame_start: Option<Instant>,
    // Recording, for a measurement window.
    pub recording: bool,
    pub rec_painted_at: Vec<Instant>,
    pub rec_paint_ns: Vec<u64>,
    pub rec_latency_ns: Vec<u64>,
    pub rec_ui_frame_ns: Vec<u64>,
    pub rec_window_frames: u64,
}

/// Everything the scripted scenarios wait for or read back.
#[derive(Default)]
pub struct EventLog {
    pub events: Vec<(Instant, String)>,
    pub replies: HashMap<u64, serde_json::Value>,
    pub ready_at: Option<Instant>,
    pub first_frame_at: Option<Instant>,
}

impl EventLog {
    fn push(&mut self, text: String) {
        self.events.push((Instant::now(), text));
    }
}

pub struct Pane {
    pub browser: Rc<RefCell<HelperBrowser>>,
    pub helper_pid: u32,
    pub focus: FocusHandle,
    /// Something else in the app to give focus to, for the focus-out check.
    pub other_focus: FocusHandle,
    pub latest: Arc<Mutex<Option<UiFrame>>>,
    pub counters: Arc<ReaderCounters>,
    pub watch: Arc<Mutex<Watch>>,
    pub view_frame: Option<UiFrame>,
    popup_rect: Option<proto::Rect>,
    popup_frame: Option<UiFrame>,
    cursor: CursorStyle,
    composing: Option<String>,
    ime_bounds: Vec<proto::Rect>,
    engine: String,
    status: String,
    stats_line: String,
    pub paint: Rc<RefCell<PaintState>>,
    pub log: Rc<RefCell<EventLog>>,
    pub sidebar: bool,
    pub spawned_at: Instant,
    pub tree: stats::ProcessTree,
    quiet_ui: bool,
    taken: u64,
    usage_at: (Instant, usage::Usage),
    _subscriptions: Vec<Subscription>,
    _tasks: Vec<Task<()>>,
}

fn main() {
    let args = Args::parse();
    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let (to_ui, from_helper) = mpsc::unbounded::<ToUi>();
        let latest = Arc::new(Mutex::new(None::<UiFrame>));
        let counters = Arc::new(ReaderCounters::default());
        let watch = Arc::new(Mutex::new(Watch::default()));
        let spawned_at = Instant::now();
        let browser = match start_helper(
            &args,
            to_ui,
            latest.clone(),
            counters.clone(),
            watch.clone(),
        ) {
            Ok(browser) => browser,
            Err(error) => {
                eprintln!("app: could not start the helper: {error:#}");
                cx.quit();
                return;
            }
        };

        let displays = cx.displays();
        for (index, display) in displays.iter().enumerate() {
            eprintln!(
                "app: display {index}: id {:?}, bounds {:?}",
                display.id(),
                display.bounds()
            );
        }
        let display_id = args
            .display
            .and_then(|index| displays.get(index))
            .map(|display| display.id());
        let bounds = Bounds::centered(
            display_id,
            size(px(args.width), px(args.height + TOOLBAR_HEIGHT)),
            cx,
        );
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            display_id,
            titlebar: Some(TitlebarOptions {
                title: Some("Browser trial".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let args_for_view = args.clone();
        let opened = cx.open_window(options, move |window, cx| {
            cx.new(|cx| {
                Pane::new(
                    args_for_view,
                    browser,
                    latest,
                    counters,
                    watch,
                    spawned_at,
                    from_helper,
                    window,
                    cx,
                )
            })
        });
        match opened {
            Ok(_) => eprintln!("app: window opened"),
            Err(error) => {
                eprintln!("app: could not open the window: {error:#}");
                cx.quit();
            }
        }
        cx.activate(true);
    });
}

fn start_helper(
    args: &Args,
    to_ui: mpsc::UnboundedSender<ToUi>,
    latest: Arc<Mutex<Option<UiFrame>>>,
    counters: Arc<ReaderCounters>,
    watch: Arc<Mutex<Watch>>,
) -> anyhow::Result<HelperBrowser> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().unwrap_or(std::path::Path::new("."));
    let mut helper_args = Vec::new();
    let program = match args.helper.as_str() {
        "standin" => {
            helper_args.extend(["--fps".to_string(), args.fps.to_string()]);
            if args.demo_popup {
                helper_args.push("--demo-popup".into());
            }
            dir.join(format!("spike-standin{}", std::env::consts::EXE_SUFFIX))
        }
        "cef" => {
            // A dedicated profile beside the build, never the user's browser profile. A fresh
            // one per run unless UDX_SPIKE_PROFILE names one.
            let profile = match std::env::var_os("UDX_SPIKE_PROFILE") {
                Some(dir) => std::path::PathBuf::from(dir),
                None => dir
                    .join("cef-profiles")
                    .join(format!("run-{}", std::process::id())),
            };
            std::fs::create_dir_all(&profile)?;
            helper_args.extend(["--profile-dir".to_string(), profile.display().to_string()]);
            if let Some(url) = &args.start_url {
                helper_args.extend(["--start-url".to_string(), url.clone()]);
            }
            if args.tabs > 1 {
                helper_args.extend(["--tabs".to_string(), args.tabs.to_string()]);
            }
            if let Some(url) = &args.hidden_url {
                helper_args.extend(["--hidden-url".to_string(), url.clone()]);
            }
            if args.probe_shared_texture {
                helper_args.push("--probe-shared-texture".into());
            }
            if let Some(fps) = args.engine_fps {
                helper_args.extend(["--frame-rate".to_string(), fps.to_string()]);
            }
            // On macOS CEF's browser process must run from an app bundle that holds the
            // framework and the helper bundles (see cef-helper/bundle-macos.sh).
            if cfg!(target_os = "macos") {
                dir.join("spike-cef-helper.app/Contents/MacOS/spike-cef-helper")
            } else {
                dir.join(format!("spike-cef-helper{}", std::env::consts::EXE_SUFFIX))
            }
        }
        other => anyhow::bail!("unknown helper {other}"),
    };
    let options = HelperOptions {
        program,
        args: helper_args,
        max_width: MAX_FRAME.0,
        max_height: MAX_FRAME.1,
        slots: 3,
    };
    // The sink runs on the reader thread: wrap pixels as a GPUI image there, not on the UI thread.
    let browser = HelperBrowser::spawn(options, move |event| {
        let message = match event {
            BrowserEvent::Frame(frame) => {
                counters.received.fetch_add(1, Ordering::Relaxed);
                counters.received_total.fetch_add(1, Ordering::Relaxed);
                counters.copy_ns.fetch_add(frame.copy_ns, Ordering::Relaxed);
                counters
                    .copy_ns_total
                    .fetch_add(frame.copy_ns, Ordering::Relaxed);
                let started = Instant::now();
                let kind = frame.kind;
                if kind == proto::FrameKind::View {
                    probe(&watch, &frame);
                }
                let Some(ui) = wrap(frame) else {
                    eprintln!("app: frame with a bad size dropped");
                    return;
                };
                counters
                    .wrap_ns
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
                match kind {
                    proto::FrameKind::Popup => ToUi::PopupFrame(ui),
                    proto::FrameKind::View => {
                        let previous = latest.lock().unwrap().replace(ui);
                        if previous.is_some() {
                            // The UI has not taken the last one yet; it is already waking up.
                            counters.superseded.fetch_add(1, Ordering::Relaxed);
                            return;
                        }
                        ToUi::FrameReady
                    }
                }
            }
            BrowserEvent::Ready { engine } => ToUi::Ready(engine),
            BrowserEvent::Popup { show, rect } => ToUi::Popup { show, rect },
            BrowserEvent::Cursor(cursor) => ToUi::Cursor(cursor),
            BrowserEvent::ImeCompositionBounds { characters, .. } => ToUi::ImeBounds(characters),
            BrowserEvent::DevTools(message) => ToUi::DevTools(message),
            BrowserEvent::Status(text) => ToUi::Status(text),
            BrowserEvent::Exited { status } => ToUi::Exited(status),
        };
        let _ = to_ui.unbounded_send(message);
    })?;
    Ok(browser)
}

/// The latency probe's reader-thread half (see [`Watch`]).
fn probe(watch: &Mutex<Watch>, frame: &proto::Frame) {
    let mut w = watch.lock().unwrap();
    let Some((x, y, rw, rh)) = w.rect else { return };
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let stride = frame.width as usize * 4;
    for row in y..(y + rh).min(frame.height) {
        let start = row as usize * stride + x as usize * 4;
        let end = (start + rw as usize * 4).min((row as usize + 1) * stride);
        for byte in &frame.bgra[start.min(end)..end] {
            hash = (hash ^ *byte as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    if let Some((_, baseline)) = w.armed
        && hash != baseline
    {
        w.hit = Some((Instant::now(), frame.seq));
        w.armed = None;
    }
    w.last_hash = hash;
}

fn wrap(frame: proto::Frame) -> Option<UiFrame> {
    let buffer = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)?;
    // GPUI's RenderImage holds BGRA bytes, which is what the helper painted.
    let image = Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
    Some(UiFrame {
        seq: frame.seq,
        image,
        produced_unix_ns: frame.produced_unix_ns,
    })
}

impl Pane {
    #[allow(clippy::too_many_arguments)]
    fn new(
        args: Args,
        browser: HelperBrowser,
        latest: Arc<Mutex<Option<UiFrame>>>,
        counters: Arc<ReaderCounters>,
        watch: Arc<Mutex<Watch>>,
        spawned_at: Instant,
        mut from_helper: mpsc::UnboundedReceiver<ToUi>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let other_focus = cx.focus_handle();
        let helper_pid = browser.pid();
        let browser = Rc::new(RefCell::new(browser));
        let mut tasks = Vec::new();

        tasks.push(cx.spawn(async move |this, cx| {
            while let Some(message) = from_helper.next().await {
                if this
                    .update(cx, |pane, cx| pane.on_helper(message, cx))
                    .is_err()
                {
                    break;
                }
            }
        }));

        tasks.push(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |pane, cx| pane.tick(cx)).is_err() {
                    break;
                }
            }
        }));

        if let Some(seconds) = args.seconds {
            tasks.push(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_secs(seconds))
                    .await;
                eprintln!("app: {seconds} s are up, quitting");
                let _ = this.update(cx, |pane, cx| {
                    pane.browser.borrow_mut().shutdown();
                    cx.quit();
                });
            }));
        }

        if args.selfcheck {
            tasks.push(cx.spawn_in(window, async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(3000))
                    .await;
                let _ = this.update_in(cx, |pane, window, _cx| pane.selfcheck(window));
            }));
        }

        if let Some(scenario) = args.scenario.clone() {
            let args = args.clone();
            tasks.push(cx.spawn_in(window, async move |this, cx| {
                script::run(scenario, args, this, cx).await;
            }));
        }

        if let Some(url) = args.url.clone() {
            let mut browser = browser.borrow_mut();
            let id = browser.navigate(&url);
            eprintln!("app: devtools call {id}: Page.navigate");
            let id = browser.evaluate("document.title");
            eprintln!("app: devtools call {id}: Runtime.evaluate document.title");
        }

        let subscriptions = vec![
            cx.on_focus(&focus, window, |pane, _window, _cx| {
                pane.send(proto::InputEvent::Focus { focused: true })
            }),
            cx.on_blur(&focus, window, |pane, _window, _cx| {
                pane.send(proto::InputEvent::Focus { focused: false })
            }),
            cx.on_app_quit(|pane, _cx| {
                pane.browser.borrow_mut().shutdown();
                async {}
            }),
        ];

        Pane {
            browser,
            helper_pid,
            focus,
            other_focus,
            latest,
            counters,
            watch,
            view_frame: None,
            popup_rect: None,
            popup_frame: None,
            cursor: CursorStyle::Arrow,
            composing: None,
            ime_bounds: Vec::new(),
            engine: "starting".into(),
            status: String::new(),
            stats_line: String::new(),
            paint: Rc::new(RefCell::new(PaintState::default())),
            log: Rc::new(RefCell::new(EventLog::default())),
            sidebar: false,
            spawned_at,
            tree: stats::ProcessTree::default(),
            quiet_ui: args.quiet_ui,
            taken: 0,
            usage_at: (Instant::now(), usage::current()),
            _subscriptions: subscriptions,
            _tasks: tasks,
        }
    }

    pub fn send(&mut self, event: proto::InputEvent) {
        self.browser.borrow_mut().input(event);
    }

    fn on_helper(&mut self, message: ToUi, cx: &mut Context<Self>) {
        match message {
            ToUi::FrameReady => {
                if let Some(frame) = self.latest.lock().unwrap().take() {
                    let mut log = self.log.borrow_mut();
                    if log.first_frame_at.is_none() {
                        log.first_frame_at = Some(Instant::now());
                        eprintln!(
                            "app: first frame {:.1} ms after spawn",
                            self.spawned_at.elapsed().as_secs_f64() * 1e3
                        );
                    }
                    drop(log);
                    self.view_frame = Some(frame);
                    self.taken += 1;
                    cx.notify();
                }
            }
            ToUi::PopupFrame(frame) => {
                self.popup_frame = Some(frame);
                cx.notify();
            }
            ToUi::Popup { show, rect } => {
                eprintln!("app: popup show={show} rect={rect:?}");
                self.log
                    .borrow_mut()
                    .push(format!("popup show={show} rect={rect:?}"));
                self.popup_rect = if show { rect } else { None };
                if !show {
                    self.popup_frame = None;
                }
                cx.notify();
            }
            ToUi::Cursor(kind) => {
                eprintln!("app: cursor {kind:?}");
                self.log.borrow_mut().push(format!("cursor {kind:?}"));
                self.cursor = cursor_style(kind);
                cx.notify();
            }
            ToUi::ImeBounds(characters) => {
                eprintln!("app: composition bounds {characters:?}");
                self.log
                    .borrow_mut()
                    .push(format!("ime-bounds {}", characters.len()));
                self.ime_bounds = characters;
            }
            ToUi::DevTools(message) => {
                let mut log = self.log.borrow_mut();
                match message.get("id").and_then(|v| v.as_u64()) {
                    Some(id) => {
                        let shown = message.to_string();
                        eprintln!(
                            "app: devtools reply {}",
                            shown.chars().take(300).collect::<String>()
                        );
                        self.status = format!(
                            "DevTools {id}: {}",
                            shown.chars().take(120).collect::<String>()
                        );
                        log.replies.insert(id, message);
                    }
                    None => {
                        let method = message["method"].as_str().unwrap_or("?").to_string();
                        log.push(format!("devtools-event {method}"));
                    }
                }
                cx.notify();
            }
            ToUi::Status(text) => {
                self.log.borrow_mut().push(text);
            }
            ToUi::Ready(engine) => {
                eprintln!(
                    "app: helper ready: {engine} ({:.1} ms after spawn)",
                    self.spawned_at.elapsed().as_secs_f64() * 1e3
                );
                self.log.borrow_mut().ready_at = Some(Instant::now());
                self.engine = engine;
                cx.notify();
            }
            ToUi::Exited(status) => {
                eprintln!("app: helper gone: {status}");
                self.log.borrow_mut().push(format!("exited {status}"));
                self.engine = format!("helper gone ({status})");
                cx.notify();
            }
        }
    }

    /// Once a second: log and show what the last second looked like.
    fn tick(&mut self, cx: &mut Context<Self>) {
        let received = self.counters.received.swap(0, Ordering::Relaxed);
        let superseded = self.counters.superseded.load(Ordering::Relaxed);
        let copy_ns = self.counters.copy_ns.swap(0, Ordering::Relaxed);
        let wrap_ns = self.counters.wrap_ns.swap(0, Ordering::Relaxed);
        let taken = std::mem::take(&mut self.taken);
        let mut p = self.paint.borrow_mut();
        let painted = std::mem::take(&mut p.new_frames_painted);
        let paints = std::mem::take(&mut p.paints);
        let paint_ns = std::mem::take(&mut p.paint_ns);
        let paint_max = std::mem::take(&mut p.paint_ns_max);
        let latency_ns = std::mem::take(&mut p.latency_ns);
        let latency_max = std::mem::take(&mut p.latency_ns_max);
        let pane = p.pane;
        let size = p.sent_size;
        drop(p);

        let now = usage::current();
        let elapsed = self.usage_at.0.elapsed();
        let cpu = now.cpu_percent_since(&self.usage_at.1, elapsed);
        self.usage_at = (Instant::now(), now);
        let tree = self.tree.sample(self.helper_pid as i32);
        let tree_footprint: u64 = tree.iter().map(|s| s.footprint).sum();

        let avg = |total: u64, n: u64| total as f64 / n.max(1) as f64 / 1e6;
        let device = size.map(|(w, h, s)| {
            let s = s as f32 / 100.0;
            format!("{}x{}", (w as f32 * s).round(), (h as f32 * s).round())
        });
        self.stats_line = format!(
            "{painted} fps painted, paint {:.2} ms, latency {:.1} ms, app cpu {cpu:.0}%",
            avg(paint_ns, painted),
            avg(latency_ns, painted),
        );
        eprintln!(
            "app: stats received={received} superseded_total={superseded} taken={taken} painted_new={painted} paint_calls={paints} \
             copy_avg_ms={:.2} wrap_avg_ms={:.3} paint_image_avg_ms={:.2} paint_image_max_ms={:.2} \
             latency_avg_ms={:.1} latency_max_ms={:.1} cpu={cpu:.1}% resident={:.0}MB footprint={:.0}MB \
             helper_tree_processes={} helper_tree_footprint={:.0}MB pane_logical={:?} device={}",
            avg(copy_ns, received),
            avg(wrap_ns, received),
            avg(paint_ns, painted),
            paint_max as f64 / 1e6,
            avg(latency_ns, painted),
            latency_max as f64 / 1e6,
            now.resident_bytes as f64 / 1e6,
            now.footprint_bytes as f64 / 1e6,
            tree.len(),
            tree_footprint as f64 / 1e6,
            pane.map(|b| (f32::from(b.size.width), f32::from(b.size.height))),
            device.unwrap_or_else(|| "none".into()),
        );
        if !self.quiet_ui {
            cx.notify();
        }
    }

    /// Reads the window back and checks two pixels: the overlay's magenta swatch must be
    /// magenta (the overlay draws above the page), and for the stand-in its green corner marker
    /// must be green (the page's frames reach the window). Returns the readback for the
    /// scenarios.
    pub fn selfcheck(&mut self, window: &mut Window) -> Option<image::RgbaImage> {
        let image = match window.render_to_image() {
            Ok(image) => image,
            Err(error) => {
                eprintln!("app: selfcheck could not read the window back: {error:#}");
                return None;
            }
        };
        let scale = window.scale_factor();
        let sample = |at: Point<Pixels>| {
            let x = (f32::from(at.x) * scale) as u32;
            let y = (f32::from(at.y) * scale) as u32;
            image.get_pixel_checked(x, y).map(|p| p.0)
        };
        let p = self.paint.borrow();
        let near = |c: Option<[u8; 4]>, want: [u8; 3]| {
            c.is_some_and(|c| (0..3).all(|i| (c[i] as i32 - want[i] as i32).abs() <= 8))
        };
        let swatch = p.swatch.and_then(|b| sample(b.center()));
        let marker = p
            .pane
            .and_then(|b| sample(b.origin + point(px(4.0), px(4.0))));
        eprintln!(
            "app: selfcheck window_readback={}x{} scale={scale} overlay_swatch_pixel={swatch:?} overlay_above_page={} \
             page_corner_pixel={marker:?} standin_marker_visible={}",
            image.width(),
            image.height(),
            near(swatch, [255, 0, 255]),
            near(marker, [0, 255, 0]),
        );
        Some(image)
    }

    pub fn local(&self, position: Point<Pixels>) -> (f32, f32) {
        let origin = self
            .paint
            .borrow()
            .pane
            .map(|b| b.origin)
            .unwrap_or_default();
        (
            f32::from(position.x - origin.x),
            f32::from(position.y - origin.y),
        )
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let (x, y) = self.local(event.position);
        self.send(proto::InputEvent::PointerDown {
            x,
            y,
            button: button(event.button),
            click_count: event.click_count as u32,
            modifiers: modifiers(&event.modifiers),
        });
    }

    fn mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        let (x, y) = self.local(event.position);
        self.send(proto::InputEvent::PointerUp {
            x,
            y,
            button: button(event.button),
            click_count: event.click_count as u32,
            modifiers: modifiers(&event.modifiers),
        });
    }

    fn mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        let (x, y) = self.local(event.position);
        self.send(proto::InputEvent::PointerMove {
            x,
            y,
            pressed: event.pressed_button.map(button),
            modifiers: modifiers(&event.modifiers),
        });
    }

    fn scroll(&mut self, event: &ScrollEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        let (x, y) = self.local(event.position);
        let (delta_x, delta_y, unit) = match event.delta {
            ScrollDelta::Pixels(d) => (f32::from(d.x), f32::from(d.y), proto::WheelUnit::Pixels),
            ScrollDelta::Lines(d) => (d.x, d.y, proto::WheelUnit::Lines),
        };
        self.send(proto::InputEvent::Wheel {
            x,
            y,
            delta_x,
            delta_y,
            unit,
            modifiers: modifiers(&event.modifiers),
        });
    }

    fn key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        // Propagation is left alone, so the platform text input still sees the key and
        // delivers characters and compositions through the input handler below.
        self.send(proto::InputEvent::KeyDown {
            key: event.keystroke.key.clone(),
            key_char: event.keystroke.key_char.clone(),
            modifiers: modifiers(&event.keystroke.modifiers),
            is_held: event.is_held,
        });
    }

    fn key_up(&mut self, event: &KeyUpEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        self.send(proto::InputEvent::KeyUp {
            key: event.keystroke.key.clone(),
            modifiers: modifiers(&event.keystroke.modifiers),
        });
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px_3()
                .py_1()
                .rounded_md()
                .bg(rgb(0x3b3b3b))
                .hover(|s| s.bg(rgb(0x4b4b4b)))
                .cursor_pointer()
                .child(label)
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .h(px(TOOLBAR_HEIGHT))
            .bg(rgb(0x2b2b2b))
            .track_focus(&self.other_focus)
            .child(format!("Engine: {}", self.engine))
            .child(
                button("navigate", "Navigate").on_click(cx.listener(|pane, _, _, _| {
                    let id = pane.browser.borrow_mut().navigate("https://example.com/");
                    eprintln!("app: devtools call {id}: Page.navigate https://example.com/");
                })),
            )
            .child(
                button("evaluate", "Evaluate").on_click(cx.listener(|pane, _, _, _| {
                    let id = pane.browser.borrow_mut().evaluate("document.title");
                    eprintln!("app: devtools call {id}: Runtime.evaluate document.title");
                })),
            )
            .child(div().flex_1().truncate().child(self.status.clone()))
            .child(self.stats_line.clone())
    }

    fn overlay(&self) -> impl IntoElement {
        let paint = self.paint.clone();
        div()
            .id("overlay")
            .absolute()
            .top(px(16.0))
            .right(px(16.0))
            .w(px(280.0))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .rounded_md()
            .bg(rgba(0x111827e6))
            .border_1()
            .border_color(rgb(0xf59e0b))
            .text_color(rgb(0xf9fafb))
            // An ask card over the page must take clicks itself, not pass them to the page.
            .occlude()
            .child("Allow this page to use the camera?")
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    .child(
                        canvas(
                            move |bounds, _window, _cx| {
                                paint.borrow_mut().swatch = Some(bounds);
                            },
                            |bounds, (), window, _cx| {
                                window.paint_quad(fill(bounds, rgb(0xff00ff)));
                            },
                        )
                        .w(px(24.0))
                        .h(px(24.0)),
                    )
                    .child("stand-in card (GPUI, above the page)"),
            )
    }
}

impl Render for Pane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.paint.borrow_mut().frame_start = Some(Instant::now());
        let entity = cx.entity();
        let paint = self.paint.clone();
        let paint_for_prepaint = self.paint.clone();
        let paint_for_end = self.paint.clone();
        let browser = self.browser.clone();
        let browser_for_paint = self.browser.clone();
        let focus = self.focus.clone();
        let view_frame = self.view_frame.clone();
        let popup = self.popup_rect.zip(self.popup_frame.clone());
        let cursor = self.cursor;
        let watch = self.watch.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xf0f0f0))
            .text_sm()
            .child(self.toolbar(cx))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .flex_row()
                    .when_some_sidebar(self.sidebar)
                    .child(
                        div()
                            .id("browser-pane")
                            .relative()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .track_focus(&self.focus)
                            .on_any_mouse_down(cx.listener(Self::mouse_down))
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
                            .on_mouse_up(MouseButton::Right, cx.listener(Self::mouse_up))
                            .on_mouse_up(MouseButton::Middle, cx.listener(Self::mouse_up))
                            .on_mouse_move(cx.listener(Self::mouse_move))
                            .on_scroll(cx.listener(Self::scroll))
                            .on_key_down(cx.listener(Self::key_down))
                            .on_key_up(cx.listener(Self::key_up))
                            .child(
                                canvas(
                                    move |bounds, window, _cx| {
                                        // Tell the engine the pane's size whenever it or the
                                        // scale changes.
                                        let scale = window.scale_factor();
                                        let w = f32::from(bounds.size.width).round() as u32;
                                        let h = f32::from(bounds.size.height).round() as u32;
                                        let key = (w, h, (scale * 100.0).round() as u32);
                                        let mut p = paint_for_prepaint.borrow_mut();
                                        p.pane = Some(bounds);
                                        if p.sent_size != Some(key) && w > 0 && h > 0 {
                                            p.sent_size = Some(key);
                                            eprintln!("app: pane {w}x{h} logical at scale {scale}");
                                            browser.borrow_mut().resize(w, h, scale);
                                        }
                                        // In prepaint, like every element's hitbox, so the
                                        // overlay (also inserted in prepaint, later) sits above
                                        // it for hit testing.
                                        window.insert_hitbox(bounds, HitboxBehavior::Normal)
                                    },
                                    move |bounds, hitbox, window, cx| {
                                        window.handle_input(
                                            &focus,
                                            ElementInputHandler::new(bounds, entity.clone()),
                                            cx,
                                        );
                                        window.set_cursor_style(cursor, &hitbox);
                                        // The page hears that the pointer left when it leaves
                                        // the pane or moves onto something drawn above it, such
                                        // as the overlay. (GPUI's hover state is not used: it
                                        // also turns off while typing.)
                                        let leave_paint = paint.clone();
                                        let leave_browser = browser_for_paint.clone();
                                        window.on_mouse_event(
                                            move |event: &MouseMoveEvent, phase, window, _cx| {
                                                if phase != DispatchPhase::Bubble {
                                                    return;
                                                }
                                                let inside = hitbox.is_hovered(window);
                                                let mut p = leave_paint.borrow_mut();
                                                if p.pointer_inside && !inside {
                                                    leave_browser.borrow_mut().input(
                                                        proto::InputEvent::PointerLeave {
                                                            modifiers: modifiers(&event.modifiers),
                                                        },
                                                    );
                                                }
                                                p.pointer_inside = inside;
                                            },
                                        );
                                        paint_frames(
                                            &paint, &watch, bounds, view_frame, popup, window,
                                        );
                                    },
                                )
                                .size_full(),
                            )
                            .child(self.overlay()),
                    ),
            )
            .child(
                // Painted last: closes the measurement of how long the UI thread spent building
                // this window frame (render, layout, prepaint, paint, including paint_image).
                canvas(
                    |_, _, _| {},
                    move |_, _, _, _| {
                        let mut p = paint_for_end.borrow_mut();
                        if let Some(start) = p.frame_start.take()
                            && p.recording
                        {
                            p.rec_ui_frame_ns.push(start.elapsed().as_nanos() as u64);
                            p.rec_window_frames += 1;
                        }
                    },
                )
                .absolute()
                .size_0(),
            )
    }
}

/// A sidebar of fixed width beside the browser pane, toggled by the resize check.
trait SidebarExt {
    fn when_some_sidebar(self, show: bool) -> Self;
}

impl SidebarExt for gpui::Div {
    fn when_some_sidebar(self, show: bool) -> Self {
        if show {
            self.child(
                div()
                    .w(px(SIDEBAR_WIDTH))
                    .h_full()
                    .bg(rgb(0x252526))
                    .p_3()
                    .child("sidebar (pane resize check)"),
            )
        } else {
            self
        }
    }
}

/// Paints the newest page frame and any popup, and frees the atlas space of the frame painted
/// before. Each frame is a new `RenderImage`, so GPUI uploads it once into its sprite atlas;
/// one that was superseded before it was painted never reaches the atlas at all.
fn paint_frames(
    paint: &Rc<RefCell<PaintState>>,
    watch: &Mutex<Watch>,
    bounds: Bounds<Pixels>,
    view_frame: Option<UiFrame>,
    popup: Option<(proto::Rect, UiFrame)>,
    window: &mut Window,
) {
    let mut p = paint.borrow_mut();
    p.paints += 1;
    let scale = window.scale_factor();
    let Some(frame) = view_frame else {
        window.paint_quad(fill(bounds, rgb(0x101010)));
        return;
    };
    let device = frame.image.size(0);
    let rect = Bounds::new(
        bounds.origin,
        size(
            px(device.width.0 as f32 / scale),
            px(device.height.0 as f32 / scale),
        ),
    );
    let started = Instant::now();
    let _ = window.paint_image(
        rect,
        rect,
        Corners::default(),
        frame.image.clone(),
        0,
        false,
    );
    let is_new = p.painted_view.as_ref().map(|(seq, _)| *seq) != Some(frame.seq);
    if is_new {
        let now = Instant::now();
        let paint_ns = (now - started).as_nanos() as u64;
        let latency = spike_protocol::wire::unix_ns().saturating_sub(frame.produced_unix_ns);
        p.first_painted.get_or_insert(now);
        p.new_frames_painted += 1;
        p.paint_ns += paint_ns;
        p.paint_ns_max = p.paint_ns_max.max(paint_ns);
        p.latency_ns += latency;
        p.latency_ns_max = p.latency_ns_max.max(latency);
        if p.recording {
            p.rec_painted_at.push(now);
            p.rec_paint_ns.push(paint_ns);
            p.rec_latency_ns.push(latency);
        }
        {
            let mut w = watch.lock().unwrap();
            if w.hit.is_some_and(|(_, seq)| seq == frame.seq) && w.painted.is_none() {
                w.painted = Some(now);
            }
        }
        if let Some((_, old)) = p.painted_view.replace((frame.seq, frame.image.clone())) {
            let _ = window.drop_image(old);
        }
    }

    match popup {
        Some((rect, frame)) => {
            let at = Bounds::new(
                bounds.origin + point(px(rect.x as f32), px(rect.y as f32)),
                size(px(rect.width as f32), px(rect.height as f32)),
            );
            let _ = window.paint_image(at, at, Corners::default(), frame.image.clone(), 0, false);
            let changed = p
                .painted_popup
                .as_ref()
                .is_none_or(|old| old.id != frame.image.id);
            if changed {
                p.popup_painted += 1;
                if let Some(old) = p.painted_popup.replace(frame.image.clone()) {
                    let _ = window.drop_image(old);
                }
            }
        }
        None => {
            if let Some(old) = p.painted_popup.take() {
                let _ = window.drop_image(old);
            }
        }
    }
}

impl EntityInputHandler for Pane {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        // The page's text is not visible from here; the composition is all the app knows.
        None
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = self
            .composing
            .as_ref()
            .map_or(0, |t| t.encode_utf16().count());
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.composing.as_ref().map(|t| 0..t.encode_utf16().count())
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        eprintln!("app: input handler: unmark_text");
        if self.composing.take().is_some() {
            self.send(proto::InputEvent::ImeFinish);
        }
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.composing = None;
        self.send(proto::InputEvent::Text { text: text.into() });
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        eprintln!(
            "app: input handler: replace_and_mark_text_in_range {new_text:?} {new_selected_range:?}"
        );
        if new_text.is_empty() {
            self.composing = None;
            self.send(proto::InputEvent::ImeCancel);
            return;
        }
        let len = new_text.encode_utf16().count();
        let selected = new_selected_range.unwrap_or(len..len);
        self.composing = Some(new_text.into());
        self.send(proto::InputEvent::ImeSetComposition {
            text: new_text.into(),
            selected_start: selected.start as u32,
            selected_end: selected.end as u32,
        });
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // Where the engine says the composed characters are, so the candidate window follows.
        let r = self
            .ime_bounds
            .get(range_utf16.start)
            .or(self.ime_bounds.last())?;
        Some(Bounds::new(
            element_bounds.origin + point(px(r.x as f32), px(r.y as f32)),
            size(px(r.width as f32), px(r.height as f32)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

fn modifiers(m: &gpui::Modifiers) -> proto::Modifiers {
    proto::Modifiers {
        shift: m.shift,
        control: m.control,
        alt: m.alt,
        platform: m.platform,
        function: m.function,
    }
}

fn button(b: MouseButton) -> proto::PointerButton {
    match b {
        MouseButton::Left => proto::PointerButton::Left,
        MouseButton::Right => proto::PointerButton::Right,
        MouseButton::Middle => proto::PointerButton::Middle,
        MouseButton::Navigate(NavigationDirection::Back) => proto::PointerButton::Back,
        MouseButton::Navigate(NavigationDirection::Forward) => proto::PointerButton::Forward,
    }
}

fn cursor_style(kind: proto::CursorKind) -> CursorStyle {
    use proto::CursorKind::*;
    match kind {
        Arrow => CursorStyle::Arrow,
        IBeam => CursorStyle::IBeam,
        PointingHand => CursorStyle::PointingHand,
        Crosshair => CursorStyle::Crosshair,
        Grab => CursorStyle::OpenHand,
        Grabbing => CursorStyle::ClosedHand,
        ResizeLeftRight => CursorStyle::ResizeLeftRight,
        ResizeUpDown => CursorStyle::ResizeUpDown,
        NotAllowed => CursorStyle::OperationNotAllowed,
        // GPUI hides the cursor only while typing; an arrow is the nearest.
        Hidden => CursorStyle::Arrow,
    }
}
