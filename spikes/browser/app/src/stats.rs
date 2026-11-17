//! Measurement helpers for the trial: the helper's process tree (in `spike_protocol::tree`, shared
//! with the headless smoke check) and percentiles.

pub use spike_protocol::tree::{ProcSample, ProcessTree, cpu_between};

/// The p-th percentile (0 to 100) of `values`, nearest rank. `None` when empty.
pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    Some(sorted[rank.clamp(1, sorted.len()) - 1])
}

pub fn summary(values: &[f64]) -> serde_json::Value {
    let mean = if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    };
    serde_json::json!({
        "n": values.len(),
        "min": values.iter().copied().reduce(f64::min).map(round2),
        "mean": mean.map(round2),
        "p50": percentile(values, 50.0).map(round2),
        "p95": percentile(values, 95.0).map(round2),
        "p99": percentile(values, 99.0).map(round2),
        "max": percentile(values, 100.0).map(round2),
    })
}

pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
