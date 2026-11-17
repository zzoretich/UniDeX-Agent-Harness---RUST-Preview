//! Maps the UI toolkit's key names onto what CEF's `SendKeyEvent` needs: a Windows virtual-key
//! code (Chromium's `VKEY_*`, used on every platform) and the platform's native key code (a
//! macOS `kVK_*` value; Chromium reads it on macOS for some keys).
//!
//! The precedent crate's open issue was exactly this (special keys did nothing on macOS), so
//! the trial maps names explicitly instead of passing the typed character.

pub struct KeyCodes {
    pub windows: i32,
    pub mac: i32,
    /// A character CEF needs as a separate `KEYEVENT_CHAR`, for keys the platform text input
    /// does not turn into text (Return, Tab). Typed text arrives through the input handler.
    pub char_event: Option<u16>,
}

pub fn key_codes(key: &str) -> Option<KeyCodes> {
    let (windows, mac, char_event) = match key {
        "enter" => (0x0D, 0x24, Some(0x0D)),
        "tab" => (0x09, 0x30, Some(0x09)),
        "backspace" => (0x08, 0x33, None),
        "escape" => (0x1B, 0x35, None),
        "space" => (0x20, 0x31, None),
        "left" => (0x25, 0x7B, None),
        "up" => (0x26, 0x7E, None),
        "right" => (0x27, 0x7C, None),
        "down" => (0x28, 0x7D, None),
        "home" => (0x24, 0x73, None),
        "end" => (0x23, 0x77, None),
        "pageup" => (0x21, 0x74, None),
        "pagedown" => (0x22, 0x79, None),
        "delete" => (0x2E, 0x75, None),
        "insert" => (0x2D, 0x72, None),
        "shift" => (0x10, 0x38, None),
        "control" => (0x11, 0x3B, None),
        "alt" => (0x12, 0x3A, None),
        "platform" => (0x5B, 0x37, None),
        "function" => (0xFF, 0x3F, None),
        "f1" => (0x70, 0x7A, None),
        "f2" => (0x71, 0x78, None),
        "f3" => (0x72, 0x63, None),
        "f4" => (0x73, 0x76, None),
        "f5" => (0x74, 0x60, None),
        "f6" => (0x75, 0x61, None),
        "f7" => (0x76, 0x62, None),
        "f8" => (0x77, 0x64, None),
        "f9" => (0x78, 0x65, None),
        "f10" => (0x79, 0x6D, None),
        "f11" => (0x7A, 0x67, None),
        "f12" => (0x7B, 0x6F, None),
        ";" => (0xBA, 0x29, None),
        "=" => (0xBB, 0x18, None),
        "," => (0xBC, 0x2B, None),
        "-" => (0xBD, 0x1B, None),
        "." => (0xBE, 0x2F, None),
        "/" => (0xBF, 0x2C, None),
        "`" => (0xC0, 0x32, None),
        "[" => (0xDB, 0x21, None),
        "\\" => (0xDC, 0x2A, None),
        "]" => (0xDD, 0x1E, None),
        "'" => (0xDE, 0x27, None),
        single if single.chars().count() == 1 => {
            let c = single.chars().next()?.to_ascii_uppercase();
            let mac = mac_letter_or_digit(c)?;
            (c as i32, mac, None)
        }
        _ => return None,
    };
    Some(KeyCodes {
        windows,
        mac,
        char_event,
    })
}

/// macOS virtual key codes for A to Z and 0 to 9 (ANSI layout positions).
fn mac_letter_or_digit(c: char) -> Option<i32> {
    Some(match c {
        'A' => 0x00,
        'S' => 0x01,
        'D' => 0x02,
        'F' => 0x03,
        'H' => 0x04,
        'G' => 0x05,
        'Z' => 0x06,
        'X' => 0x07,
        'C' => 0x08,
        'V' => 0x09,
        'B' => 0x0B,
        'Q' => 0x0C,
        'W' => 0x0D,
        'E' => 0x0E,
        'R' => 0x0F,
        'Y' => 0x10,
        'T' => 0x11,
        '1' => 0x12,
        '2' => 0x13,
        '3' => 0x14,
        '4' => 0x15,
        '6' => 0x16,
        '5' => 0x17,
        '9' => 0x19,
        '7' => 0x1A,
        '8' => 0x1C,
        '0' => 0x1D,
        'O' => 0x1F,
        'U' => 0x20,
        'I' => 0x22,
        'P' => 0x23,
        'L' => 0x25,
        'J' => 0x26,
        'K' => 0x28,
        'N' => 0x2D,
        'M' => 0x2E,
        _ => return None,
    })
}
