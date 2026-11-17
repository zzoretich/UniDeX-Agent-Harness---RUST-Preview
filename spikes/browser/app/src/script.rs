//! Scripted scenarios: input checks and measurement windows, driven through the app's own
//! event paths. Throwaway.
//!
//! Pointer, wheel and key events go through GPUI's own dispatch (`Window::dispatch_event`,
//! `Window::dispatch_keystroke`: hit testing, focus, element listeners, the input handler), the
//! path real events take after the platform layer. Input-method compositions go through GPUI's
//! macOS view as an input method's calls do (`mac_ime.rs`). Results are read back from the page
//! with DevTools `Runtime.evaluate`.
//!
//! Scenarios (`--scenario`):
//! - `input`: every input check on `pages/form.html`, one `check:` line each, then a renderer
//!   crash and its recovery.
//! - `scroll`: wheel events every 8 ms for `--measure-seconds` over `pages/long.html`.
//! - `anim`: no input, `pages/anim.html` animating; the overlay over the moving page.
//! - `idle`: no input at all (use with `--quiet-ui`).
//! - `latency`: 20 keys typed into a caret-less field; key-to-frame time for each.
//! - `memory`: waits for every tab to load, settles, samples memory.

use crate::stats::{self, ProcSample};
use crate::{Args, Pane};
use gpui::{
    AsyncWindowContext, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, PlatformInput, Point, ScrollDelta, ScrollEvent, WeakEntity, point, px,
};
use serde_json::{Value, json};
use spike_protocol::usage;
use spike_protocol::view::BrowserView as _;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

type This = WeakEntity<Pane>;
type Cx = AsyncWindowContext;

async fn sleep(cx: &Cx, ms: u64) {
    cx.background_executor()
        .timer(Duration::from_millis(ms))
        .await;
}

async fn wait_for(this: &This, cx: &mut Cx, timeout_ms: u64, pred: impl Fn(&Pane) -> bool) -> bool {
    let start = Instant::now();
    loop {
        if this.update(cx, |p, _| pred(p)).unwrap_or(false) {
            return true;
        }
        if start.elapsed() > Duration::from_millis(timeout_ms) {
            return false;
        }
        sleep(cx, 10).await;
    }
}

fn events_len(this: &This, cx: &mut Cx) -> usize {
    this.update(cx, |p, _| p.log.borrow().events.len())
        .unwrap_or(0)
}

async fn wait_event(
    this: &This,
    cx: &mut Cx,
    since: usize,
    needle: &str,
    timeout_ms: u64,
) -> Option<String> {
    let start = Instant::now();
    loop {
        let found = this
            .update(cx, |p, _| {
                p.log
                    .borrow()
                    .events
                    .iter()
                    .skip(since)
                    .find(|(_, e)| e.contains(needle))
                    .map(|(_, e)| e.clone())
            })
            .ok()
            .flatten();
        if found.is_some() {
            return found;
        }
        if start.elapsed() > Duration::from_millis(timeout_ms) {
            return None;
        }
        sleep(cx, 10).await;
    }
}

async fn devtools(this: &This, cx: &mut Cx, method: &str, params: Value) -> Option<Value> {
    let id = this
        .update(cx, |p, _| p.browser.borrow_mut().devtools(method, params))
        .ok()?;
    wait_for(this, cx, 5000, |p| p.log.borrow().replies.contains_key(&id)).await;
    this.update(cx, |p, _| p.log.borrow_mut().replies.remove(&id))
        .ok()
        .flatten()
}

/// `Runtime.evaluate` with `returnByValue`; the value, or `Null`.
async fn eval(this: &This, cx: &mut Cx, expression: &str) -> Value {
    let reply = devtools(
        this,
        cx,
        "Runtime.evaluate",
        json!({ "expression": expression, "returnByValue": true, "awaitPromise": true }),
    )
    .await;
    reply
        .and_then(|r| r.get("result")?.get("result")?.get("value").cloned())
        .unwrap_or(Value::Null)
}

async fn navigate(this: &This, cx: &mut Cx, url: &str) -> bool {
    let since = events_len(this, cx);
    let reply = devtools(this, cx, "Page.navigate", json!({ "url": url })).await;
    reply.is_some()
        && wait_event(this, cx, since, "load-end", 15000)
            .await
            .is_some()
}

fn origin(this: &This, cx: &mut Cx) -> Point<Pixels> {
    this.update(cx, |p, _| p.paint.borrow().pane.map(|b| b.origin))
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn dispatch(cx: &mut Cx, event: PlatformInput) {
    let _ = cx.update(|window, cx| window.dispatch_event(event, cx));
}

fn move_to(cx: &mut Cx, at: Point<Pixels>) {
    dispatch(
        cx,
        PlatformInput::MouseMove(MouseMoveEvent {
            position: at,
            pressed_button: None,
            modifiers: Default::default(),
        }),
    );
}

fn click(cx: &mut Cx, at: Point<Pixels>) {
    press(cx, at);
    release(cx, at);
}

/// A click with the button held for a moment, as a person clicks (a select popup opens on the
/// press and must still be open at the release).
async fn click_held(cx: &mut Cx, at: Point<Pixels>) {
    press(cx, at);
    sleep(cx, 120).await;
    release(cx, at);
}

fn press(cx: &mut Cx, at: Point<Pixels>) {
    move_to(cx, at);
    dispatch(
        cx,
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }),
    );
}

fn release(cx: &mut Cx, at: Point<Pixels>) {
    dispatch(
        cx,
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Default::default(),
            click_count: 1,
        }),
    );
}

fn wheel(cx: &mut Cx, at: Point<Pixels>, dy: f32) {
    dispatch(
        cx,
        PlatformInput::Scroll(ScrollEvent {
            position: at,
            delta: ScrollDelta::Pixels(point(px(0.0), px(dy))),
            ..Default::default()
        }),
    );
}

fn parse(keystroke: &str) -> Result<Keystroke, gpui::InvalidKeystrokeError> {
    let os = if cfg!(target_os = "macos") {
        gpui::OperatingSystem::Mac
    } else if cfg!(windows) {
        gpui::OperatingSystem::Windows
    } else {
        gpui::OperatingSystem::Linux
    };
    Keystroke::parse_with_os(keystroke, os)
}

fn key(cx: &mut Cx, keystroke: &str) {
    match parse(keystroke) {
        Ok(k) => {
            let _ = cx.update(|window, cx| window.dispatch_keystroke(k, cx));
        }
        Err(error) => eprintln!("app: script: bad keystroke {keystroke}: {error:#}"),
    }
}

async fn type_keys(cx: &mut Cx, keys: &[&str]) {
    for k in keys {
        key(cx, k);
        sleep(cx, 40).await;
    }
}

fn chars(text: &str) -> Vec<String> {
    text.chars()
        .map(|c| match c {
            ' ' => "space".to_string(),
            c => c.to_string(),
        })
        .collect()
}

async fn type_text(cx: &mut Cx, text: &str) {
    let keys = chars(text);
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    type_keys(cx, &refs).await;
}

#[derive(Default)]
struct Checks {
    passed: u32,
    failed: u32,
}

impl Checks {
    fn check(&mut self, name: &str, ok: bool, detail: impl std::fmt::Display) {
        if ok {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        eprintln!(
            "check: {name} {} {detail}",
            if ok { "PASS" } else { "FAIL" }
        );
    }
}

/// The start of a measurement window.
struct Mark {
    at: Instant,
    tree: Vec<ProcSample>,
    app: usage::Usage,
    received: u64,
    superseded: u64,
    copy_ns: u64,
}

fn begin(this: &This, cx: &mut Cx) -> Option<Mark> {
    this.update(cx, |p, _| {
        let mut s = p.paint.borrow_mut();
        s.recording = true;
        s.rec_painted_at.clear();
        s.rec_paint_ns.clear();
        s.rec_latency_ns.clear();
        s.rec_ui_frame_ns.clear();
        s.rec_window_frames = 0;
        drop(s);
        let tree = p.tree.sample(p.helper_pid as i32);
        Mark {
            at: Instant::now(),
            tree,
            app: usage::current(),
            received: p.counters.received_total(),
            superseded: p.counters.superseded.load(Ordering::Relaxed),
            copy_ns: p.counters.copy_ns_total.load(Ordering::Relaxed),
        }
    })
    .ok()
}

fn end(this: &This, cx: &mut Cx, scenario: &str, mark: Mark, extra: Value) {
    let _ = this.update(cx, |p, _| {
        let seconds = mark.at.elapsed().as_secs_f64();
        let mut s = p.paint.borrow_mut();
        s.recording = false;
        let ms = |v: &[u64]| v.iter().map(|n| *n as f64 / 1e6).collect::<Vec<f64>>();
        let intervals: Vec<f64> = s
            .rec_painted_at
            .windows(2)
            .map(|w| (w[1] - w[0]).as_secs_f64() * 1e3)
            .collect();
        let frames = s.rec_painted_at.len();
        let paint_ms = ms(&s.rec_paint_ns);
        let latency_ms = ms(&s.rec_latency_ns);
        let ui_ms = ms(&s.rec_ui_frame_ns);
        let window_frames = s.rec_window_frames;
        drop(s);
        let tree = p.tree.sample(p.helper_pid as i32);
        let app = usage::current();
        let app_cpu = app.cpu_percent_since(&mark.app, mark.at.elapsed());
        let received = p.counters.received_total() - mark.received;
        let superseded = p.counters.superseded.load(Ordering::Relaxed) - mark.superseded;
        let copy_ns = p.counters.copy_ns_total.load(Ordering::Relaxed) - mark.copy_ns;
        let processes: Vec<Value> = stats::cpu_between(&mark.tree, &tree, seconds)
            .into_iter()
            .map(|(s, cpu)| {
                json!({
                    "pid": s.pid,
                    "kind": s.kind,
                    "cpu_pct": stats::round2(cpu),
                    "footprint_mb": stats::round2(s.footprint as f64 / 1e6),
                    "resident_mb": stats::round2(s.resident as f64 / 1e6),
                    "sandboxed": s.sandboxed,
                })
            })
            .collect();
        let tree_cpu: f64 = stats::cpu_between(&mark.tree, &tree, seconds)
            .iter()
            .map(|(_, c)| c)
            .sum();
        let tree_footprint: u64 = tree.iter().map(|s| s.footprint).sum();
        let log = p.log.borrow();
        let since_spawn =
            |t: Option<Instant>| t.map(|t| stats::round2((t - p.spawned_at).as_secs_f64() * 1e3));
        let summary = json!({
            "scenario": scenario,
            "seconds": stats::round2(seconds),
            "frames_painted": frames,
            "fps_painted": stats::round2(frames as f64 / seconds),
            "frame_interval_ms": stats::summary(&intervals),
            "paint_image_ms": stats::summary(&paint_ms),
            "helper_paint_to_app_paint_ms": stats::summary(&latency_ms),
            "ui_thread_ms_per_window_frame": stats::summary(&ui_ms),
            "window_frames": window_frames,
            "frames_received": received,
            "frames_superseded": superseded,
            "copy_out_avg_ms": stats::round2(copy_ns as f64 / 1e6 / received.max(1) as f64),
            "app_cpu_pct": stats::round2(app_cpu),
            "app_footprint_mb": stats::round2(app.footprint_bytes as f64 / 1e6),
            "app_resident_mb": stats::round2(app.resident_bytes as f64 / 1e6),
            "helper_tree_cpu_pct": stats::round2(tree_cpu),
            "helper_tree_footprint_mb": stats::round2(tree_footprint as f64 / 1e6),
            "processes": processes,
            "startup_ms": {
                "ready": since_spawn(log.ready_at),
                "first_frame_received": since_spawn(log.first_frame_at),
                "first_frame_painted": since_spawn(p.paint.borrow().first_painted),
            },
            "extra": extra,
        });
        eprintln!("app: summary {summary}");
    });
}

fn quit(this: &This, cx: &mut Cx) {
    let _ = this.update(cx, |p, cx| {
        p.browser.borrow_mut().shutdown();
        cx.quit();
    });
}

pub async fn run(scenario: String, args: Args, this: This, cx: &mut Cx) {
    let started = wait_for(&this, cx, 30000, |p| {
        let log = p.log.borrow();
        log.ready_at.is_some() && log.first_frame_at.is_some()
    })
    .await;
    if !started {
        eprintln!("app: script: the helper did not become ready with a frame");
        quit(&this, cx);
        return;
    }
    // The start page (given on the command line) has loaded.
    if wait_event(&this, cx, 0, "load-end", 20000).await.is_none() {
        eprintln!("app: script: the start page did not finish loading");
    }
    sleep(cx, 300).await;
    eprintln!("app: script: {scenario}");
    match scenario.as_str() {
        "input" => input(&this, cx).await,
        "scroll" => scroll(&this, cx, args.measure_seconds).await,
        "anim" => anim(&this, cx, args.measure_seconds).await,
        "idle" => watch(&this, cx, "idle", 3000, args.measure_seconds, json!({})).await,
        "memory" => memory(&this, cx, &args).await,
        "latency" => latency(&this, cx).await,
        #[cfg(target_os = "macos")]
        "ime" => {
            let mut c = Checks::default();
            let o = origin(&this, cx);
            ime(&this, cx, &mut c, o + point(px(590.0), px(160.0))).await;
        }
        other => eprintln!("app: script: unknown scenario {other}"),
    }
    eprintln!("app: script: done");
    quit(&this, cx);
}

async fn watch(this: &This, cx: &mut Cx, name: &str, settle_ms: u64, seconds: u64, extra: Value) {
    sleep(cx, settle_ms).await;
    let Some(mark) = begin(this, cx) else { return };
    sleep(cx, seconds * 1000).await;
    end(this, cx, name, mark, extra);
}

async fn memory(this: &This, cx: &mut Cx, args: &Args) {
    let tabs = args.tabs.max(1) as usize;
    let all_loaded = wait_for(this, cx, 60000, |p| {
        p.log
            .borrow()
            .events
            .iter()
            .filter(|(_, e)| e.starts_with("load-end browser"))
            .count()
            >= tabs
    })
    .await;
    let nodes = eval(this, cx, "document.getElementsByTagName('*').length").await;
    watch(
        this,
        cx,
        "memory",
        5000,
        args.measure_seconds,
        json!({ "tabs": tabs, "all_tabs_loaded": all_loaded, "visible_tab_dom_nodes": nodes }),
    )
    .await;
}

async fn scroll(this: &This, cx: &mut Cx, seconds: u64) {
    let o = origin(this, cx);
    let at = o + point(px(640.0), px(400.0));
    move_to(cx, at);
    sleep(cx, 1000).await;
    eval(this, cx, "window.__startRaf()").await;
    let Some(mark) = begin(this, cx) else { return };
    let until = Instant::now() + Duration::from_secs(seconds);
    let mut events = 0u64;
    while Instant::now() < until {
        // A trackpad-like stream: 3 px every 8 ms (375 px a second) towards the page's end.
        wheel(cx, at, -3.0);
        events += 1;
        sleep(cx, 8).await;
    }
    let page = eval(this, cx, "window.__stopRaf()").await;
    let scrolled_down = page["scrollY"].as_f64().is_some_and(|y| y > 0.0);
    end(
        this,
        cx,
        "scroll",
        mark,
        json!({ "wheel_events": events, "page": page, "wheel_sign_ok": scrolled_down }),
    );
}

async fn anim(this: &This, cx: &mut Cx, seconds: u64) {
    sleep(cx, 1500).await;
    // The overlay over a moving page: two readbacks half a second apart.
    let first = this
        .update_in(cx, |p, window, _| {
            p.selfcheck(window).map(|i| (i, p.paint.borrow().pane))
        })
        .ok()
        .flatten();
    sleep(cx, 500).await;
    let second = this
        .update_in(cx, |p, window, _| p.selfcheck(window))
        .ok()
        .flatten();
    let mut overlay = json!(null);
    if let (Some((a, Some(pane))), Some(b)) = (first, second) {
        let scale = a.width() as f32 / f32::from(pane.size.width + pane.origin.x).max(1.0);
        // The canvas area of the animation page.
        let region = |img: &image::RgbaImage| {
            let mut sum = 0u64;
            for y in 0..100u32 {
                for x in 0..300u32 {
                    let px = ((f32::from(pane.origin.x) + 340.0) * scale) as u32 + x;
                    let py = ((f32::from(pane.origin.y) + 400.0) * scale) as u32 + y;
                    if let Some(p) = img.get_pixel_checked(px, py) {
                        sum = sum
                            .wrapping_mul(31)
                            .wrapping_add(p.0[0] as u64 + p.0[1] as u64);
                    }
                }
            }
            sum
        };
        overlay = json!({ "page_changed_between_readbacks": region(&a) != region(&b) });
    }
    let before = eval(this, cx, "window.__frames")
        .await
        .as_f64()
        .unwrap_or(0.0);
    let Some(mark) = begin(this, cx) else { return };
    sleep(cx, seconds * 1000).await;
    let after = eval(this, cx, "window.__frames")
        .await
        .as_f64()
        .unwrap_or(0.0);
    let secs = mark.at.elapsed().as_secs_f64();
    end(
        this,
        cx,
        "anim",
        mark,
        json!({ "page_raf_fps": stats::round2((after - before) / secs), "overlay": overlay }),
    );
}

async fn latency(this: &This, cx: &mut Cx) {
    let o = origin(this, cx);
    click(cx, o + point(px(220.0), px(40.0)));
    sleep(cx, 600).await;
    let scale = this
        .update_in(cx, |_, window, _| window.scale_factor())
        .unwrap_or(1.0);
    // The field's inside, in device pixels (the field sits at 20, 20, 400 x 40 logical).
    let rect = (
        (24.0 * scale) as u32,
        (24.0 * scale) as u32,
        (392.0 * scale) as u32,
        (32.0 * scale) as u32,
    );
    let _ = this.update(cx, |p, _| p.watch.lock().unwrap().rect = Some(rect));
    sleep(cx, 300).await;
    let Some(mark) = begin(this, cx) else { return };
    let mut received = Vec::new();
    let mut painted = Vec::new();
    let mut misses = 0;
    for c in "abcdefghijklmnopqrst".chars() {
        let keystroke = parse(&c.to_string()).expect("a letter");
        let _ = this.update(cx, |p, _| {
            let mut w = p.watch.lock().unwrap();
            w.hit = None;
            w.painted = None;
            w.armed = Some((Instant::now(), w.last_hash));
        });
        // Dispatched at window level, outside any update of the pane, as a real key arrives.
        let t0 = cx
            .update(|window, cx| {
                let t0 = Instant::now();
                window.dispatch_keystroke(keystroke, cx);
                t0
            })
            .ok();
        let Some(t0) = t0 else { break };
        let ok = wait_for(this, cx, 1000, |p| {
            p.watch.lock().unwrap().painted.is_some()
        })
        .await;
        if ok {
            let (hit, done) = this
                .update(cx, |p, _| {
                    let w = p.watch.lock().unwrap();
                    (w.hit.map(|h| h.0), w.painted)
                })
                .unwrap_or((None, None));
            if let (Some(hit), Some(done)) = (hit, done) {
                received.push((hit - t0).as_secs_f64() * 1e3);
                painted.push((done - t0).as_secs_f64() * 1e3);
            }
        } else {
            misses += 1;
        }
        sleep(cx, 250).await;
    }
    let value = eval(this, cx, "document.getElementById('lat').value").await;
    end(
        this,
        cx,
        "latency",
        mark,
        json!({
            "keys": 20,
            "misses": misses,
            "typed_value": value,
            "key_to_frame_in_app_ms": stats::summary(&received),
            "key_to_frame_painted_ms": stats::summary(&painted),
            "note": "ends when paint_image is called; presentation adds up to one display frame",
        }),
    );
}

async fn input(this: &This, cx: &mut Cx) {
    let mut c = Checks::default();
    let o = origin(this, cx);
    let at = |x: f32, y: f32| o + point(px(x), px(y));
    let (scale, pane_size) = this
        .update_in(cx, |p, window, _| {
            (window.scale_factor(), p.paint.borrow().pane.map(|b| b.size))
        })
        .unwrap_or((1.0, None));

    // Scale: the frame is in device pixels and the page draws one device pixel per checker cell.
    let frame = this.update(cx, |p, _| p.view_frame.clone()).ok().flatten();
    if let (Some(frame), Some(size)) = (frame, pane_size) {
        let device = frame.image.size(0);
        let want = (
            (f32::from(size.width) * scale).round() as i32,
            (f32::from(size.height) * scale).round() as i32,
        );
        c.check(
            "frame-in-device-pixels",
            (device.width.0, device.height.0) == want,
            format!(
                "frame {}x{} for a pane of {:?} at scale {scale}",
                device.width.0, device.height.0, size
            ),
        );
        let bytes = frame.image.as_bytes(0).unwrap_or_default();
        let row = ((200.0 * scale) as usize + 4) * device.width.0 as usize * 4;
        let values: Vec<u8> = (0..8)
            .map(|i| {
                let x = (440.0 * scale) as usize + 4 + i;
                bytes.get(row + x * 4).copied().unwrap_or(128)
            })
            .collect();
        let alternating = values
            .windows(2)
            .all(|w| (w[0] as i32 - w[1] as i32).abs() > 200);
        c.check("checker-sharp-in-frame", alternating, format!("{values:?}"));
    }
    let dpr = eval(this, cx, "window.devicePixelRatio").await;
    c.check(
        "page-device-pixel-ratio",
        dpr.as_f64()
            .is_some_and(|d| (d as f32 - scale).abs() < 0.01),
        format!("devicePixelRatio {dpr} at scale {scale}"),
    );
    let readback = this
        .update_in(cx, |p, window, _| p.selfcheck(window))
        .ok()
        .flatten();
    if let Some(img) = readback {
        let y = ((f32::from(o.y) + 200.0) * scale) as u32 + 4;
        let values: Vec<u8> = (0..8)
            .map(|i| {
                let x = ((f32::from(o.x) + 440.0) * scale) as u32 + 4 + i;
                img.get_pixel_checked(x, y).map_or(128, |p| p.0[0])
            })
            .collect();
        let alternating = values
            .windows(2)
            .all(|w| (w[0] as i32 - w[1] as i32).abs() > 200);
        c.check(
            "checker-sharp-in-window",
            alternating,
            format!("{values:?}"),
        );
        let swatch = this
            .update(cx, |p, _| p.paint.borrow().swatch)
            .ok()
            .flatten();
        if let Some(s) = swatch {
            let x = (f32::from(s.center().x) * scale) as u32;
            let y = (f32::from(s.center().y) * scale) as u32;
            let px = img.get_pixel_checked(x, y).map(|p| p.0);
            c.check(
                "overlay-above-page",
                px.is_some_and(|p| p[0] > 245 && p[1] < 10 && p[2] > 245),
                format!("swatch pixel {px:?}"),
            );
        }
    }

    // Cursor shapes reported by the page.
    let since = events_len(this, cx);
    move_to(cx, at(320.0, 268.0));
    let hand = wait_event(this, cx, since, "cursor PointingHand", 2000).await;
    c.check("cursor-hand-over-link", hand.is_some(), "");
    let since = events_len(this, cx);
    move_to(cx, at(220.0, 100.0));
    let ibeam = wait_event(this, cx, since, "cursor IBeam", 2000).await;
    c.check("cursor-ibeam-over-field", ibeam.is_some(), "");
    let since = events_len(this, cx);
    move_to(cx, at(700.0, 380.0));
    let arrow = wait_event(this, cx, since, "cursor Arrow", 2000).await;
    c.check("cursor-arrow-elsewhere", arrow.is_some(), "");

    // Click and focus.
    click(cx, at(220.0, 100.0));
    sleep(cx, 200).await;
    let focus = eval(
        this,
        cx,
        "document.activeElement.id + ',' + document.hasFocus()",
    )
    .await;
    c.check("click-focuses-field", focus == json!("name,true"), &focus);

    // Typing, Backspace, arrows.
    type_text(cx, "hello world").await;
    sleep(cx, 150).await;
    let value = eval(this, cx, "document.getElementById('name').value").await;
    c.check("typing", value == json!("hello world"), &value);
    type_keys(cx, &["backspace"; 5]).await;
    type_keys(cx, &["left"]).await;
    // Shift-1 as a real US-layout key event carries it: key "1", Shift, character "!". (GPUI's
    // keystroke parser would make the character "1"; it does not know keyboard layouts.)
    let bang = Keystroke {
        modifiers: gpui::Modifiers {
            shift: true,
            ..Default::default()
        },
        key: "1".into(),
        key_char: Some("!".into()),
    };
    let _ = cx.update(|window, cx| window.dispatch_keystroke(bang, cx));
    sleep(cx, 150).await;
    let value = eval(this, cx, "document.getElementById('name').value").await;
    c.check(
        "backspace-and-left-arrow",
        value == json!("hello! "),
        &value,
    );

    // Command shortcuts: select all, copy, paste into another field. (This puts the test text
    // on the system clipboard.)
    type_keys(cx, &["cmd-a", "cmd-c"]).await;
    click(cx, at(590.0, 100.0));
    sleep(cx, 150).await;
    type_keys(cx, &["cmd-v"]).await;
    sleep(cx, 200).await;
    let value = eval(this, cx, "document.getElementById('other').value").await;
    c.check("cmd-a-cmd-c-cmd-v", value == json!("hello! "), &value);

    // Tab moves focus to the next field (the text area).
    type_keys(cx, &["tab"]).await;
    sleep(cx, 150).await;
    let active = eval(this, cx, "document.activeElement.id").await;
    c.check("tab-moves-focus", active == json!("area"), &active);
    type_text(cx, "one").await;
    type_keys(cx, &["enter"]).await;
    type_text(cx, "two").await;
    type_keys(cx, &["up"]).await;
    type_text(cx, "Z").await;
    sleep(cx, 150).await;
    let value = eval(this, cx, "document.getElementById('area').value").await;
    c.check(
        "enter-and-up-arrow-in-textarea",
        value == json!("oneZ\ntwo"),
        &value,
    );

    // Input-method compositions through GPUI's macOS text-input client.
    #[cfg(target_os = "macos")]
    ime(this, cx, &mut c, at(590.0, 160.0)).await;

    // Wheel reaches the page with the expected sign.
    let since = events_len(this, cx);
    let _ = since;
    wheel(cx, at(400.0, 380.0), -120.0);
    sleep(cx, 200).await;
    let wheel_log = eval(
        this,
        cx,
        "window.__log.filter(e => e.startsWith('wheel:')).join(' ')",
    )
    .await;
    c.check(
        "wheel-reaches-page-scrolling-down",
        wheel_log.as_str().is_some_and(|s| s.contains("wheel:120")),
        &wheel_log,
    );

    // The native select popup, drawn by the app from the engine's popup frames.
    let since = events_len(this, cx);
    click_held(cx, at(120.0, 268.0)).await;
    let shown = wait_event(this, cx, since, "popup show=true", 2000).await;
    let popup_painted = wait_for(this, cx, 2000, |p| p.paint.borrow().popup_painted > 0).await;
    c.check(
        "select-popup-shown-and-painted",
        shown.is_some() && popup_painted,
        shown.unwrap_or_default(),
    );
    type_keys(cx, &["down", "enter"]).await;
    let hidden = wait_event(this, cx, since, "popup show=false", 2000).await;
    let value = eval(this, cx, "document.getElementById('pick').value").await;
    c.check(
        "select-popup-keyboard-choice",
        value == json!("two") && hidden.is_some(),
        format!("value {value}, hidden {}", hidden.is_some()),
    );

    // Focus out and back in.
    let _ = this.update_in(cx, |p, window, cx| window.focus(&p.other_focus, cx));
    sleep(cx, 200).await;
    let has = eval(this, cx, "document.hasFocus()").await;
    c.check("focus-out", has == json!(false), &has);
    click(cx, at(220.0, 100.0));
    sleep(cx, 200).await;
    let has = eval(this, cx, "document.hasFocus()").await;
    c.check("focus-back-in", has == json!(true), &has);

    // Pane resize: a sidebar takes 240 logical pixels from the pane.
    let before = eval(this, cx, "innerWidth").await.as_f64().unwrap_or(0.0);
    let _ = this.update(cx, |p, cx| {
        p.sidebar = true;
        cx.notify();
    });
    let want = ((before - 240.0) * scale as f64).round() as i32;
    let resized = wait_for(this, cx, 3000, |p| {
        p.view_frame
            .as_ref()
            .is_some_and(|f| f.image.size(0).width.0 == want)
    })
    .await;
    let after = eval(this, cx, "innerWidth").await.as_f64().unwrap_or(0.0);
    c.check(
        "pane-resize",
        resized && (before - after - 240.0).abs() < 1.0,
        format!("innerWidth {before} -> {after}, frame width reached {want}: {resized}"),
    );
    let _ = this.update(cx, |p, cx| {
        p.sidebar = false;
        cx.notify();
    });
    sleep(cx, 300).await;

    // What the trial refuses and reports instead (the product would ask or decide).
    for (id, expect) in [
        ("b-download", "refused: download"),
        ("b-file", "refused: file dialog"),
        ("b-popup", "refused: popup window"),
        ("b-geo", "refused: permission prompt"),
        ("b-media", "refused: media access"),
        ("b-alert", "suppressed: page dialog"),
    ] {
        let rect = eval(
            this,
            cx,
            &format!("(r => [r.x + r.width / 2, r.y + r.height / 2])(document.getElementById('{id}').getBoundingClientRect())"),
        )
        .await;
        let (Some(x), Some(y)) = (rect[0].as_f64(), rect[1].as_f64()) else {
            c.check(&format!("refusal-{id}"), false, "no button rect");
            continue;
        };
        let since = events_len(this, cx);
        click(cx, at(x as f32, y as f32));
        let seen = wait_event(this, cx, since, expect, 2000).await;
        if id == "b-media" && seen.is_none() {
            // The engine may fail the request before any permission is asked.
            sleep(cx, 300).await;
            let page = eval(
                this,
                cx,
                "window.__log.filter(e => e.startsWith('media:')).join(' ')",
            )
            .await;
            let denied = page.as_str().is_some_and(|s| s.contains("media:denied"));
            c.check(
                "refusal-b-media",
                denied,
                format!("no prompt reached the helper; the page saw {page}"),
            );
            continue;
        }
        c.check(
            &format!("refusal-{id}"),
            seen.is_some(),
            seen.unwrap_or_default(),
        );
    }
    sleep(cx, 300).await;
    let page_log = eval(
        this,
        cx,
        "window.__log.filter(e => /^(geo|media|alert|open):/.test(e)).join(' ')",
    )
    .await;
    eprintln!("app: page saw: {page_log}");

    // Codecs: CEF's published builds carry no proprietary codecs.
    let codecs = eval(
        this,
        cx,
        "(() => { const v = document.createElement('video'), r = {}; for (const t of [\
          'video/mp4; codecs=\"avc1.42E01E\"', 'audio/mp4; codecs=\"mp4a.40.2\"', 'video/mp4; codecs=\"hvc1.1.6.L93.B0\"', \
          'video/webm; codecs=\"vp9\"', 'video/webm; codecs=\"av01.0.05M.08\"', 'audio/webm; codecs=\"opus\"']) \
          r[t] = v.canPlayType(t) || 'no'; return r; })()",
    )
    .await;
    let h264 = codecs["video/mp4; codecs=\"avc1.42E01E\""].as_str() == Some("no");
    c.check("codecs-h264-absent-as-expected", h264, &codecs);

    // DevTools without a port: navigate, then evaluate.
    let navigated = navigate(
        this,
        cx,
        "data:text/html,<title>devtools-check</title><p>navigated</p>",
    )
    .await;
    let title = eval(this, cx, "document.title").await;
    c.check(
        "devtools-navigate-and-evaluate",
        navigated && title == json!("devtools-check"),
        &title,
    );

    // A renderer crash and the helper's automatic reload.
    let since = events_len(this, cx);
    let taken_before = this
        .update(cx, |p, _| p.counters.received_total())
        .unwrap_or(0);
    let _ = this.update(cx, |p, _| {
        p.browser.borrow_mut().devtools("Page.crash", json!({}));
    });
    let crashed = wait_event(this, cx, since, "renderer-terminated", 5000).await;
    let reloaded = wait_event(this, cx, since, "load-end", 10000).await;
    let answer = eval(this, cx, "6 * 7").await;
    let frames_after = wait_for(this, cx, 3000, |p| {
        p.counters.received_total() > taken_before
    })
    .await;
    c.check(
        "renderer-crash-recovery",
        crashed.is_some() && reloaded.is_some() && answer == json!(42) && frames_after,
        format!(
            "{} / {} / {answer} / frames {frames_after}",
            crashed.unwrap_or_default(),
            reloaded.unwrap_or_default()
        ),
    );

    eprintln!(
        "app: input checks: {} passed, {} failed",
        c.passed, c.failed
    );
}

#[cfg(target_os = "macos")]
async fn ime(this: &This, cx: &mut Cx, c: &mut Checks, field: Point<Pixels>) {
    use crate::mac_ime::NativeView;
    click(cx, field);
    sleep(cx, 200).await;
    let Some(view) = this
        .update_in(cx, |_, window, _| NativeView::of(window))
        .ok()
        .flatten()
    else {
        c.check("ime-native-view", false, "no NSView");
        return;
    };
    let focused = eval(this, cx, "document.activeElement.id").await;
    eprintln!("app: ime: page focus on {focused}");
    // Japanese: "ni" -> に -> 日本, committed.
    let since = events_len(this, cx);
    view.set_marked_text("に", 1..1);
    let bounds = wait_event(this, cx, since, "ime-bounds", 2000).await;
    view.set_marked_text("日本", 2..2);
    sleep(cx, 100).await;
    let during = eval(this, cx, "document.getElementById('ime').value").await;
    let rect = view.first_rect(0, 1);
    view.insert_text("日本");
    sleep(cx, 150).await;
    let value = eval(this, cx, "document.getElementById('ime').value").await;
    let events = eval(
        this,
        cx,
        "window.__log.filter(e => e.startsWith('composition')).join(' ')",
    )
    .await;
    c.check(
        "ime-japanese-compose-and-commit",
        value == json!("日本")
            && events.as_str().is_some_and(|e| {
                e.contains("compositionstart:ime") && e.contains("compositionend:ime:日本")
            }),
        format!("during {during}, value {value}, events {events}"),
    );
    c.check(
        "ime-candidate-rect-from-engine",
        bounds.is_some() && rect.2 > 0.0 && rect.3 > 0.0,
        format!("screen rect {rect:?}"),
    );
    // Chinese pinyin: n -> ni -> 你, committed.
    view.set_marked_text("n", 1..1);
    sleep(cx, 50).await;
    view.set_marked_text("ni", 2..2);
    sleep(cx, 50).await;
    view.set_marked_text("你", 1..1);
    sleep(cx, 50).await;
    view.insert_text("你");
    sleep(cx, 150).await;
    let value = eval(this, cx, "document.getElementById('ime').value").await;
    c.check(
        "ime-chinese-compose-and-commit",
        value == json!("日本你"),
        &value,
    );
    // Cancel: an emptied composition leaves the field as it was.
    view.set_marked_text("か", 1..1);
    sleep(cx, 50).await;
    view.set_marked_text("", 0..0);
    sleep(cx, 150).await;
    let value = eval(this, cx, "document.getElementById('ime').value").await;
    c.check("ime-cancel", value == json!("日本你"), &value);
    // Unmark: the marked text is kept.
    view.set_marked_text("ん", 1..1);
    sleep(cx, 50).await;
    view.unmark_text();
    sleep(cx, 150).await;
    let value = eval(this, cx, "document.getElementById('ime').value").await;
    c.check("ime-unmark-keeps-text", value == json!("日本你ん"), &value);
}
