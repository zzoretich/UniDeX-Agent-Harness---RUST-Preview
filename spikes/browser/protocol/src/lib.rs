//! The app-facing half of the browser trial (Task 12, decision 0012). Throwaway.
//!
//! - [`view`]: the `BrowserView` trait the app draws and drives, and the events it gets back.
//! - [`wire`]: the messages between the app and a helper process (JSON lines on the helper's
//!   inherited stdin and stdout, never a named socket or a port).
//! - [`shm`]: the shared-memory frame slots the helper paints into (inherited, never named).
//! - [`helper`]: `HelperBrowser`, the `BrowserView` that spawns a helper process (the stand-in
//!   now, the CEF helper in phase B) and turns its messages into events.
//! - [`usage`]: a process's own CPU time and memory, for the trial's measurements.
//! - [`tree`]: the helper's process tree: CPU, memory and Chromium's sandbox state per process.

pub mod helper;
pub mod shm;
pub mod tree;
pub mod usage;
pub mod view;
pub mod wire;

pub use view::*;
