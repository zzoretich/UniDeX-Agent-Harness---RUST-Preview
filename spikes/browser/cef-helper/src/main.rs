//! The CEF helper as an executable (macOS and Linux). See `helper.rs`.

fn main() -> std::process::ExitCode {
    let code = spike_cef_helper::run(std::ptr::null_mut());
    std::process::ExitCode::from(code.clamp(0, 255) as u8)
}
