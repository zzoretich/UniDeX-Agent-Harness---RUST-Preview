//! Percentiles and small JSON summaries of timing samples.

use serde_json::{Value, json};

/// Nearest-rank percentile of `sorted` (ascending), `p` in 0..=100.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// Median, 95th, 99th percentile, max and count of samples in milliseconds.
pub fn summary_ms(samples: &[f64]) -> Value {
    let mut v: Vec<f64> = samples.iter().copied().filter(|x| x.is_finite()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if v.is_empty() {
        return json!({ "count": 0 });
    }
    json!({
        "count": v.len(),
        "median_ms": round3(percentile(&v, 50.0)),
        "p95_ms": round3(percentile(&v, 95.0)),
        "p99_ms": round3(percentile(&v, 99.0)),
        "max_ms": round3(*v.last().unwrap()),
    })
}

/// Frame intervals in ms: the summary plus counts over the refresh interval, 1.5 and 2 times it, 16.7 and 33.3 ms.
pub fn frame_summary(intervals: &[f64], refresh_ms: f64) -> Value {
    let mut s = summary_ms(intervals);
    let over = |limit: f64| intervals.iter().filter(|x| **x > limit).count();
    if let Value::Object(map) = &mut s {
        map.insert("refresh_ms".into(), json!(round3(refresh_ms)));
        map.insert("over_1_05x_refresh".into(), json!(over(refresh_ms * 1.05)));
        map.insert("over_1_5x_refresh".into(), json!(over(refresh_ms * 1.5)));
        map.insert("over_2x_refresh".into(), json!(over(refresh_ms * 2.0)));
        map.insert("over_16_7ms".into(), json!(over(16.7)));
        map.insert("over_33_3ms".into(), json!(over(33.3)));
    }
    s
}

pub fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}
