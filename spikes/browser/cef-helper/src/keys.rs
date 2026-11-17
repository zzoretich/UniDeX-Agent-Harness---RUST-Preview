//! Maps the UI toolkit's key names onto what CEF's `SendKeyEvent` needs: a Windows virtual-key
//! code (Chromium's `VKEY_*`, used on every platform) and the platform's native key code, which
//! Chromium reads for the DOM `code` and some keys:
//!
//! - macOS: a `kVK_*` value (ANSI positions). Exercised by the trial's input checks.
//! - Linux: an XKB keycode (the evdev code plus 8; US positions). Unverified: no input ran there.
//! - Windows: the `WM_KEYDOWN` / `WM_KEYUP` `lParam` (repeat count 1, the set-1 scan code, the
//!   extended-key bit, and for a release the previous-state and transition bits). Unverified.
//!
//! The precedent crate's open issue was exactly this (special keys did nothing on macOS), so
//! the trial maps names explicitly instead of passing the typed character.

pub struct KeyCodes {
    pub windows: i32,
    /// The native key code for a press and for a release (they differ only on Windows).
    pub native_down: i32,
    pub native_up: i32,
    /// A character CEF needs as a separate `KEYEVENT_CHAR`, for keys the platform text input
    /// does not turn into text (Return, Tab). Typed text arrives through the input handler.
    pub char_event: Option<u16>,
}

/// One key: Windows virtual key, macOS `kVK_*`, XKB keycode, Windows scan code, extended key.
struct Row(i32, i32, i32, i32, bool);

pub fn key_codes(key: &str) -> Option<KeyCodes> {
    let (row, char_event) = match key {
        "enter" => (Row(0x0D, 0x24, 36, 0x1C, false), Some(0x0D)),
        "tab" => (Row(0x09, 0x30, 23, 0x0F, false), Some(0x09)),
        "backspace" => (Row(0x08, 0x33, 22, 0x0E, false), None),
        "escape" => (Row(0x1B, 0x35, 9, 0x01, false), None),
        "space" => (Row(0x20, 0x31, 65, 0x39, false), None),
        "left" => (Row(0x25, 0x7B, 113, 0x4B, true), None),
        "up" => (Row(0x26, 0x7E, 111, 0x48, true), None),
        "right" => (Row(0x27, 0x7C, 114, 0x4D, true), None),
        "down" => (Row(0x28, 0x7D, 116, 0x50, true), None),
        "home" => (Row(0x24, 0x73, 110, 0x47, true), None),
        "end" => (Row(0x23, 0x77, 115, 0x4F, true), None),
        "pageup" => (Row(0x21, 0x74, 112, 0x49, true), None),
        "pagedown" => (Row(0x22, 0x79, 117, 0x51, true), None),
        "delete" => (Row(0x2E, 0x75, 119, 0x53, true), None),
        "insert" => (Row(0x2D, 0x72, 118, 0x52, true), None),
        "shift" => (Row(0x10, 0x38, 50, 0x2A, false), None),
        "control" => (Row(0x11, 0x3B, 37, 0x1D, false), None),
        "alt" => (Row(0x12, 0x3A, 64, 0x38, false), None),
        "platform" => (Row(0x5B, 0x37, 133, 0x5B, true), None),
        // The Fn key has no XKB keycode or scan code: the keyboard handles it.
        "function" => (Row(0xFF, 0x3F, 0, 0, false), None),
        "f1" => (Row(0x70, 0x7A, 67, 0x3B, false), None),
        "f2" => (Row(0x71, 0x78, 68, 0x3C, false), None),
        "f3" => (Row(0x72, 0x63, 69, 0x3D, false), None),
        "f4" => (Row(0x73, 0x76, 70, 0x3E, false), None),
        "f5" => (Row(0x74, 0x60, 71, 0x3F, false), None),
        "f6" => (Row(0x75, 0x61, 72, 0x40, false), None),
        "f7" => (Row(0x76, 0x62, 73, 0x41, false), None),
        "f8" => (Row(0x77, 0x64, 74, 0x42, false), None),
        "f9" => (Row(0x78, 0x65, 75, 0x43, false), None),
        "f10" => (Row(0x79, 0x6D, 76, 0x44, false), None),
        "f11" => (Row(0x7A, 0x67, 95, 0x57, false), None),
        "f12" => (Row(0x7B, 0x6F, 96, 0x58, false), None),
        ";" => (Row(0xBA, 0x29, 47, 0x27, false), None),
        "=" => (Row(0xBB, 0x18, 21, 0x0D, false), None),
        "," => (Row(0xBC, 0x2B, 59, 0x33, false), None),
        "-" => (Row(0xBD, 0x1B, 20, 0x0C, false), None),
        "." => (Row(0xBE, 0x2F, 60, 0x34, false), None),
        "/" => (Row(0xBF, 0x2C, 61, 0x35, false), None),
        "`" => (Row(0xC0, 0x32, 49, 0x29, false), None),
        "[" => (Row(0xDB, 0x21, 34, 0x1A, false), None),
        "\\" => (Row(0xDC, 0x2A, 51, 0x2B, false), None),
        "]" => (Row(0xDD, 0x1E, 35, 0x1B, false), None),
        "'" => (Row(0xDE, 0x27, 48, 0x28, false), None),
        single if single.chars().count() == 1 => {
            let c = single.chars().next()?.to_ascii_uppercase();
            (letter_or_digit(c)?, None)
        }
        _ => return None,
    };
    let Row(windows, mac, xkb, scan, extended) = row;
    let (native_down, native_up) = native(mac, xkb, scan, extended);
    Some(KeyCodes {
        windows,
        native_down,
        native_up,
        char_event,
    })
}

#[cfg(target_os = "macos")]
fn native(mac: i32, _xkb: i32, _scan: i32, _extended: bool) -> (i32, i32) {
    (mac, mac)
}

#[cfg(target_os = "linux")]
fn native(_mac: i32, xkb: i32, _scan: i32, _extended: bool) -> (i32, i32) {
    (xkb, xkb)
}

#[cfg(windows)]
fn native(_mac: i32, _xkb: i32, scan: i32, extended: bool) -> (i32, i32) {
    let down = 1u32 | ((scan as u32) << 16) | if extended { 1 << 24 } else { 0 };
    let up = down | (1 << 30) | (1 << 31);
    (down as i32, up as i32)
}

/// A to Z and 0 to 9: (macOS `kVK_*`, XKB keycode, Windows scan code) at ANSI / US positions.
fn letter_or_digit(c: char) -> Option<Row> {
    let (mac, xkb, scan) = match c {
        'A' => (0x00, 38, 0x1E),
        'S' => (0x01, 39, 0x1F),
        'D' => (0x02, 40, 0x20),
        'F' => (0x03, 41, 0x21),
        'H' => (0x04, 43, 0x23),
        'G' => (0x05, 42, 0x22),
        'Z' => (0x06, 52, 0x2C),
        'X' => (0x07, 53, 0x2D),
        'C' => (0x08, 54, 0x2E),
        'V' => (0x09, 55, 0x2F),
        'B' => (0x0B, 56, 0x30),
        'Q' => (0x0C, 24, 0x10),
        'W' => (0x0D, 25, 0x11),
        'E' => (0x0E, 26, 0x12),
        'R' => (0x0F, 27, 0x13),
        'Y' => (0x10, 29, 0x15),
        'T' => (0x11, 28, 0x14),
        '1' => (0x12, 10, 0x02),
        '2' => (0x13, 11, 0x03),
        '3' => (0x14, 12, 0x04),
        '4' => (0x15, 13, 0x05),
        '6' => (0x16, 15, 0x07),
        '5' => (0x17, 14, 0x06),
        '9' => (0x19, 18, 0x0A),
        '7' => (0x1A, 16, 0x08),
        '8' => (0x1C, 17, 0x09),
        '0' => (0x1D, 19, 0x0B),
        'O' => (0x1F, 32, 0x18),
        'U' => (0x20, 30, 0x16),
        'I' => (0x22, 31, 0x17),
        'P' => (0x23, 33, 0x19),
        'L' => (0x25, 46, 0x26),
        'J' => (0x26, 44, 0x24),
        'K' => (0x28, 45, 0x25),
        'N' => (0x2D, 57, 0x31),
        'M' => (0x2E, 58, 0x32),
        _ => return None,
    };
    Some(Row(c as i32, mac, xkb, scan, false))
}
