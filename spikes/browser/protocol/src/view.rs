//! The `BrowserView` trait, shaped from design section 5.8.
//!
//! It hides the engine (CEF off-screen in a helper process first; on Windows possibly the OS web
//! view) from the app. The app draws [`Frame`]s as an element, forwards input as [`InputEvent`]s,
//! and drives pages over the Chrome DevTools Protocol with [`BrowserView::devtools`]. Everything
//! the engine reports arrives as a [`BrowserEvent`] through the sink given when the view is made.
//! No GPUI types appear here, so the core keeps no UI code.

use serde::{Deserialize, Serialize};

/// A rectangle in pixels. Logical (device-independent) or device pixels, as each use says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Modifier keys held during an input event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    /// Command on macOS, the Windows key elsewhere.
    pub platform: bool,
    pub function: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WheelUnit {
    /// Exact pixel deltas (trackpads).
    Pixels,
    /// Line steps (mouse wheels).
    Lines,
}

/// Input forwarded from the app into the page. Positions are logical pixels relative to the
/// top-left corner of the browser view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputEvent {
    PointerMove {
        x: f32,
        y: f32,
        pressed: Option<PointerButton>,
        modifiers: Modifiers,
    },
    PointerDown {
        x: f32,
        y: f32,
        button: PointerButton,
        click_count: u32,
        modifiers: Modifiers,
    },
    PointerUp {
        x: f32,
        y: f32,
        button: PointerButton,
        click_count: u32,
        modifiers: Modifiers,
    },
    PointerLeave {
        modifiers: Modifiers,
    },
    Wheel {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        unit: WheelUnit,
        modifiers: Modifiers,
    },
    /// A key went down. `key` is the platform-neutral key name the UI toolkit reports
    /// ("a", "enter", "backspace", "left", "shift"), `key_char` the character it would type.
    /// The helper maps it to the engine's key codes.
    KeyDown {
        key: String,
        key_char: Option<String>,
        modifiers: Modifiers,
        is_held: bool,
    },
    KeyUp {
        key: String,
        modifiers: Modifiers,
    },
    /// Text committed by the platform's text input (a typed character, or the end of an input
    /// method composition).
    Text {
        text: String,
    },
    /// An input method composition started or changed. Ranges are UTF-16 offsets in `text`.
    ImeSetComposition {
        text: String,
        selected_start: u32,
        selected_end: u32,
    },
    /// The composition ended and its current text is kept.
    ImeFinish,
    /// The composition was cancelled.
    ImeCancel,
    Focus {
        focused: bool,
    },
}

/// The cursor the page asks for. A subset; the helper maps the engine's set onto it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorKind {
    #[default]
    Arrow,
    IBeam,
    PointingHand,
    Crosshair,
    Grab,
    Grabbing,
    ResizeLeftRight,
    ResizeUpDown,
    NotAllowed,
    Hidden,
}

/// Which surface a frame belongs to: the page itself, or a popup widget such as an open
/// `<select>` list, which off-screen rendering paints separately for the app to place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameKind {
    View,
    Popup,
}

/// One painted frame, copied out of shared memory. BGRA, 8 bits per channel, rows packed with
/// no padding, upper-left origin: what CEF's `OnPaint` hands over.
///
/// This is the CPU path, the only one every platform can draw with the pinned GPUI snapshot.
/// A faster path (an IOSurface on macOS, a DXGI shared handle on Windows, dmabuf on Linux) would
/// add a variant here; see decision 0012.
pub struct Frame {
    pub kind: FrameKind,
    pub seq: u64,
    /// Device pixels.
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    /// Device-pixel rectangles that changed since the previous frame of this kind.
    pub dirty: Vec<Rect>,
    /// When the helper finished painting it (Unix time in nanoseconds).
    pub produced_unix_ns: u64,
    /// How long the copy out of shared memory took in this process.
    pub copy_ns: u64,
}

/// Everything a browser engine reports to the app.
pub enum BrowserEvent {
    /// The engine is up. `engine` names it and its version, for the UI and the logs.
    Ready {
        engine: String,
    },
    Frame(Frame),
    /// Show or hide a popup widget; `rect` is in logical pixels relative to the view.
    Popup {
        show: bool,
        rect: Option<Rect>,
    },
    Cursor(CursorKind),
    /// Where the composition's characters are, in logical pixels relative to the view, so the
    /// input method's candidate window can sit next to them.
    ImeCompositionBounds {
        range_start: u32,
        range_end: u32,
        characters: Vec<Rect>,
    },
    /// A DevTools protocol message from the engine: a result or error for a call made with
    /// [`BrowserView::devtools`] (it carries the call's `id`), or an event.
    DevTools(serde_json::Value),
    /// A line the engine reports for the log (see `wire::FromHelper::Status`).
    Status(String),
    /// The engine process ended. The view needs a new engine to show anything again.
    Exited {
        status: String,
    },
}

/// What the app needs from an embedded browser engine.
pub trait BrowserView {
    /// The view's size in logical pixels and the display's scale factor. The engine renders
    /// `width * scale_factor` by `height * scale_factor` device pixels.
    fn resize(&mut self, width: u32, height: u32, scale_factor: f32);

    fn input(&mut self, event: InputEvent);

    /// Sends one Chrome DevTools Protocol call and returns its id. The answer arrives as a
    /// [`BrowserEvent::DevTools`] carrying the same id.
    fn devtools(&mut self, method: &str, params: serde_json::Value) -> u64;

    fn navigate(&mut self, url: &str) -> u64 {
        self.devtools("Page.navigate", serde_json::json!({ "url": url }))
    }

    fn evaluate(&mut self, expression: &str) -> u64 {
        self.devtools(
            "Runtime.evaluate",
            serde_json::json!({ "expression": expression, "returnByValue": true }),
        )
    }

    /// Asks the engine to stop and waits briefly; the engine is killed if it does not.
    fn shutdown(&mut self);
}
