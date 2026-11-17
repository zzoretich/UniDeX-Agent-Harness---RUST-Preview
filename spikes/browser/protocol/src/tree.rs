//! The helper's process tree, for measurements and for the sandbox check: CPU time, memory and
//! Chromium's sandbox state of the helper and every process it started.
//!
//! - macOS: `proc_pid_rusage` (the numbers Activity Monitor shows) and `sandbox_check` (the
//!   libSystem call behind Activity Monitor's "Sandbox" column).
//! - Linux: `/proc/<pid>/stat`, `statm` and `status` (the seccomp mode), and whether the
//!   process's PID and user namespaces differ from the caller's (Chromium's namespace or setuid
//!   sandbox gives renderers their own).
//! - Windows: not read; the tree is empty.

use std::collections::HashMap;

/// One process at one moment.
#[derive(Clone, Debug)]
pub struct ProcSample {
    pub pid: i32,
    /// "browser", "renderer", "gpu-process", "utility:network.mojom.NetworkService", ...
    pub kind: String,
    pub cpu_ns: u64,
    /// macOS: the physical footprint. Linux: the resident size.
    pub footprint: u64,
    pub resident: u64,
    /// Whether the OS reports the process as sandboxed (macOS: `sandbox_check`; Linux: seccomp
    /// filter mode 2). `None` where it cannot be read.
    pub sandboxed: Option<bool>,
    /// The raw evidence behind `sandboxed`, for logs.
    pub sandbox_detail: String,
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
            queue.extend(os::children(pid));
        }
        out.sort_by_key(|s| s.pid);
        out
    }

    fn one(&mut self, pid: i32) -> Option<ProcSample> {
        let (cpu_ns, footprint, resident) = os::rusage(pid)?;
        let kind = self
            .kinds
            .entry(pid)
            .or_insert_with(|| kind_from_command(&os::command(pid)))
            .clone();
        let (sandboxed, sandbox_detail) = os::sandboxed(pid);
        Some(ProcSample {
            pid,
            kind,
            cpu_ns,
            footprint,
            resident,
            sandboxed,
            sandbox_detail,
        })
    }
}

fn kind_from_command(command: &str) -> String {
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

    pub fn sandboxed(pid: i32) -> (Option<bool>, String) {
        // SAFETY: a query about one process; no pointers besides the null operation.
        let result = unsafe { sandbox_check(pid, std::ptr::null(), 0) };
        let state = match result {
            1 => Some(true),
            0 => Some(false),
            _ => None,
        };
        (state, format!("sandbox_check={result}"))
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

    pub fn command(pid: i32) -> String {
        std::process::Command::new("ps")
            .args(["-o", "command=", "-p", &pid.to_string()])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    }
}

#[cfg(target_os = "linux")]
mod os {
    fn read(pid: i32, file: &str) -> Option<String> {
        std::fs::read_to_string(format!("/proc/{pid}/{file}")).ok()
    }

    /// The fields of /proc/<pid>/stat after the command name (which may contain spaces).
    fn stat_fields(pid: i32) -> Option<Vec<String>> {
        let stat = read(pid, "stat")?;
        let after = stat.rsplit_once(')')?.1;
        Some(after.split_whitespace().map(str::to_string).collect())
    }

    pub fn rusage(pid: i32) -> Option<(u64, u64, u64)> {
        let fields = stat_fields(pid)?;
        // After ")": state(0) ppid(1) ... utime(11) stime(12).
        let ticks: u64 =
            fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?;
        // SAFETY: a plain sysconf query.
        let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u64;
        let pages: u64 = read(pid, "statm")?
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()?;
        // SAFETY: a plain sysconf query.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(1) as u64;
        let resident = pages * page;
        Some((ticks * 1_000_000_000 / hz, resident, resident))
    }

    pub fn sandboxed(pid: i32) -> (Option<bool>, String) {
        let seccomp = read(pid, "status").and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("Seccomp:"))
                .map(|v| v.trim().to_string())
        });
        let ns = |name: &str| {
            let theirs = std::fs::read_link(format!("/proc/{pid}/ns/{name}")).ok();
            let ours = std::fs::read_link(format!("/proc/self/ns/{name}")).ok();
            match (theirs, ours) {
                (Some(t), Some(o)) if t != o => "own",
                (Some(_), Some(_)) => "shared",
                _ => "unknown",
            }
        };
        let detail = format!(
            "seccomp={} pid_ns={} user_ns={} net_ns={}",
            seccomp.as_deref().unwrap_or("?"),
            ns("pid"),
            ns("user"),
            ns("net")
        );
        (seccomp.map(|s| s == "2"), detail)
    }

    pub fn children(pid: i32) -> Vec<i32> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return out;
        };
        for entry in entries.flatten() {
            let Some(child) = entry
                .file_name()
                .to_str()
                .and_then(|n| n.parse::<i32>().ok())
            else {
                continue;
            };
            let parent = stat_fields(child).and_then(|f| f.get(1)?.parse::<i32>().ok());
            if parent == Some(pid) {
                out.push(child);
            }
        }
        out
    }

    pub fn command(pid: i32) -> String {
        read(pid, "cmdline")
            .map(|c| c.replace('\0', " "))
            .unwrap_or_default()
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod os {
    pub fn rusage(_pid: i32) -> Option<(u64, u64, u64)> {
        None
    }
    pub fn sandboxed(_pid: i32) -> (Option<bool>, String) {
        (None, "not read on this platform".into())
    }
    pub fn children(_pid: i32) -> Vec<i32> {
        Vec::new()
    }
    pub fn command(_pid: i32) -> String {
        String::new()
    }
}
