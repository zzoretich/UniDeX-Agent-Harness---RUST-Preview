//! Frame and token timing without GPUI's profiler, as the chat trial measured it (decision 0008).
//!
//! Two clocks are read on the UI thread: when the root view's `render` starts, and when the last element of the
//! window has been painted. "Paint interval" is the time between successive paint ends; "draw work" is render start to
//! paint end (layout, prepaint and paint, not the hand-over to the GPU). A token's delay runs from the moment its
//! producer sent it to the paint end of the first frame that includes it. The trial found that GPUI's own profiler
//! counted the same slow frames as these paint intervals, so this cheap probe can run in every build.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Nearest-rank percentile of `sorted` (ascending), `p` in 0..=100. `None` when there are no samples.
pub fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    Some(sorted[rank.clamp(1, sorted.len()) - 1])
}

/// Median, 95th and 99th percentile and maximum of a set of samples, in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    pub count: usize,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

impl Summary {
    /// Summarises the finite samples; `None` when there are none.
    pub fn of(samples_ms: &[f64]) -> Option<Summary> {
        let mut sorted: Vec<f64> = samples_ms
            .iter()
            .copied()
            .filter(|x| x.is_finite())
            .collect();
        sorted.sort_by(f64::total_cmp);
        Some(Summary {
            count: sorted.len(),
            median_ms: percentile(&sorted, 50.0)?,
            p95_ms: percentile(&sorted, 95.0)?,
            p99_ms: percentile(&sorted, 99.0)?,
            max_ms: *sorted.last()?,
        })
    }
}

impl std::fmt::Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "p50 {:.1} ms, p99 {:.1} ms, max {:.1} ms",
            self.median_ms, self.p99_ms, self.max_ms
        )
    }
}

/// Paint intervals against the display's refresh interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameSummary {
    pub frames: usize,
    pub intervals: Summary,
    pub draw_work: Summary,
    pub refresh_ms: f64,
    /// Intervals longer than 1.5 times the refresh interval: at least one refresh was missed.
    pub over_1_5x: usize,
    /// Intervals longer than twice the refresh interval.
    pub over_2x: usize,
}

impl std::fmt::Display for FrameSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} frames, paint interval {}; {} over 1.5x and {} over 2x the {:.1} ms refresh interval",
            self.frames, self.intervals, self.over_1_5x, self.over_2x, self.refresh_ms
        )
    }
}

/// One drawn frame, as the app sees it.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub render_start: Instant,
    pub paint_end: Instant,
}

/// Records drawn frames. Call [`FrameRecorder::render_started`] at the top of the root view's `render` and
/// [`FrameRecorder::painted`] from an element painted last in the window.
#[derive(Debug, Default)]
pub struct FrameRecorder {
    frames: Vec<Frame>,
    render_start: Option<Instant>,
}

impl FrameRecorder {
    pub fn render_started(&mut self, at: Instant) {
        self.render_start = Some(at);
    }

    pub fn painted(&mut self, at: Instant) {
        let render_start = self.render_start.take().unwrap_or(at);
        self.frames.push(Frame {
            render_start,
            paint_end: at,
        });
    }

    pub fn frames(&self) -> usize {
        self.frames.len()
    }

    /// Time between successive paint ends, in ms.
    pub fn paint_intervals_ms(&self) -> Vec<f64> {
        self.frames
            .windows(2)
            .map(|pair| millis(pair[1].paint_end - pair[0].paint_end))
            .collect()
    }

    /// Render start to paint end of each frame, in ms.
    pub fn draw_work_ms(&self) -> Vec<f64> {
        self.frames
            .iter()
            .map(|frame| millis(frame.paint_end - frame.render_start))
            .collect()
    }

    /// Starts a new measurement window. The trial measured fixed windows (10 to 46 s); a recorder that is never
    /// cleared grows by one entry per frame.
    pub fn clear(&mut self) {
        self.frames.clear();
        self.render_start = None;
    }

    /// `None` until two frames have been painted.
    pub fn summary(&self, refresh: Duration) -> Option<FrameSummary> {
        let intervals = self.paint_intervals_ms();
        let refresh_ms = millis(refresh);
        let over = |limit: f64| intervals.iter().filter(|x| **x > limit).count();
        Some(FrameSummary {
            frames: self.frames.len(),
            intervals: Summary::of(&intervals)?,
            draw_work: Summary::of(&self.draw_work_ms())?,
            refresh_ms,
            over_1_5x: over(refresh_ms * 1.5),
            over_2x: over(refresh_ms * 2.0),
        })
    }
}

/// Token delays: from the producer's send to the paint end of the first frame that includes the token.
///
/// Call [`TokenLatency::applied`] when a token reaches the view's state, [`TokenLatency::frame_rendered`] when the view
/// renders (every token applied so far is in that frame) and [`TokenLatency::frame_painted`] when that frame's paint
/// ends.
#[derive(Debug, Default)]
pub struct TokenLatency {
    pending: VecDeque<Instant>,
    in_frame: usize,
    delays_ms: Vec<f64>,
}

impl TokenLatency {
    pub fn applied(&mut self, sent: Instant) {
        self.pending.push_back(sent);
    }

    pub fn frame_rendered(&mut self) {
        self.in_frame = self.pending.len();
    }

    pub fn frame_painted(&mut self, at: Instant) {
        for sent in self.pending.drain(..self.in_frame) {
            self.delays_ms
                .push(millis(at.saturating_duration_since(sent)));
        }
        self.in_frame = 0;
    }

    /// Tokens applied but not yet painted.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    pub fn summary(&self) -> Option<Summary> {
        Summary::of(&self.delays_ms)
    }
}

fn millis(duration: Duration) -> f64 {
    duration.as_nanos() as f64 / 1e6
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn percentile_is_nearest_rank() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&v, 50.0), Some(50.0));
        assert_eq!(percentile(&v, 95.0), Some(95.0));
        assert_eq!(percentile(&v, 99.0), Some(99.0));
        assert_eq!(percentile(&v, 100.0), Some(100.0));
        assert_eq!(percentile(&v, 0.0), Some(1.0));
        assert_eq!(percentile(&[7.0], 99.0), Some(7.0));
        assert_eq!(percentile(&[], 50.0), None);
    }

    #[test]
    fn summary_sorts_and_skips_non_finite_samples() {
        let s = Summary::of(&[3.0, f64::NAN, 1.0, 2.0, f64::INFINITY]).unwrap();
        assert_eq!(s.count, 3);
        assert_eq!(s.median_ms, 2.0);
        assert_eq!(s.max_ms, 3.0);
        assert_eq!(Summary::of(&[f64::NAN]), None);
    }

    #[test]
    fn recorder_measures_intervals_and_draw_work() {
        let t0 = Instant::now();
        let mut r = FrameRecorder::default();
        // Three frames at 10 ms intervals, then one late frame (25 ms) and one very late (21 ms is over 2x).
        for (start, end) in [(0, 2), (8, 10), (18, 20), (40, 45), (60, 66)] {
            r.render_started(t0 + ms(start));
            r.painted(t0 + ms(end));
        }
        assert_eq!(r.frames(), 5);
        assert_eq!(r.paint_intervals_ms(), vec![8.0, 10.0, 25.0, 21.0]);
        assert_eq!(r.draw_work_ms(), vec![2.0, 2.0, 2.0, 5.0, 6.0]);
        let s = r.summary(ms(10)).unwrap();
        assert_eq!(s.frames, 5);
        assert_eq!(s.refresh_ms, 10.0);
        assert_eq!(s.over_1_5x, 2);
        assert_eq!(s.over_2x, 2);
        assert_eq!(s.intervals.max_ms, 25.0);
        assert_eq!(s.draw_work.max_ms, 6.0);
        r.clear();
        assert_eq!(r.frames(), 0);
        assert_eq!(r.summary(ms(10)), None);
    }

    #[test]
    fn summaries_read_as_one_line() {
        let s = Summary::of(&[10.0, 10.04, 12.5]).unwrap();
        assert_eq!(s.to_string(), "p50 10.0 ms, p99 12.5 ms, max 12.5 ms");
        let f = FrameSummary {
            frames: 3,
            intervals: s,
            draw_work: s,
            refresh_ms: 10.0,
            over_1_5x: 1,
            over_2x: 0,
        };
        assert_eq!(
            f.to_string(),
            "3 frames, paint interval p50 10.0 ms, p99 12.5 ms, max 12.5 ms; 1 over 1.5x and 0 over 2x the \
             10.0 ms refresh interval"
        );
    }

    #[test]
    fn a_paint_without_a_render_counts_no_draw_work() {
        let t0 = Instant::now();
        let mut r = FrameRecorder::default();
        r.painted(t0);
        assert_eq!(r.draw_work_ms(), vec![0.0]);
    }

    #[test]
    fn tokens_are_timed_to_the_first_frame_that_includes_them() {
        let t0 = Instant::now();
        let mut l = TokenLatency::default();
        l.applied(t0);
        l.applied(t0 + ms(1));
        l.frame_rendered();
        // Applied after the render started: it belongs to the next frame.
        l.applied(t0 + ms(2));
        l.frame_painted(t0 + ms(10));
        assert_eq!(l.pending(), 1);
        // A frame painted without a render in between includes nothing new.
        l.frame_painted(t0 + ms(12));
        assert_eq!(l.pending(), 1);
        l.frame_rendered();
        l.frame_painted(t0 + ms(20));
        assert_eq!(l.pending(), 0);
        let s = l.summary().unwrap();
        assert_eq!(s.count, 3);
        assert_eq!(s.max_ms, 18.0);
        assert_eq!(s.median_ms, 10.0);
    }
}
