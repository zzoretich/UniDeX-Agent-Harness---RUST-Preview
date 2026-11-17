//! A process's own CPU time and memory, read from the OS, for the trial's measurements.

/// One reading. Compare two readings to get CPU use over an interval.
#[derive(Clone, Copy, Debug, Default)]
pub struct Usage {
    /// User plus system CPU time so far.
    pub cpu_ns: u64,
    /// Resident memory now. On macOS this counts shared-memory pages the process touched, so
    /// the frame slots appear in both processes.
    pub resident_bytes: u64,
    /// macOS only: the footprint Activity Monitor shows as "Memory".
    pub footprint_bytes: u64,
}

impl Usage {
    /// CPU use between two readings, in percent of one core.
    pub fn cpu_percent_since(&self, earlier: &Usage, elapsed: std::time::Duration) -> f64 {
        let cpu = self.cpu_ns.saturating_sub(earlier.cpu_ns) as f64;
        100.0 * cpu / elapsed.as_nanos().max(1) as f64
    }
}

#[cfg(target_os = "macos")]
pub fn current() -> Usage {
    let mut usage = Usage::default();
    // SAFETY: plain libc calls into zeroed out-structures of the right type.
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut ru) == 0 {
            usage.cpu_ns = timeval_ns(ru.ru_utime) + timeval_ns(ru.ru_stime);
        }
        let mut info: libc::rusage_info_v4 = std::mem::zeroed();
        let buffer = (&mut info as *mut libc::rusage_info_v4).cast::<libc::rusage_info_t>();
        if libc::proc_pid_rusage(libc::getpid(), libc::RUSAGE_INFO_V4, buffer) == 0 {
            usage.resident_bytes = info.ri_resident_size;
            usage.footprint_bytes = info.ri_phys_footprint;
        }
    }
    usage
}

#[cfg(target_os = "linux")]
pub fn current() -> Usage {
    let mut usage = Usage::default();
    // SAFETY: plain libc call into a zeroed out-structure.
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut ru) == 0 {
            usage.cpu_ns = timeval_ns(ru.ru_utime) + timeval_ns(ru.ru_stime);
        }
    }
    if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
        let pages: u64 = statm
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        usage.resident_bytes = pages * 4096;
    }
    usage
}

/// Not measured on Windows in this trial.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn current() -> Usage {
    Usage::default()
}

#[cfg(unix)]
fn timeval_ns(tv: libc::timeval) -> u64 {
    tv.tv_sec as u64 * 1_000_000_000 + tv.tv_usec as u64 * 1_000
}
