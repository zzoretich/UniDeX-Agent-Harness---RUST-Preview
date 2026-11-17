//! The trial app: one window with three solid blocks (red, green, blue), a state block (grey until the button is
//! clicked, then orange), a line of text, a "Count" button and a "Clicks: N" label, at the fixed places
//! `render_ci::*` describes.
//!
//! Modes (one of them is required, so a stray launch never lingers):
//!   --scenario <dir>   wait for the window to draw, capture one frame in-app, check it, write `<dir>/frame-inapp.png`
//!                      and `<dir>/scenario.json`, write `<dir>/ready`, hold the window open for `--hold-ms` (so CI
//!                      can capture the virtual display too), then exit 0 if the check passed, 1 if not.
//!   --automation       (feature `automation` only) open the automation socket and print its endpoint on stdout as
//!                      `RENDER_CI_ENDPOINT=<path>`; `render-ci-client` drives the rest.
//! Always: `--exit-after <seconds>` (default 120) ends the process whatever happens.

#[cfg(feature = "automation")]
mod automation;
mod capture;

use gpui::{
    App, Bounds, Context, Entity, Pixels, Role, TitlebarOptions, Window, WindowBounds,
    WindowOptions, canvas, div, prelude::*, px, rgb, size, text,
};
use render_ci::*;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// One element the app tagged for automation, with the bounds GPUI laid it out at in the last frame. This is the
/// app's own bookkeeping (a `canvas` probe inside each tagged element records its prepaint bounds), not something
/// GPUI enumerates for us; GPUI's own tree is the AccessKit one (see `automation`).
#[derive(Clone, Debug)]
pub struct Tagged {
    pub id: &'static str,
    pub role: &'static str,
    pub label: String,
    pub bounds: Bounds<Pixels>,
}

pub type Registry = Rc<RefCell<Vec<Tagged>>>;

pub struct TrialView {
    pub clicks: u32,
    /// The clicks value as of the last render, so a client can wait for the screen to catch up.
    pub rendered_clicks: u32,
    pub render_count: u64,
    pub first_render: Option<Instant>,
    pub registry: Registry,
}

/// An invisible, absolutely positioned child that records its parent's bounds during prepaint.
fn probe(
    registry: &Registry,
    id: &'static str,
    role: &'static str,
    label: String,
) -> impl IntoElement {
    let registry = registry.clone();
    canvas(
        move |bounds, _window, _cx| {
            registry.borrow_mut().push(Tagged {
                id,
                role,
                label,
                bounds,
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn block(registry: &Registry, id: &'static str, at: (f32, f32), colour: u32) -> impl IntoElement {
    div()
        .absolute()
        .left(px(at.0))
        .top(px(at.1))
        .size(px(BLOCK_SIDE))
        .bg(rgb(colour))
        .child(probe(registry, id, "block", format!("#{colour:06x}")))
}

impl Render for TrialView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_count += 1;
        self.rendered_clicks = self.clicks;
        if self.first_render.is_none() {
            self.first_render = Some(Instant::now());
            log::info!("first render");
        }
        self.registry.borrow_mut().clear();
        let reg = &self.registry;
        let state = if self.clicks > 0 {
            STATE_CLICKED
        } else {
            STATE_IDLE
        };
        let clicks_label = format!("Clicks: {}", self.clicks);

        div()
            .size_full()
            .relative()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .child(block(reg, "red", RED_AT, RED))
            .child(block(reg, "green", GREEN_AT, GREEN))
            .child(block(reg, "blue", BLUE_AT, BLUE))
            .child(block(reg, "state", STATE_AT, state))
            .child(
                div()
                    .id("line")
                    .role(Role::Label)
                    .absolute()
                    .left(px(TEXT_AT.0))
                    .top(px(TEXT_AT.1))
                    .text_size(px(TEXT_SIZE))
                    .child(text!(TEXT_LINE))
                    .child(probe(reg, "line", "label", TEXT_LINE.to_string())),
            )
            .child(
                div()
                    .id("count")
                    .role(Role::Button)
                    .aria_label("Count")
                    .absolute()
                    .left(px(BUTTON_RECT.0))
                    .top(px(BUTTON_RECT.1))
                    .w(px(BUTTON_RECT.2))
                    .h(px(BUTTON_RECT.3))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(BUTTON))
                    .text_color(rgb(0xffffff))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.clicks += 1;
                        log::info!("button clicked; clicks = {}", this.clicks);
                        cx.notify();
                    }))
                    .child(text!("Count"))
                    .child(probe(reg, "count", "button", "Count".to_string())),
            )
            .child(
                div()
                    .id("clicks")
                    .role(Role::Label)
                    .absolute()
                    .left(px(LABEL_AT.0))
                    .top(px(LABEL_AT.1))
                    .text_size(px(18.0))
                    .child(text!(clicks_label.clone()))
                    .child(probe(reg, "clicks", "label", clicks_label)),
            )
    }
}

struct Args {
    scenario: Option<std::path::PathBuf>,
    hold_ms: u64,
    automation: bool,
    exit_after: u64,
    log_level: log::LevelFilter,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        scenario: None,
        hold_ms: 0,
        automation: false,
        exit_after: 120,
        log_level: log::LevelFilter::Info,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--scenario" => {
                args.scenario = Some(it.next().ok_or("--scenario needs a folder")?.into())
            }
            "--hold-ms" => {
                args.hold_ms = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--hold-ms needs a number")?
            }
            "--automation" => args.automation = true,
            "--exit-after" => {
                args.exit_after = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--exit-after needs a number of seconds")?
            }
            "--log" => {
                args.log_level = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--log needs a level (error, warn, info, debug, trace)")?
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if args.scenario.is_none() && !args.automation {
        return Err("give --scenario <dir> or --automation".into());
    }
    Ok(args)
}

/// A minimal logger to stderr, so GPUI's own log lines (adapter selection, device creation, a11y) reach CI logs.
struct StderrLogger {
    start: Instant,
    level: log::LevelFilter,
}

impl log::Log for StderrLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level
    }
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            eprintln!(
                "[{:>7.3}s {:<5} {}] {}",
                self.start.elapsed().as_secs_f64(),
                record.level(),
                record.target(),
                record.args()
            );
        }
    }
    fn flush(&self) {}
}

pub fn platform_json() -> Value {
    json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "pid": std::process::id(),
        "features": {
            "capture": cfg!(feature = "capture"),
            "automation": cfg!(feature = "automation"),
        },
    })
}

/// What GPUI says about the window: renderer, scale, sizes.
pub fn window_json(window: &Window) -> Value {
    let gpu = window.gpu_specs();
    let b = window.bounds();
    let v = window.viewport_size();
    json!({
        "gpu": gpu.map(|g| json!({
            "device_name": g.device_name,
            "driver_name": g.driver_name,
            "driver_info": g.driver_info,
            "is_software_emulated": g.is_software_emulated,
        })),
        "scale_factor": window.scale_factor(),
        "bounds_logical": [f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height)],
        "viewport_logical": [f32::from(v.width), f32::from(v.height)],
        "a11y_enabled": window.is_a11y_enabled(),
        "a11y_active": window.is_a11y_active(),
    })
}

fn main() {
    let start = Instant::now();
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("render-ci: {e}");
            std::process::exit(2);
        }
    };
    if args.automation && !cfg!(feature = "automation") {
        eprintln!(
            "render-ci: the automation socket is not compiled into this build; refusing --automation"
        );
        std::process::exit(2);
    }
    let _ = log::set_boxed_logger(Box::new(StderrLogger {
        start,
        level: args.log_level,
    }));
    log::set_max_level(args.log_level);
    log::info!("render-ci starting: {}", platform_json());
    for var in [
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_SESSION_TYPE",
        "VK_DRIVER_FILES",
        "VK_ICD_FILENAMES",
        "LIBGL_ALWAYS_SOFTWARE",
    ] {
        if let Ok(v) = std::env::var(var) {
            log::info!("env {var}={v}");
        }
    }

    // The safety net: whatever happens, this process ends.
    let exit_after = args.exit_after;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(exit_after));
        eprintln!("render-ci: --exit-after {exit_after}s reached; exiting with code 3");
        std::process::exit(3);
    });

    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(WINDOW_W), px(WINDOW_H)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("render-ci".into()),
                ..Default::default()
            }),
            // Never throttle when the window is not focused: CI desktops may not give it focus.
            inactive_frame_interval: None,
            ..Default::default()
        };
        let registry: Registry = Rc::new(RefCell::new(Vec::new()));
        let opened = cx.open_window(options, |_window, cx| {
            cx.new(|_cx| TrialView {
                clicks: 0,
                rendered_clicks: 0,
                render_count: 0,
                first_render: None,
                registry: registry.clone(),
            })
        });
        let handle = match opened {
            Ok(h) => {
                log::info!("window opened after {} ms", start.elapsed().as_millis());
                h
            }
            Err(error) => {
                eprintln!("render-ci: could not open the window: {error:#}");
                std::process::exit(4);
            }
        };
        cx.activate(true);
        let view: Entity<TrialView> = match handle.update(cx, |_, _, cx| cx.entity()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("render-ci: no root view: {e:#}");
                std::process::exit(4);
            }
        };

        if let Some(dir) = args.scenario.clone() {
            run_scenario(cx, handle.into(), view.clone(), dir, args.hold_ms, start);
        }
        #[cfg(feature = "automation")]
        if args.automation {
            automation::start(cx, handle.into(), view, start);
        }
    });
}

/// Waits for the window to draw, captures a frame in-app, checks it and writes the results.
fn run_scenario(
    cx: &mut App,
    window: gpui::AnyWindowHandle,
    view: Entity<TrialView>,
    dir: std::path::PathBuf,
    hold_ms: u64,
    start: Instant,
) {
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("render-ci: cannot create {}: {e}", dir.display());
        std::process::exit(2);
    }
    cx.spawn(async move |cx| {
        // Wait (up to 60 s) for a first render, then let a few more frames go by.
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let (count, first) = cx.update(|cx| {
                let v = view.read(cx);
                (v.render_count, v.first_render)
            });
            if let Some(first) = first
                && first.elapsed() >= Duration::from_millis(800)
            {
                log::info!("rendered {count} time(s); capturing");
                break;
            }
            if Instant::now() > deadline {
                eprintln!("render-ci: no render within 60 s");
                let _ = std::fs::write(
                    dir.join("scenario.json"),
                    json!({ "ok": false, "error": "no render within 60 s", "platform": platform_json() }).to_string(),
                );
                std::process::exit(5);
            }
            cx.background_executor().timer(Duration::from_millis(50)).await;
        }

        let result = cx.update(|cx| {
            cx.update_window(window, |_root, window, cx| {
                let info = window_json(window);
                let v = view.read(cx);
                let render_count = v.render_count;
                let ttfr = v.first_render.map(|f| f.duration_since(start).as_millis());
                let captured = capture::capture_frame(window);
                (info, render_count, ttfr, captured)
            })
        });
        let (info, render_count, ttfr, captured) = match result {
            Ok(r) => r,
            Err(e) => {
                eprintln!("render-ci: window gone: {e:#}");
                std::process::exit(5);
            }
        };
        let (frame_json, ok) = match captured {
            Ok((img, method)) => {
                let path = dir.join("frame-inapp.png");
                let saved = img.save(&path).map_err(|e| e.to_string());
                let report = check_frame(&img, false);
                (
                    json!({ "method": method, "saved": saved.is_ok(), "save_error": saved.err(), "check": report.json }),
                    report.ok,
                )
            }
            Err(e) => (json!({ "error": format!("{e:#}") }), false),
        };
        let out = json!({
            "ok": ok,
            "platform": platform_json(),
            "window": info,
            "render_count": render_count,
            "time_to_first_render_ms": ttfr,
            "frame": frame_json,
        });
        let text = serde_json::to_string_pretty(&out).unwrap_or_default();
        println!("{text}");
        let _ = std::fs::write(dir.join("scenario.json"), &text);
        let _ = std::fs::write(dir.join("ready"), b"ready\n");
        log::info!("scenario done (ok = {ok}); holding {hold_ms} ms");
        cx.background_executor().timer(Duration::from_millis(hold_ms)).await;
        std::process::exit(if ok { 0 } else { 1 });
    })
    .detach();
}
