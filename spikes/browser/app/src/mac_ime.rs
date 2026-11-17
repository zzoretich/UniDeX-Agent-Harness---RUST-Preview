//! macOS: drives the window's own text-input client the way the OS input method does.
//!
//! An input method (Japanese, Chinese and so on) talks to the focused view through the
//! `NSTextInputClient` protocol: `setMarkedText:selectedRange:replacementRange:` while the user
//! composes, `insertText:replacementRange:` to commit, `unmarkText` to keep what is marked, and
//! `firstRectForCharacterRange:actualRange:` to place its candidate window. GPUI's view
//! implements these and forwards them to the focused element's input handler. Calling them on
//! GPUI's own view from the app therefore runs the same path an input method's calls take,
//! from the view inward. It does not run the input method itself (no input source is switched,
//! no candidate window opens): what reaches the page is what an input method would send.
//!
//! The calls must be made outside any GPUI update, as the OS makes them from the run loop.

use objc2::msg_send;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSPoint, NSRange, NSRect, NSSize, NSString};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// `NSNotFound`, used for "no replacement range".
const NOT_FOUND: usize = isize::MAX as usize;

pub struct NativeView(*mut AnyObject);

impl NativeView {
    pub fn of(window: &gpui::Window) -> Option<NativeView> {
        let handle = HasWindowHandle::window_handle(window).ok()?;
        match handle.as_raw() {
            RawWindowHandle::AppKit(appkit) => Some(NativeView(appkit.ns_view.as_ptr().cast())),
            _ => None,
        }
    }

    pub fn set_marked_text(&self, text: &str, selected: std::ops::Range<usize>) {
        let text = NSString::from_str(text);
        let selected = NSRange::new(selected.start, selected.end - selected.start);
        let replacement = NSRange::new(NOT_FOUND, 0);
        // SAFETY: GPUI's view implements NSTextInputClient; arguments are valid objects/structs.
        unsafe {
            let _: () = msg_send![self.0, setMarkedText: &*text, selectedRange: selected, replacementRange: replacement];
        }
    }

    pub fn insert_text(&self, text: &str) {
        let text = NSString::from_str(text);
        let replacement = NSRange::new(NOT_FOUND, 0);
        // SAFETY: as above.
        unsafe {
            let _: () = msg_send![self.0, insertText: &*text, replacementRange: replacement];
        }
    }

    pub fn unmark_text(&self) {
        // SAFETY: as above.
        unsafe {
            let _: () = msg_send![self.0, unmarkText];
        }
    }

    /// Where the input method would put its candidate window, in screen coordinates.
    pub fn first_rect(&self, start: usize, len: usize) -> (f64, f64, f64, f64) {
        let range = NSRange::new(start, len);
        // SAFETY: as above; a null actual-range pointer is allowed by the protocol.
        let rect: NSRect = unsafe {
            msg_send![self.0, firstRectForCharacterRange: range, actualRange: std::ptr::null_mut::<NSRange>()]
        };
        let NSRect {
            origin: NSPoint { x, y },
            size: NSSize { width, height },
        } = rect;
        (x, y, width, height)
    }
}
