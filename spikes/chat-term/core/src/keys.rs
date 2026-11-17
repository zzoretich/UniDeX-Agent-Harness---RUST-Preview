//! Encoding keystrokes and pastes into the bytes a terminal program expects (xterm conventions).
//!
//! The key names are GPUI's (`"up"`, `"pageup"`, `"f5"`, a single character for printable keys), but nothing here
//! depends on GPUI, so the table can be checked on every platform without a window.

/// A platform-neutral keystroke.
#[derive(Clone, Copy, Debug, Default)]
pub struct Key<'a> {
    /// GPUI's key name: a character for printable keys, or a name such as "enter" or "left".
    pub key: &'a str,
    /// The character the key would type with the current modifiers, if any (GPUI's `key_char`).
    pub key_char: Option<&'a str>,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// The xterm modifier parameter: 1 + shift + 2 alt + 4 ctrl.
fn modifier_param(k: &Key) -> u8 {
    1 + k.shift as u8 + 2 * k.alt as u8 + 4 * k.ctrl as u8
}

fn csi_with_mods(k: &Key, final_byte: char, app_cursor: bool) -> Vec<u8> {
    let m = modifier_param(k);
    if m > 1 {
        format!("\x1b[1;{m}{final_byte}").into_bytes()
    } else if app_cursor {
        format!("\x1bO{final_byte}").into_bytes()
    } else {
        format!("\x1b[{final_byte}").into_bytes()
    }
}

fn tilde_with_mods(k: &Key, code: u8) -> Vec<u8> {
    let m = modifier_param(k);
    if m > 1 {
        format!("\x1b[{code};{m}~").into_bytes()
    } else {
        format!("\x1b[{code}~").into_bytes()
    }
}

/// The control byte for ctrl plus `c`, if there is one.
fn ctrl_byte(c: char) -> Option<u8> {
    match c {
        'a'..='z' => Some(c as u8 - b'a' + 1),
        'A'..='Z' => Some(c as u8 - b'A' + 1),
        '@' | ' ' | '2' => Some(0),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '-' | '7' => Some(0x1f),
        '?' | '8' => Some(0x7f),
        _ => None,
    }
}

/// Bytes for a keystroke, or `None` when the terminal should not receive it (a bare modifier, a platform shortcut).
/// `app_cursor` is the terminal's application cursor mode (DECCKM), which changes the arrow and Home/End sequences.
pub fn encode(k: &Key, app_cursor: bool) -> Option<Vec<u8>> {
    let alt_prefix = |mut bytes: Vec<u8>| {
        if k.alt {
            bytes.insert(0, 0x1b);
        }
        bytes
    };
    let bytes = match k.key {
        "enter" => alt_prefix(vec![b'\r']),
        "tab" if k.shift => b"\x1b[Z".to_vec(),
        "tab" => alt_prefix(vec![b'\t']),
        "backspace" if k.ctrl => alt_prefix(vec![0x08]),
        "backspace" => alt_prefix(vec![0x7f]),
        "escape" => vec![0x1b],
        "space" if k.ctrl => alt_prefix(vec![0]),
        "space" => alt_prefix(vec![b' ']),
        "up" => csi_with_mods(k, 'A', app_cursor),
        "down" => csi_with_mods(k, 'B', app_cursor),
        "right" => csi_with_mods(k, 'C', app_cursor),
        "left" => csi_with_mods(k, 'D', app_cursor),
        "home" => csi_with_mods(k, 'H', app_cursor),
        "end" => csi_with_mods(k, 'F', app_cursor),
        "insert" => tilde_with_mods(k, 2),
        "delete" => tilde_with_mods(k, 3),
        "pageup" => tilde_with_mods(k, 5),
        "pagedown" => tilde_with_mods(k, 6),
        "f1" => csi_or_ss3(k, 'P'),
        "f2" => csi_or_ss3(k, 'Q'),
        "f3" => csi_or_ss3(k, 'R'),
        "f4" => csi_or_ss3(k, 'S'),
        "f5" => tilde_with_mods(k, 15),
        "f6" => tilde_with_mods(k, 17),
        "f7" => tilde_with_mods(k, 18),
        "f8" => tilde_with_mods(k, 19),
        "f9" => tilde_with_mods(k, 20),
        "f10" => tilde_with_mods(k, 21),
        "f11" => tilde_with_mods(k, 23),
        "f12" => tilde_with_mods(k, 24),
        key => {
            let mut chars = key.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None; // an unknown named key or a bare modifier
            }
            if k.ctrl {
                alt_prefix(vec![ctrl_byte(c)?])
            } else if k.alt {
                // Alt as Meta: ESC then the unmodified character (shifted when shift is held).
                let base: String = if k.shift {
                    c.to_uppercase().collect()
                } else {
                    c.to_string()
                };
                alt_prefix(base.into_bytes())
            } else {
                let typed = k.key_char.map(str::to_string).unwrap_or_else(|| {
                    if k.shift {
                        c.to_uppercase().collect()
                    } else {
                        c.to_string()
                    }
                });
                typed.into_bytes()
            }
        }
    };
    Some(bytes)
}

fn csi_or_ss3(k: &Key, final_byte: char) -> Vec<u8> {
    let m = modifier_param(k);
    if m > 1 {
        format!("\x1b[1;{m}{final_byte}").into_bytes()
    } else {
        format!("\x1bO{final_byte}").into_bytes()
    }
}

/// Bytes for a paste. With bracketed paste mode on, the text is wrapped in ESC[200~ and ESC[201~, and any end marker
/// inside the text is removed so a paste cannot end the bracket early. Without it, newlines become carriage returns,
/// as a typed Enter would be.
pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    if bracketed {
        let clean = text.replace("\x1b[201~", "").replace("\x1b[200~", "");
        let mut out = b"\x1b[200~".to_vec();
        out.extend_from_slice(clean.as_bytes());
        out.extend_from_slice(b"\x1b[201~");
        out
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

/// The expected bytes for a set of keystrokes, used by `headless-check` (compared with what a program in the PTY reads)
/// and by the terminal binary's self-test.
pub fn table() -> Vec<(Key<'static>, bool, &'static [u8])> {
    let k = |key: &'static str| Key {
        key,
        key_char: None,
        ..Default::default()
    };
    vec![
        (
            Key {
                key_char: Some("a"),
                ..k("a")
            },
            false,
            b"a",
        ),
        (
            Key {
                key_char: Some("A"),
                shift: true,
                ..k("a")
            },
            false,
            b"A",
        ),
        (
            Key {
                ctrl: true,
                ..k("c")
            },
            false,
            b"\x03",
        ),
        (
            Key {
                ctrl: true,
                ..k("d")
            },
            false,
            b"\x04",
        ),
        (
            Key {
                ctrl: true,
                ..k("z")
            },
            false,
            b"\x1a",
        ),
        (
            Key {
                ctrl: true,
                ..k("\\")
            },
            false,
            b"\x1c",
        ),
        (
            Key {
                ctrl: true,
                ..k("space")
            },
            false,
            b"\x00",
        ),
        (
            Key {
                alt: true,
                ..k("b")
            },
            false,
            b"\x1bb",
        ),
        (
            Key {
                alt: true,
                ctrl: true,
                ..k("x")
            },
            false,
            b"\x1b\x18",
        ),
        (k("enter"), false, b"\r"),
        (k("tab"), false, b"\t"),
        (
            Key {
                shift: true,
                ..k("tab")
            },
            false,
            b"\x1b[Z",
        ),
        (k("backspace"), false, b"\x7f"),
        (
            Key {
                alt: true,
                ..k("backspace")
            },
            false,
            b"\x1b\x7f",
        ),
        (k("escape"), false, b"\x1b"),
        (k("up"), false, b"\x1b[A"),
        (k("down"), false, b"\x1b[B"),
        (k("right"), false, b"\x1b[C"),
        (k("left"), false, b"\x1b[D"),
        (k("up"), true, b"\x1bOA"),
        (k("left"), true, b"\x1bOD"),
        (
            Key {
                ctrl: true,
                ..k("right")
            },
            false,
            b"\x1b[1;5C",
        ),
        (
            Key {
                shift: true,
                ..k("left")
            },
            false,
            b"\x1b[1;2D",
        ),
        (
            Key {
                alt: true,
                ..k("up")
            },
            false,
            b"\x1b[1;3A",
        ),
        (k("home"), false, b"\x1b[H"),
        (k("end"), false, b"\x1b[F"),
        (k("pageup"), false, b"\x1b[5~"),
        (k("delete"), false, b"\x1b[3~"),
        (
            Key {
                ctrl: true,
                ..k("delete")
            },
            false,
            b"\x1b[3;5~",
        ),
        (k("f1"), false, b"\x1bOP"),
        (k("f5"), false, b"\x1b[15~"),
        (k("f12"), false, b"\x1b[24~"),
        (
            Key {
                key_char: Some("\u{e9}"),
                ..k("\u{e9}")
            },
            false,
            "\u{e9}".as_bytes(),
        ),
    ]
}
