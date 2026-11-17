//! The browser trial's GPUI window (Task 12, decision 0012). Throwaway.
//!
//! A window with a toolbar and a browser pane. The pane draws frames that a helper process
//! paints into shared memory, as an element; forwards pointer, wheel, key, text and
//! input-method events to the helper; shows the cursor and popups the helper asks for; and draws
//! a GPUI overlay card above the page. The helper is the stand-in test pattern now and the CEF
//! helper in phase B; both speak the protocol in `spike-protocol`.
//!
//! ```sh
//! cargo run --release -p spike-app -- [options]
//!   --helper standin|cef   which helper to start (default standin)
//!   --width W --height H   requested pane size in logical pixels (default 1280 x 800)
//!   --display N            open the window on the Nth display GPUI lists (logged at start)
//!   --fps N                frame rate the stand-in paints at (default 60)
//!   --seconds N            quit after N seconds (for measurements)
//!   --demo-popup           the stand-in shows and hides a popup every 1.5 s
//!   --script               drive a scripted input sequence through GPUI's own event dispatch
//!   --selfcheck            read the window back and check the overlay is drawn above the page
//!   --url URL              navigate there at start (a DevTools call), then evaluate document.title
//! ```
//!
//! Everything it measures goes to stderr, one `app:` line per second; the helper's lines start
//! with its own name.

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::{
    App, AppContext as _, Bounds, Context, Corners, CursorStyle, DispatchPhase,
    ElementInputHandler, EntityInputHandler, FocusHandle, HitboxBehavior, InteractiveElement as _,
    IntoElement, KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, NavigationDirection, ParentElement as _, Pixels, PlatformInput, Point, Render,
    RenderImage, ScrollDelta, ScrollEvent, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, TitlebarOptions, UTF16Selection, Window, WindowBounds, WindowOptions,
    canvas, div, fill, point, px, rgb, rgba, size,
};
use spike_protocol::helper::{HelperBrowser, HelperOptions};
use spike_protocol::usage;
use spike_protocol::view::{self as proto, BrowserEvent, BrowserView as _};
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOOLBAR_HEIGHT: f32 = 44.0;
/// Shared memory is sized for frames up to this many device pixels.
const MAX_FRAME: (u32, u32) = (3840, 2400);

#[derive(Clone)]
struct Args {
    helper: String,
    width: f32,
    height: f32,
    fps: u32,
    seconds: Option<u64>,
    demo_popup: bool,
    script: bool,
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
        Args {
            helper: value("--helper").unwrap_or_else(|| "standin".into()),
            width: value("--width")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1280.0),
            height: value("--height")
                .and_then(|v| v.parse().ok())
                .unwrap_or(800.0),
            fps: value("--fps").and_then(|v| v.parse().ok()).unwrap_or(60),
            seconds: value("--seconds").and_then(|v| v.parse().ok()),
            demo_popup: flag("--demo-popup"),
            script: flag("--script"),
            selfcheck: flag("--selfcheck"),
            url: value("--url"),
            display: value("--display").and_then(|v| v.parse().ok()),
        }
    }
}

/// A frame ready to paint: the pixels already wrapped as a GPUI image off the UI thread.
#[derive(Clone)]
struct UiFrame {
    seq: u64,
    image: Arc<RenderImage>,
    produced_unix_ns: u64,
}

/// Counters the helper's reader thread updates.
#[derive(Default)]
struct ReaderCounters {
    received: AtomicU64,
    superseded: AtomicU64,
    copy_ns: AtomicU64,
    wrap_ns: AtomicU64,
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
    Ready(String),
    Exited(String),
}

/// State the paint closures share with the view across frames.
#[derive(Default)]
struct PaintState {
    pane: Option<Bounds<Pixels>>,
    swatch: Option<Bounds<Pixels>>,
    sent_size: Option<(u32, u32, u32)>,
    painted_view: Option<(u64, Arc<RenderImage>)>,
    painted_popup: Option<Arc<RenderImage>>,
    pointer_inside: bool,
    // Per-second counters.
    paints: u64,
    new_frames_painted: u64,
    paint_ns: u64,
    paint_ns_max: u64,
    latency_ns: u64,
    latency_ns_max: u64,
}

struct Pane {
    browser: Rc<RefCell<HelperBrowser>>,
    helper_pid: u32,
    focus: FocusHandle,
    latest: Arc<Mutex<Option<UiFrame>>>,
    counters: Arc<ReaderCounters>,
    view_frame: Option<UiFrame>,
    popup_rect: Option<proto::Rect>,
    popup_frame: Option<UiFrame>,
    cursor: CursorStyle,
    composing: Option<String>,
    ime_bounds: Vec<proto::Rect>,
    engine: String,
    status: String,
    stats_line: String,
    paint: Rc<RefCell<PaintState>>,
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
        let browser = match start_helper(&args, to_ui, latest.clone(), counters.clone()) {
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
            // A dedicated profile beside the build, never the user's browser profile.
            let profile = dir.join("cef-profile");
            std::fs::create_dir_all(&profile)?;
            helper_args.extend(["--profile-dir".to_string(), profile.display().to_string()]);
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
                counters.copy_ns.fetch_add(frame.copy_ns, Ordering::Relaxed);
                let started = Instant::now();
                let kind = frame.kind;
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
            BrowserEvent::Exited { status } => ToUi::Exited(status),
        };
        let _ = to_ui.unbounded_send(message);
    })?;
    Ok(browser)
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
        mut from_helper: mpsc::UnboundedReceiver<ToUi>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
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

        if args.script {
            tasks.push(cx.spawn_in(window, async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(1500))
                    .await;
                run_script(this, cx).await;
            }));
        }

        if let Some(url) = args.url.clone() {
            let mut browser = browser.borrow_mut();
            let id = browser.navigate(&url);
            eprintln!("app: devtools call {id}: Page.navigate {url}");
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
            latest,
            counters,
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
            taken: 0,
            usage_at: (Instant::now(), usage::current()),
            _subscriptions: subscriptions,
            _tasks: tasks,
        }
    }

    fn send(&mut self, event: proto::InputEvent) {
        self.browser.borrow_mut().input(event);
    }

    fn on_helper(&mut self, message: ToUi, cx: &mut Context<Self>) {
        match message {
            ToUi::FrameReady => {
                if let Some(frame) = self.latest.lock().unwrap().take() {
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
                self.popup_rect = if show { rect } else { None };
                if !show {
                    self.popup_frame = None;
                }
                cx.notify();
            }
            ToUi::Cursor(kind) => {
                eprintln!("app: cursor {kind:?}");
                self.cursor = cursor_style(kind);
                cx.notify();
            }
            ToUi::ImeBounds(characters) => {
                eprintln!("app: composition bounds {characters:?}");
                self.ime_bounds = characters;
            }
            ToUi::DevTools(message) => {
                eprintln!("app: devtools reply {message}");
                self.status = format!("DevTools: {message}");
                cx.notify();
            }
            ToUi::Ready(engine) => {
                eprintln!("app: helper ready: {engine}");
                self.engine = engine;
                cx.notify();
            }
            ToUi::Exited(status) => {
                eprintln!("app: helper gone: {status}");
                self.engine = format!("helper gone ({status})");
                cx.notify();
            }
        }
    }

    /// Once a second: log and show what the last second looked like.
    fn tick(&mut self, cx: &mut Context<Self>) {
        let received = self.counters.received.swap(0, Ordering::Relaxed);
        let superseded = self.counters.superseded.swap(0, Ordering::Relaxed);
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

        let avg = |total: u64, n: u64| total as f64 / n.max(1) as f64 / 1e6;
        let device = size.map(|(w, h, s)| {
            let s = s as f32 / 100.0;
            format!("{}x{}", (w as f32 * s).round(), (h as f32 * s).round())
        });
        self.stats_line = format!(
            "{painted} fps painted, {received} received, paint {:.2} ms, latency {:.1} ms, app cpu {cpu:.0}%",
            avg(paint_ns, painted),
            avg(latency_ns, painted),
        );
        eprintln!(
            "app: stats received={received} superseded={superseded} taken={taken} painted_new={painted} paint_calls={paints} \
             copy_avg_ms={:.2} wrap_avg_ms={:.3} paint_image_avg_ms={:.2} paint_image_max_ms={:.2} \
             latency_avg_ms={:.1} latency_max_ms={:.1} cpu={cpu:.1}% resident={:.0}MB footprint={:.0}MB \
             pane_logical={:?} device={} helper_pid={}",
            avg(copy_ns, received),
            avg(wrap_ns, received),
            avg(paint_ns, painted),
            paint_max as f64 / 1e6,
            avg(latency_ns, painted),
            latency_max as f64 / 1e6,
            now.resident_bytes as f64 / 1e6,
            now.footprint_bytes as f64 / 1e6,
            pane.map(|b| (f32::from(b.size.width), f32::from(b.size.height))),
            device.unwrap_or_else(|| "none".into()),
            self.helper_pid,
        );
        cx.notify();
    }

    /// Reads the window back and checks two pixels: the overlay's magenta swatch must be
    /// magenta (the overlay draws above the page), and the stand-in's green corner marker must
    /// be green (the page's frames reach the window).
    fn selfcheck(&mut self, window: &mut Window) {
        let image = match window.render_to_image() {
            Ok(image) => image,
            Err(error) => {
                eprintln!("app: selfcheck could not read the window back: {error:#}");
                return;
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
        let swatch = p.swatch.map(|b| sample(b.center()));
        let marker = p.pane.map(|b| sample(b.origin + point(px(4.0), px(4.0))));
        let swatch = swatch.flatten();
        let marker = marker.flatten();
        eprintln!(
            "app: selfcheck window_readback={}x{} scale={scale} overlay_swatch_pixel={swatch:?} overlay_above_page={} \
             page_marker_pixel={marker:?} page_frame_visible={}",
            image.width(),
            image.height(),
            near(swatch, [255, 0, 255]),
            near(marker, [0, 255, 0]),
        );
    }

    fn local(&self, position: Point<Pixels>) -> (f32, f32) {
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
        eprintln!("app: pane got mouse down at ({x:.0}, {y:.0})");
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
            .child("GPUI overlay, drawn above the page")
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
                    .child("self-check swatch"),
            )
    }
}

impl Render for Pane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let paint = self.paint.clone();
        let paint_for_prepaint = self.paint.clone();
        let browser = self.browser.clone();
        let browser_for_paint = self.browser.clone();
        let focus = self.focus.clone();
        let view_frame = self.view_frame.clone();
        let popup = self.popup_rect.zip(self.popup_frame.clone());
        let cursor = self.cursor;

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
                    .id("browser-pane")
                    .relative()
                    .flex_1()
                    .w_full()
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
                                // Tell the engine the pane's size whenever it or the scale changes.
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
                                // In prepaint, like every element's hitbox, so the overlay (also
                                // inserted in prepaint, later) sits above it for hit testing.
                                window.insert_hitbox(bounds, HitboxBehavior::Normal)
                            },
                            move |bounds, hitbox, window, cx| {
                                window.handle_input(
                                    &focus,
                                    ElementInputHandler::new(bounds, entity.clone()),
                                    cx,
                                );
                                window.set_cursor_style(cursor, &hitbox);
                                // The page hears that the pointer left when it leaves the pane or
                                // moves onto something drawn above it, such as the overlay. (GPUI's
                                // hover state is not used: it also turns off while typing.)
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
                                paint_frames(&paint, bounds, view_frame, popup, window);
                            },
                        )
                        .size_full(),
                    )
                    .child(self.overlay()),
            )
    }
}

/// Paints the newest page frame and any popup, and frees the atlas space of the frame painted
/// before. Each frame is a new `RenderImage`, so GPUI uploads it once into its sprite atlas;
/// one that was superseded before it was painted never reaches the atlas at all.
fn paint_frames(
    paint: &Rc<RefCell<PaintState>>,
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
        let paint_ns = started.elapsed().as_nanos() as u64;
        let latency = spike_protocol::wire::unix_ns().saturating_sub(frame.produced_unix_ns);
        p.new_frames_painted += 1;
        p.paint_ns += paint_ns;
        p.paint_ns_max = p.paint_ns_max.max(paint_ns);
        p.latency_ns += latency;
        p.latency_ns_max = p.latency_ns_max.max(latency);
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
            if changed && let Some(old) = p.painted_popup.replace(frame.image.clone()) {
                let _ = window.drop_image(old);
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

/// A scripted input sequence through GPUI's own event dispatch (hit testing, focus, element
/// listeners, the input handler), so the forwarding path runs end to end without a person at
/// the keyboard. Input-method compositions cannot be dispatched this way; the script calls the
/// pane's input-handler methods directly for those, which skips the OS input method.
async fn run_script(this: gpui::WeakEntity<Pane>, cx: &mut gpui::AsyncWindowContext) {
    let executor = cx.background_executor().clone();
    let pause = |ms| executor.timer(Duration::from_millis(ms));
    let Ok((pane, swatch)) = this.update(cx, |p, _| {
        let s = p.paint.borrow();
        (s.pane, s.swatch)
    }) else {
        return;
    };
    let Some(pane) = pane else {
        eprintln!("app: script: the pane has not been laid out");
        return;
    };
    let at = |x: f32, y: f32| pane.origin + point(px(x), px(y));
    eprintln!("app: script: start");

    // Pointer moves: into the stand-in's link area, then its text field.
    for (x, y) in [(400.0, 300.0), (120.0, 145.0), (200.0, 80.0)] {
        let _ = cx.update(|window, cx| {
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: at(x, y),
                    pressed_button: None,
                    modifiers: Default::default(),
                }),
                cx,
            )
        });
        pause(150).await;
    }

    // Click the text field: focuses the pane and reaches the page.
    click(cx, at(200.0, 80.0));
    pause(150).await;

    // Type through GPUI's keystroke dispatch, which also feeds the input handler.
    for key in ["h", "e", "l", "l", "o", "space", "backspace"] {
        if let Ok(keystroke) = Keystroke::parse(key) {
            let _ = cx.update(|window, cx| window.dispatch_keystroke(keystroke, cx));
        }
        pause(60).await;
    }

    // A composition, by direct calls (see above): "ni" becomes "日本" and is committed.
    let _ = this.update_in(cx, |p, window, cx| {
        p.replace_and_mark_text_in_range(None, "に", Some(1..1), window, cx);
    });
    pause(150).await;
    let _ = this.update_in(cx, |p, window, cx| {
        p.replace_and_mark_text_in_range(None, "日本", Some(2..2), window, cx);
        let bounds = p.bounds_for_range(0..1, pane, window, cx);
        eprintln!("app: script: candidate window would sit at {bounds:?}");
    });
    pause(150).await;
    let _ = this.update_in(cx, |p, window, cx| {
        p.replace_text_in_range(None, "日本", window, cx);
    });
    pause(150).await;

    // A wheel step over the page.
    let _ = cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::Scroll(ScrollEvent {
                position: at(400.0, 400.0),
                delta: ScrollDelta::Pixels(point(px(0.0), px(-120.0))),
                ..Default::default()
            }),
            cx,
        )
    });
    pause(150).await;

    // A click on the overlay must not reach the page.
    if let Some(swatch) = swatch {
        eprintln!("app: script: moving onto the overlay; the page should hear PointerLeave");
        let _ = cx.update(|window, cx| {
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: swatch.center(),
                    pressed_button: None,
                    modifiers: Default::default(),
                }),
                cx,
            )
        });
        pause(150).await;
        eprintln!(
            "app: script: clicking the overlay at {:?}; no 'pane got mouse down' line should follow",
            swatch.center()
        );
        click(cx, swatch.center());
    }
    pause(300).await;
    eprintln!("app: script: done");
}

fn click(cx: &mut gpui::AsyncWindowContext, position: Point<Pixels>) {
    let _ = cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
    });
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
