//! The part of the Task 10 trial that needs no GPUI: content generation, incremental markdown, statistics, key
//! encoding, the two PTY backends and the terminal session. `headless-check` runs all of it without a display, which
//! is what CI can exercise on Linux and Windows.

pub mod content;
pub mod keys;
pub mod md;
pub mod procstat;
pub mod pty;
pub mod stats;
