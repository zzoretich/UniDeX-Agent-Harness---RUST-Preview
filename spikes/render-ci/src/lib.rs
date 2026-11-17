//! Shared, GPUI-free parts of the rendering trial: the scene's fixed layout, the frame check, and the line-delimited
//! JSON framing the automation socket uses.
//!
//! The app draws solid blocks of known colours at known places, a line of text and a button. The frame check finds
//! the red block in any capture (an in-app readback or a capture of a whole virtual display), works out the origin
//! and the scale factor from it, then samples the other blocks, the background, the button and the text's region.

use image::{Rgb, RgbaImage};
use serde_json::{Value, json};
use std::io::{self, BufRead};

/// The window's content size in logical pixels.
pub const WINDOW_W: f32 = 480.0;
pub const WINDOW_H: f32 = 320.0;

pub const BACKGROUND: u32 = 0xf5f5f5;
pub const RED: u32 = 0xd32f2f;
pub const GREEN: u32 = 0x2e7d32;
pub const BLUE: u32 = 0x1565c0;
/// The state block: grey until the button has been clicked, orange after.
pub const STATE_IDLE: u32 = 0x9e9e9e;
pub const STATE_CLICKED: u32 = 0xef6c00;
pub const BUTTON: u32 = 0x2563eb;
pub const TEXT: u32 = 0x111111;

/// One solid block: left, top, side (logical pixels).
pub const BLOCK_SIDE: f32 = 60.0;
pub const RED_AT: (f32, f32) = (20.0, 20.0);
pub const GREEN_AT: (f32, f32) = (100.0, 20.0);
pub const BLUE_AT: (f32, f32) = (180.0, 20.0);
pub const STATE_AT: (f32, f32) = (260.0, 20.0);

/// The line of text the check looks for, and the region it must fall in (left, top, right, bottom).
pub const TEXT_LINE: &str = "Render check 0123 ABC xyz";
pub const TEXT_AT: (f32, f32) = (20.0, 100.0);
pub const TEXT_SIZE: f32 = 20.0;
pub const TEXT_REGION: (f32, f32, f32, f32) = (20.0, 96.0, 340.0, 132.0);

/// The button: left, top, width, height.
pub const BUTTON_RECT: (f32, f32, f32, f32) = (20.0, 150.0, 120.0, 40.0);
pub const LABEL_AT: (f32, f32) = (20.0, 210.0);

/// An empty spot that must show the background.
pub const EMPTY_AT: (f32, f32) = (400.0, 120.0);

/// Per-channel tolerance for solid colours. Wide enough for colour-space and rasteriser differences, narrow enough to
/// tell red, green, blue, grey and orange apart.
pub const COLOUR_TOLERANCE: i32 = 40;

/// The largest request or response line the socket accepts (a frame as base64 PNG fits easily).
pub const MAX_LINE: usize = 32 * 1024 * 1024;
/// The largest request line the app accepts. Requests are small; anything longer is refused.
pub const MAX_REQUEST: usize = 64 * 1024;

pub fn rgb(hex: u32) -> [u8; 3] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8]
}

fn close(a: [u8; 3], b: [u8; 3], tolerance: i32) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| (*x as i32 - *y as i32).abs() <= tolerance)
}

fn luminance(p: [u8; 3]) -> f32 {
    0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
}

fn pixel(img: &RgbaImage, x: i64, y: i64) -> Option<[u8; 3]> {
    if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
        return None;
    }
    let p = img.get_pixel(x as u32, y as u32).0;
    Some([p[0], p[1], p[2]])
}

/// Mean colour of a (2r+1)-square patch, or None if it falls outside the image.
fn patch_mean(img: &RgbaImage, cx: i64, cy: i64, r: i64) -> Option<[u8; 3]> {
    let mut sum = [0u64; 3];
    let mut n = 0u64;
    for y in cy - r..=cy + r {
        for x in cx - r..=cx + r {
            let p = pixel(img, x, y)?;
            for c in 0..3 {
                sum[c] += p[c] as u64;
            }
            n += 1;
        }
    }
    Some([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8])
}

fn hex(p: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", p[0], p[1], p[2])
}

/// The result of checking one frame: a list of named checks and an overall verdict.
pub struct FrameReport {
    pub ok: bool,
    pub json: Value,
}

/// Checks a captured frame against the known scene. `state_clicked` says which colour the state block should show.
///
/// What it catches: a blank or uniform frame, a frame from the wrong window, missing solid fills, swapped colour
/// channels, a wrong scale factor, and text that did not draw at all (no glyph pixels where the line should be).
/// What it cannot catch: which font was used, whether glyphs are correct or legible, antialiasing quality, subpixel
/// rendering, or anything outside the sampled points.
pub fn check_frame(img: &RgbaImage, state_clicked: bool) -> FrameReport {
    let mut checks = Vec::new();
    let mut ok = true;
    let mut push = |name: &str, pass: bool, detail: Value| {
        if !pass {
            ok = false;
        }
        checks.push(json!({ "check": name, "ok": pass, "detail": detail }));
    };

    // Distinct colours: a blank frame has one or two.
    let mut distinct = std::collections::HashSet::new();
    for p in img.pixels() {
        distinct.insert([p.0[0], p.0[1], p.0[2]]);
        if distinct.len() > 4096 {
            break;
        }
    }
    push(
        "not_uniform",
        distinct.len() >= 6,
        json!({ "distinct_colours_seen": distinct.len() }),
    );

    // Find the red block: the bounding box of pixels close to red.
    let red = rgb(RED);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut red_count = 0u64;
    for (x, y, p) in img.enumerate_pixels() {
        if close([p.0[0], p.0[1], p.0[2]], red, COLOUR_TOLERANCE) {
            red_count += 1;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if red_count == 0 {
        push(
            "red_block_found",
            false,
            json!({ "reason": "no pixel close to the red block's colour" }),
        );
        return FrameReport {
            ok: false,
            json: json!({ "ok": false, "image": [img.width(), img.height()], "checks": checks }),
        };
    }
    let bw = (max_x - min_x + 1) as f32;
    let bh = (max_y - min_y + 1) as f32;
    let scale = bw / BLOCK_SIDE;
    let square = (bw - bh).abs() <= (0.1 * bw).max(2.0);
    let filled = red_count as f32 >= 0.8 * bw * bh;
    push(
        "red_block_found",
        square && filled && scale >= 0.5,
        json!({ "bbox": [min_x, min_y, max_x, max_y], "pixels": red_count, "scale": scale }),
    );
    let origin_x = min_x as f32 - RED_AT.0 * scale;
    let origin_y = min_y as f32 - RED_AT.1 * scale;
    let at = |lx: f32, ly: f32| -> (i64, i64) {
        (
            (origin_x + lx * scale).round() as i64,
            (origin_y + ly * scale).round() as i64,
        )
    };
    let r = (3.0 * scale).round().max(1.0) as i64;

    let mut sample = |name: &str, lx: f32, ly: f32, expected: u32, tolerance: i32| {
        let (x, y) = at(lx, ly);
        let got = patch_mean(img, x, y, r);
        let pass = got.is_some_and(|g| close(g, rgb(expected), tolerance));
        push(
            name,
            pass,
            json!({
                "at_device_px": [x, y],
                "expected": hex(rgb(expected)),
                "got": got.map(hex),
            }),
        );
    };
    let half = BLOCK_SIDE / 2.0;
    sample(
        "green_block",
        GREEN_AT.0 + half,
        GREEN_AT.1 + half,
        GREEN,
        COLOUR_TOLERANCE,
    );
    sample(
        "blue_block",
        BLUE_AT.0 + half,
        BLUE_AT.1 + half,
        BLUE,
        COLOUR_TOLERANCE,
    );
    let state = if state_clicked {
        STATE_CLICKED
    } else {
        STATE_IDLE
    };
    sample(
        "state_block",
        STATE_AT.0 + half,
        STATE_AT.1 + half,
        state,
        COLOUR_TOLERANCE,
    );
    sample("background", EMPTY_AT.0, EMPTY_AT.1, BACKGROUND, 20);
    // Inside the button, clear of its label.
    sample(
        "button_fill",
        BUTTON_RECT.0 + 8.0,
        BUTTON_RECT.1 + 8.0,
        BUTTON,
        COLOUR_TOLERANCE,
    );

    // Text: dark pixels inside the line's region. Glyph shapes differ per platform and rasteriser, so this only asks
    // that a plausible amount of ink is there and that the region is not filled solid.
    let (x0, y0) = at(TEXT_REGION.0, TEXT_REGION.1);
    let (x1, y1) = at(TEXT_REGION.2, TEXT_REGION.3);
    let mut dark = 0u64;
    let mut total = 0u64;
    let mut ink_min_x = i64::MAX;
    let mut ink_max_x = i64::MIN;
    for y in y0..y1 {
        for x in x0..x1 {
            if let Some(p) = pixel(img, x, y) {
                total += 1;
                if luminance(p) < 128.0 {
                    dark += 1;
                    ink_min_x = ink_min_x.min(x);
                    ink_max_x = ink_max_x.max(x);
                }
            }
        }
    }
    let fraction = if total > 0 {
        dark as f64 / total as f64
    } else {
        0.0
    };
    let ink_width_logical = if dark > 0 {
        (ink_max_x - ink_min_x + 1) as f32 / scale
    } else {
        0.0
    };
    // A 25-character line at 20 px is roughly 200 to 300 logical pixels wide in any proportional UI font.
    let pass = total > 0 && (0.02..0.6).contains(&fraction) && ink_width_logical >= 120.0;
    push(
        "text_ink",
        pass,
        json!({
            "region_device_px": [x0, y0, x1, y1],
            "dark_pixels": dark,
            "fraction": fraction,
            "ink_width_logical_px": ink_width_logical,
        }),
    );

    FrameReport {
        ok,
        json: json!({
            "ok": ok,
            "image": [img.width(), img.height()],
            "scale_found": scale,
            "origin_device_px": [origin_x, origin_y],
            "checks": checks,
        }),
    }
}

/// Reads one `\n`-terminated line of at most `max` bytes. Returns Ok(None) at end of stream. A longer line is an
/// error, so a peer cannot make the reader buffer without bound.
pub fn read_line_limited(reader: &mut impl BufRead, max: usize) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if buf.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "line not terminated",
                ))
            };
        }
        if let Some(pos) = available.iter().position(|b| *b == b'\n') {
            buf.extend_from_slice(&available[..pos]);
            reader.consume(pos + 1);
            break;
        }
        let n = available.len();
        buf.extend_from_slice(available);
        reader.consume(n);
        if buf.len() > max {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "line too long"));
        }
    }
    if buf.len() > max {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "line too long"));
    }
    String::from_utf8(buf)
        .map(Some)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "line is not UTF-8"))
}

/// Converts a packed 0xRRGGBB colour to an `image` pixel, for tests and tools.
pub fn rgb_pixel(hex: u32) -> Rgb<u8> {
    Rgb(rgb(hex))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paints the scene's solid parts at a given scale and offset, as a renderer would.
    fn synthetic(scale: f32, offset: (u32, u32), clicked: bool, with_text: bool) -> RgbaImage {
        let w = (WINDOW_W * scale) as u32 + offset.0 + 10;
        let h = (WINDOW_H * scale) as u32 + offset.1 + 10;
        let mut img = RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]));
        let mut fill = |l: f32, t: f32, wd: f32, ht: f32, c: u32| {
            let c = rgb(c);
            for y in (t * scale) as u32..((t + ht) * scale) as u32 {
                for x in (l * scale) as u32..((l + wd) * scale) as u32 {
                    img.put_pixel(
                        x + offset.0,
                        y + offset.1,
                        image::Rgba([c[0], c[1], c[2], 255]),
                    );
                }
            }
        };
        fill(0.0, 0.0, WINDOW_W, WINDOW_H, BACKGROUND);
        fill(RED_AT.0, RED_AT.1, BLOCK_SIDE, BLOCK_SIDE, RED);
        fill(GREEN_AT.0, GREEN_AT.1, BLOCK_SIDE, BLOCK_SIDE, GREEN);
        fill(BLUE_AT.0, BLUE_AT.1, BLOCK_SIDE, BLOCK_SIDE, BLUE);
        let state = if clicked { STATE_CLICKED } else { STATE_IDLE };
        fill(STATE_AT.0, STATE_AT.1, BLOCK_SIDE, BLOCK_SIDE, state);
        fill(
            BUTTON_RECT.0,
            BUTTON_RECT.1,
            BUTTON_RECT.2,
            BUTTON_RECT.3,
            BUTTON,
        );
        if with_text {
            // Fake glyphs: vertical strokes along the line.
            for i in 0..25 {
                fill(TEXT_AT.0 + i as f32 * 9.0, TEXT_AT.1 + 4.0, 3.0, 14.0, TEXT);
            }
        }
        img
    }

    #[test]
    fn passes_a_correct_frame_at_several_scales_and_offsets() {
        for (scale, offset) in [
            (1.0, (0, 0)),
            (2.0, (0, 0)),
            (1.0, (37, 51)),
            (1.5, (8, 30)),
        ] {
            let img = synthetic(scale, offset, false, true);
            let report = check_frame(&img, false);
            assert!(
                report.ok,
                "scale {scale} offset {offset:?}: {}",
                report.json
            );
        }
    }

    #[test]
    fn fails_without_text() {
        let img = synthetic(1.0, (0, 0), false, false);
        assert!(!check_frame(&img, false).ok);
    }

    #[test]
    fn fails_on_wrong_state_colour() {
        let img = synthetic(1.0, (0, 0), true, true);
        assert!(!check_frame(&img, false).ok);
        assert!(check_frame(&img, true).ok);
    }

    #[test]
    fn fails_on_blank_frame() {
        let img = RgbaImage::from_pixel(480, 320, image::Rgba([245, 245, 245, 255]));
        assert!(!check_frame(&img, false).ok);
    }

    #[test]
    fn fails_on_swapped_channels() {
        let mut img = synthetic(1.0, (0, 0), false, true);
        for p in img.pixels_mut() {
            p.0.swap(0, 2);
        }
        assert!(!check_frame(&img, false).ok);
    }

    #[test]
    fn line_reader_limits_length() {
        let mut ok = io::Cursor::new(b"{\"cmd\":\"hello\"}\nrest".to_vec());
        assert_eq!(
            read_line_limited(&mut ok, 100).unwrap().as_deref(),
            Some("{\"cmd\":\"hello\"}")
        );
        let mut long = io::Cursor::new(vec![b'a'; 200]);
        assert!(read_line_limited(&mut long, 100).is_err());
    }
}
