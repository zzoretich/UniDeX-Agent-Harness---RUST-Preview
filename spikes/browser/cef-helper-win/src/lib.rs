//! The entry point CEF's Windows bootstrap executable calls (`cef_sandbox_win.h`):
//!
//! ```c
//! int RunConsoleMain(int argc, char* argv[], void* sandbox_info, cef_version_info_t* version_info);
//! ```
//!
//! The arguments are read again from the process command line by the helper; the sandbox
//! information is passed on to `CefExecuteProcess` and `CefInitialize`.

#[cfg(windows)]
#[unsafe(no_mangle)]
pub extern "C" fn RunConsoleMain(
    _argc: std::ffi::c_int,
    _argv: *mut *mut std::ffi::c_char,
    sandbox_info: *mut std::ffi::c_void,
    _version_info: *mut std::ffi::c_void,
) -> std::ffi::c_int {
    spike_cef_helper::run(sandbox_info.cast())
}
