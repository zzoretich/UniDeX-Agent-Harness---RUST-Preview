//! Small, compiled examples of GPUI on the pinned snapshot, and the window-free logic they use.
//!
//! The examples live in `examples/`. This library holds the parts of their patterns that need no window, so they are
//! unit-tested on every platform: frame and token timing, a channel a GPUI task can await, a text field's editing
//! model, saved window state, link arguments, the frame transport's checks, and the frame check of a captured scene.
//! It uses only the standard library; GPUI is a dev-dependency of this crate, used by the examples and one test. See
//! `README.md` in this crate for the dependency lines to copy, how to run an example, and how GPUI is upgraded.

pub mod channel;
pub mod frame_check;
pub mod frames;
pub mod links;
pub mod stats;
pub mod text_model;
pub mod window_state;
pub mod wire;
