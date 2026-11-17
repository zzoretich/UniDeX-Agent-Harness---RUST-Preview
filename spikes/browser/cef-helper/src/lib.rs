//! The CEF helper for the browser trial (Task 12, decision 0012), as a library so that the same
//! code runs as a plain executable (`main.rs`, macOS and Linux) and as the DLL CEF's Windows
//! bootstrap loads (`cef-helper-win`). Everything needs the `cef` feature.
#![cfg(feature = "cef")]

mod guard;
mod helper;
mod keys;
#[cfg(target_os = "macos")]
mod mac;

pub use helper::run;
#[cfg(target_os = "linux")]
pub use helper::set_process_argv;
