//! Pieces both trial binaries share: a paint probe element, frame records, process numbers and JSON output.

#![allow(dead_code)]

use chat_term_core::procstat::{self, Usage};
use chat_term_core::stats::{self, round3};
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Position, Style, Window,
};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

/// A zero-size, absolutely positioned element that calls `on_paint` when it is painted. Placed as the last child of the
/// root, it marks the end of a frame's paint phase.
pub struct Probe {
    on_paint: Rc<dyn Fn(&mut Window)>,
}

impl Probe {
    pub fn new(on_paint: impl Fn(&mut Window) + 'static) -> Self {
        Self {
            on_paint: Rc::new(on_paint),
        }
    }
}

impl IntoElement for Probe {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Probe {
    type RequestLayoutState = ();
    type PrepaintState = ();

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
        let style = Style {
            position: Position::Absolute,
            ..Style::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        (self.on_paint)(window);
    }
}

/// One drawn frame, as the app sees it.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// When the root view's `render` started (start of the draw's layout phase).
    pub render_start: Instant,
    /// When the probe was painted (end of the paint phase, before the frame is handed to the platform).
    pub paint_end: Instant,
}

/// Frame and timing records shared between the view, the probe and the scenario driver.
#[derive(Default)]
pub struct Recorder {
    pub frames: Vec<Frame>,
    pub render_start: Option<Instant>,
    /// Frame signals seen by `on_next_frame` callbacks (only while a driver runs).
    pub signals: Vec<Instant>,
    pub renders: u64,
    /// Where the current measurement window starts (index into `frames`/`signals`).
    pub window_start_frame: usize,
    pub window_start_signal: usize,
    /// Indexes into `frames` of frames to report separately (for example the frame after a jump).
    pub marks: Vec<usize>,
}

pub type SharedRecorder = Rc<RefCell<Recorder>>;

impl Recorder {
    pub fn begin_window(&mut self) {
        self.window_start_frame = self.frames.len();
        self.window_start_signal = self.signals.len();
        self.marks.clear();
    }

    /// Mark the next frame to be drawn.
    pub fn mark_next(&mut self) {
        self.marks.push(self.frames.len());
    }

    /// Draw work of the marked frames, and the paint interval that ends with each of them, in ms.
    pub fn marked_stats(&self, refresh_ms: f64) -> Value {
        let work: Vec<f64> = self
            .marks
            .iter()
            .filter_map(|&i| self.frames.get(i))
            .map(|f| (f.paint_end - f.render_start).as_secs_f64() * 1000.0)
            .collect();
        let intervals: Vec<f64> = self
            .marks
            .iter()
            .filter(|&&i| i > 0 && i < self.frames.len())
            .map(|&i| {
                (self.frames[i].paint_end - self.frames[i - 1].paint_end).as_secs_f64() * 1000.0
            })
            .collect();
        json!({ "draw_work": stats::summary_ms(&work), "paint_interval_ending_here": stats::frame_summary(&intervals, refresh_ms) })
    }

    /// Paint-to-paint intervals and draw work (render start to paint end) of frames since `begin_window`, in ms.
    pub fn frame_stats(&self, refresh_ms: f64) -> Value {
        let frames = &self.frames[self.window_start_frame..];
        let intervals: Vec<f64> = frames
            .windows(2)
            .map(|w| (w[1].paint_end - w[0].paint_end).as_secs_f64() * 1000.0)
            .collect();
        let work: Vec<f64> = frames
            .iter()
            .map(|f| (f.paint_end - f.render_start).as_secs_f64() * 1000.0)
            .collect();
        let signals = &self.signals[self.window_start_signal..];
        let signal_intervals: Vec<f64> = signals
            .windows(2)
            .map(|w| (w[1] - w[0]).as_secs_f64() * 1000.0)
            .collect();
        json!({
            "frames_drawn": frames.len(),
            "paint_intervals": stats::frame_summary(&intervals, refresh_ms),
            "draw_work": stats::summary_ms(&work),
            "frame_signal_intervals": stats::frame_summary(&signal_intervals, refresh_ms),
        })
    }
}

/// Process numbers at the start of a measurement window; `end` turns them into deltas.
pub struct UsageWindow {
    start: Usage,
    started: Instant,
}

impl UsageWindow {
    pub fn begin() -> Self {
        Self {
            start: procstat::usage(),
            started: Instant::now(),
        }
    }

    pub fn end(&self) -> Value {
        let now = procstat::usage();
        let secs = self.started.elapsed().as_secs_f64();
        json!({
            "seconds": round3(secs),
            "cpu_s": round3(now.cpu_s - self.start.cpu_s),
            "cpu_percent_of_one_core": round3(100.0 * (now.cpu_s - self.start.cpu_s) / secs),
            "voluntary_context_switches": now.voluntary_switches - self.start.voluntary_switches,
            "involuntary_context_switches": now.involuntary_switches - self.start.involuntary_switches,
            "peak_rss_mib": round3(now.max_rss_mib),
            "footprint_mib": procstat::footprint_mib().map(round3),
        })
    }
}

/// Totals for the whole process so far.
pub fn process_snapshot() -> Value {
    let u = procstat::usage();
    json!({
        "cpu_s_total": round3(u.cpu_s),
        "peak_rss_mib": round3(u.max_rss_mib),
        "footprint_mib": procstat::footprint_mib().map(round3),
    })
}

pub fn ms(d: Duration) -> f64 {
    round3(d.as_secs_f64() * 1000.0)
}

pub fn wall_ms_since_process_start(at: SystemTime) -> Option<f64> {
    procstat::process_start().map(|start| round3(procstat::ms_between(start, at)))
}

pub fn build_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

/// `--name value` from the command line.
pub fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

pub fn write_json(path: Option<&str>, value: &Value) {
    let text = serde_json::to_string_pretty(value).unwrap();
    match path {
        Some(p) => std::fs::write(p, text).expect("write --out"),
        None => println!("{text}"),
    }
}

/// The monospace family for each platform (what the OS ships).
pub fn mono_family() -> &'static str {
    if cfg!(target_os = "macos") {
        "Menlo"
    } else if cfg!(windows) {
        "Consolas"
    } else {
        "DejaVu Sans Mono"
    }
}

/// Facts about the window and display for the record.
pub fn window_facts(window: &Window, cx: &App) -> Value {
    let bounds = window.bounds();
    json!({
        "refresh_interval_ms": window.refresh_interval(cx).map(ms),
        "scale_factor": window.scale_factor(),
        "window_width": f32::from(bounds.size.width),
        "window_height": f32::from(bounds.size.height),
        "active": window.is_window_active(),
    })
}
