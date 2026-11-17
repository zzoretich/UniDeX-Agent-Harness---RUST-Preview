//! Process-level numbers the app reads about itself: CPU time, context switches, peak and current memory, and the
//! wall-clock time the OS recorded for the process start (for cold start).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Default)]
pub struct Usage {
    /// User plus system CPU time, in seconds.
    pub cpu_s: f64,
    /// Voluntary context switches: each one is the process blocking and later being woken.
    pub voluntary_switches: i64,
    pub involuntary_switches: i64,
    /// Peak resident set size over the process lifetime, in MiB.
    pub max_rss_mib: f64,
}

#[cfg(unix)]
pub fn usage() -> Usage {
    // SAFETY: getrusage writes into the zeroed struct we pass.
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    let tv = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
    // macOS reports ru_maxrss in bytes, Linux in KiB.
    let max_rss_mib = if cfg!(target_os = "macos") {
        ru.ru_maxrss as f64 / 1048576.0
    } else {
        ru.ru_maxrss as f64 / 1024.0
    };
    Usage {
        cpu_s: tv(ru.ru_utime) + tv(ru.ru_stime),
        voluntary_switches: ru.ru_nvcsw as i64,
        involuntary_switches: ru.ru_nivcsw as i64,
        max_rss_mib,
    }
}

#[cfg(not(unix))]
pub fn usage() -> Usage {
    Usage::default()
}

/// The physical footprint (what Activity Monitor calls memory), in MiB, where the OS reports it (macOS only here).
#[cfg(target_os = "macos")]
pub fn footprint_mib() -> Option<f64> {
    // SAFETY: proc_pid_rusage fills a rusage_info_v2 for our own pid.
    let mut info: libc::rusage_info_v2 = unsafe { std::mem::zeroed() };
    let rc = unsafe {
        libc::proc_pid_rusage(
            libc::getpid(),
            libc::RUSAGE_INFO_V2,
            &mut info as *mut libc::rusage_info_v2 as *mut libc::rusage_info_t,
        )
    };
    (rc == 0).then(|| info.ri_phys_footprint as f64 / 1048576.0)
}

#[cfg(not(target_os = "macos"))]
pub fn footprint_mib() -> Option<f64> {
    None
}

/// When the OS says this process started, as wall-clock time.
#[cfg(target_os = "macos")]
pub fn process_start() -> Option<SystemTime> {
    // SAFETY: proc_pidinfo fills a proc_bsdinfo for our own pid.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let rc = unsafe {
        libc::proc_pidinfo(
            libc::getpid(),
            libc::PROC_PIDTBSDINFO,
            0,
            &mut info as *mut libc::proc_bsdinfo as *mut libc::c_void,
            size,
        )
    };
    (rc == size).then(|| {
        UNIX_EPOCH
            + Duration::from_secs(info.pbi_start_tvsec)
            + Duration::from_micros(info.pbi_start_tvusec)
    })
}

/// Linux: start time in clock ticks after boot (field 22 of /proc/self/stat) plus the boot time; 10 ms resolution.
#[cfg(target_os = "linux")]
pub fn process_start() -> Option<SystemTime> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let after_comm = &stat[stat.rfind(')')? + 2..];
    let ticks: u64 = after_comm.split_whitespace().nth(19)?.parse().ok()?;
    let btime: u64 = std::fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("btime ").and_then(|v| v.trim().parse().ok()))?;
    // SAFETY: sysconf has no preconditions.
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as u64;
    Some(UNIX_EPOCH + Duration::from_secs(btime) + Duration::from_millis(ticks * 1000 / hz.max(1)))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn process_start() -> Option<SystemTime> {
    None
}

/// Milliseconds from `earlier` to `later` (negative if reversed).
pub fn ms_between(earlier: SystemTime, later: SystemTime) -> f64 {
    match later.duration_since(earlier) {
        Ok(d) => d.as_secs_f64() * 1000.0,
        Err(e) => -e.duration().as_secs_f64() * 1000.0,
    }
}
