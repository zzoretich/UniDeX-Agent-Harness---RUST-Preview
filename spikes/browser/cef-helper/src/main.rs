//! The CEF helper as an executable (macOS and Linux). See `helper.rs`.

fn main() -> std::process::ExitCode {
    #[cfg(target_os = "linux")]
    process_argv::hand_over();
    let code = spike_cef_helper::run(std::ptr::null_mut());
    std::process::ExitCode::from(code.clamp(0, 255) as u8)
}

/// Linux: the process's own `argc` and `argv`, captured before `main` the way the standard
/// library captures them (glibc passes them to every `.init_array` function), and handed to the
/// helper for CEF. Chromium writes each sub-process's role into this memory (its process title),
/// so `/proc/<pid>/cmdline` of a process forked from a zygote reads `--type=renderer` and so on
/// rather than the zygote's command line; given a copy of the arguments, Chromium's write lands
/// in the copy and no tool can see it. Kept in the executable, whose objects are always linked.
#[cfg(target_os = "linux")]
mod process_argv {
    use std::ffi::{c_char, c_int};
    use std::sync::atomic::{AtomicI32, AtomicPtr, Ordering};

    static ARGC: AtomicI32 = AtomicI32::new(0);
    static ARGV: AtomicPtr<*mut c_char> = AtomicPtr::new(std::ptr::null_mut());

    extern "C" fn capture(argc: c_int, argv: *mut *mut c_char, _envp: *mut *mut c_char) {
        ARGC.store(argc, Ordering::Relaxed);
        ARGV.store(argv, Ordering::Relaxed);
    }

    #[used]
    #[unsafe(link_section = ".init_array.00099")]
    static CAPTURE: extern "C" fn(c_int, *mut *mut c_char, *mut *mut c_char) = capture;

    pub fn hand_over() {
        spike_cef_helper::set_process_argv(
            ARGC.load(Ordering::Relaxed),
            ARGV.load(Ordering::Relaxed),
        );
    }
}
