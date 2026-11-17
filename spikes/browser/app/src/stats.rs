//! Measurement helpers for the trial: the helper's process tree (CPU time, memory footprint and
//! Chromium's sandbox state per process), and percentiles.
//!
//! macOS reads each of its own child processes with `proc_pid_rusage` (the numbers Activity
//! Monitor shows) and asks the OS whether each one runs inside a sandbox with `sandbox_check`
//! (exported by libSystem; the call Activity Monitor's "Sandbox" column uses). Elsewhere the
//! tree is not read.

use std::collections::HashMap;

/// One process at one moment.
#[derive(Clone, Debug)]
pub struct ProcSample {
    pub pid: i32,
    pub kind: String,
    pub cpu_ns: u64,
    pub footprint: u64,
    pub resident: u64,
    pub sandboxed: Option<bool>,
}

/// Reads the helper's process tree; remembers what kind each process is.
#[derive(Default)]
pub struct ProcessTree {
    kinds: HashMap<i32, String>,
}

impl ProcessTree {
    /// The helper and all its descendants.
    pub fn sample(&mut self, root: i32) -> Vec<ProcSample> {
        let mut out = Vec::new();
        let mut queue = vec![root];
        while let Some(pid) = queue.pop() {
            if let Some(sample) = self.one(pid) {
                out.push(sample);
            }
            queue.extend(children(pid));
        }
        out.sort_by_key(|s| s.pid);
        out
    }

    fn one(&mut self, pid: i32) -> Option<ProcSample> {
        let usage = os::rusage(pid)?;
        let kind = self
            .kinds
            .entry(pid)
            .or_insert_with(|| kind_of(pid))
            .clone();
        Some(ProcSample {
            pid,
            kind,
            cpu_ns: usage.0,
            footprint: usage.1,
            resident: usage.2,
            sandboxed: os::sandboxed(pid),
        })
    }
}

/// "browser", "renderer", "gpu-process", "utility:network.mojom.NetworkService" and so on,
/// from the process's own command line (read with `ps`, once per process).
fn kind_of(pid: i32) -> String {
    let command = std::process::Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let switch = |name: &str| {
        command
            .split_whitespace()
            .find_map(|a| a.strip_prefix(name))
            .map(str::to_string)
    };
    match switch("--type=") {
        None => "browser".into(),
        Some(kind) => match switch("--utility-sub-type=") {
            Some(sub) => format!("{kind}:{sub}"),
            None => kind,
        },
    }
}

fn children(pid: i32) -> Vec<i32> {
    os::children(pid)
}

/// CPU use of each process between two samples, in percent of one core, with the later
/// sample's memory. Processes that appear only in the later sample count from zero.
pub fn cpu_between(
    before: &[ProcSample],
    after: &[ProcSample],
    seconds: f64,
) -> Vec<(ProcSample, f64)> {
    after
        .iter()
        .map(|a| {
            let base = before
                .iter()
                .find(|b| b.pid == a.pid)
                .map_or(0, |b| b.cpu_ns);
            let pct = 100.0 * a.cpu_ns.saturating_sub(base) as f64 / 1e9 / seconds.max(1e-6);
            (a.clone(), pct)
        })
        .collect()
}

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

#[cfg(target_os = "macos")]
mod os {
    use std::ffi::{c_char, c_int};

    unsafe extern "C" {
        /// libSystem's sandbox query. With a null operation it returns 1 when `pid` runs inside
        /// a sandbox, 0 when it does not.
        fn sandbox_check(pid: libc::pid_t, operation: *const c_char, filter: c_int, ...) -> c_int;
    }

    /// (CPU time in ns, physical footprint, resident size).
    pub fn rusage(pid: i32) -> Option<(u64, u64, u64)> {
        // SAFETY: plain libc calls into a zeroed out-structure of the right type.
        unsafe {
            let mut info: libc::rusage_info_v4 = std::mem::zeroed();
            let buffer = (&mut info as *mut libc::rusage_info_v4).cast::<libc::rusage_info_t>();
            if libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V4, buffer) != 0 {
                return None;
            }
            // These times are in Mach absolute time units, not nanoseconds, on Apple silicon.
            let ticks = info.ri_user_time + info.ri_system_time;
            Some((
                ticks_to_ns(ticks),
                info.ri_phys_footprint,
                info.ri_resident_size,
            ))
        }
    }

    #[allow(deprecated)]
    fn ticks_to_ns(ticks: u64) -> u64 {
        let mut base = libc::mach_timebase_info { numer: 0, denom: 0 };
        // SAFETY: fills a plain struct.
        unsafe { libc::mach_timebase_info(&mut base) };
        if base.denom == 0 {
            return ticks;
        }
        (ticks as u128 * base.numer as u128 / base.denom as u128) as u64
    }

    pub fn sandboxed(pid: i32) -> Option<bool> {
        // SAFETY: a query about one process; no pointers besides the null operation.
        let result = unsafe { sandbox_check(pid, std::ptr::null(), 0) };
        match result {
            1 => Some(true),
            0 => Some(false),
            _ => None,
        }
    }

    pub fn children(pid: i32) -> Vec<i32> {
        let mut buffer = vec![0i32; 256];
        // SAFETY: the buffer is as long as we say.
        let n = unsafe {
            libc::proc_listchildpids(
                pid,
                buffer.as_mut_ptr().cast(),
                (buffer.len() * size_of::<i32>()) as i32,
            )
        };
        if n <= 0 {
            return Vec::new();
        }
        // Some releases return a count of pids, older ones a count of bytes.
        let count = (n as usize).min(buffer.len());
        buffer.truncate(count);
        buffer.retain(|p| *p > 0);
        buffer
    }
}

#[cfg(not(target_os = "macos"))]
mod os {
    pub fn rusage(_pid: i32) -> Option<(u64, u64, u64)> {
        None
    }
    pub fn sandboxed(_pid: i32) -> Option<bool> {
        None
    }
    pub fn children(_pid: i32) -> Vec<i32> {
        Vec::new()
    }
}
