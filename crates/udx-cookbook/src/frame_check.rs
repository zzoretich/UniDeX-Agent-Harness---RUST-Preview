//! A structural check of a captured frame against a known scene (decision 0013).
//!
//! The scene is solid blocks of known colours at known places, a line of text and a button; the `frame_check_scene`
//! example draws it from the constants below. The check finds the red block anywhere in a capture (an in-app readback
//! or a capture of a whole virtual display), works out the origin and the scale factor from it, then samples the other
//! blocks, the background, the button and the text's region.
//!
//! It catches a blank, uniform or black frame, the wrong window, missing fills, swapped colour channels, a wrong scale
//! and text that did not draw. It cannot tell which font was used, whether glyphs are correct or legible, how edges
//! were antialiased, or anything outside the sampled points. Pixel-exact baselines across platforms would be brittle
//! for the same reasons (fonts and rasterisers differ), which is why the check samples patches away from edges.

use std::collections::HashSet;

/// The window's content size, in logical pixels.
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

/// Every block is a square of this side, placed by its top left corner (logical pixels).
pub const BLOCK_SIDE: f32 = 60.0;
pub const RED_AT: (f32, f32) = (20.0, 20.0);
pub const GREEN_AT: (f32, f32) = (100.0, 20.0);
pub const BLUE_AT: (f32, f32) = (180.0, 20.0);
pub const STATE_AT: (f32, f32) = (260.0, 20.0);

/// The line of text, where it is drawn, and the region its ink must fall in (left, top, right, bottom).
pub const TEXT_LINE: &str = "Render check 0123 ABC xyz";
pub const TEXT_AT: (f32, f32) = (20.0, 100.0);
pub const TEXT_SIZE: f32 = 20.0;
pub const TEXT_REGION: (f32, f32, f32, f32) = (20.0, 96.0, 340.0, 132.0);

/// The button (left, top, width, height) and the label that counts its clicks.
pub const BUTTON_RECT: (f32, f32, f32, f32) = (20.0, 150.0, 120.0, 40.0);
pub const LABEL_AT: (f32, f32) = (20.0, 210.0);

/// An empty spot that must show the background.
pub const EMPTY_AT: (f32, f32) = (400.0, 120.0);

/// Per-channel tolerance for solid colours: wide enough for colour-space and rasteriser differences, narrow enough to
/// tell red, green, blue, grey and orange apart.
pub const COLOUR_TOLERANCE: i32 = 40;

/// A packed `0xRRGGBB` colour as bytes.
pub fn rgb_bytes(hex: u32) -> [u8; 3] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8]
}

/// A captured frame: RGBA, 8 bits a channel, packed rows.
#[derive(Clone, Copy, Debug)]
pub struct Rgba8<'a> {
    width: u32,
    height: u32,
    pixels: &'a [u8],
}

impl<'a> Rgba8<'a> {
    /// `None` unless `pixels` holds exactly `width * height` pixels.
    pub fn new(width: u32, height: u32, pixels: &'a [u8]) -> Option<Self> {
        let len = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        (pixels.len() == len).then_some(Rgba8 {
            width,
            height,
            pixels,
        })
    }

    fn pixel(&self, x: i64, y: i64) -> Option<[u8; 3]> {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return None;
        }
        let at = (y as usize * self.width as usize + x as usize) * 4;
        Some([self.pixels[at], self.pixels[at + 1], self.pixels[at + 2]])
    }

    /// Mean colour of the square patch of side `2r + 1` around a point, or `None` if it leaves the frame.
    fn patch_mean(&self, cx: i64, cy: i64, r: i64) -> Option<[u8; 3]> {
        let mut sum = [0u64; 3];
        let mut n = 0u64;
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                let p = self.pixel(x, y)?;
                for c in 0..3 {
                    sum[c] += u64::from(p[c]);
                }
                n += 1;
            }
        }
        Some([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8])
    }

    fn rgb_pixels(&self) -> impl Iterator<Item = [u8; 3]> + '_ {
        self.pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| [p[0], p[1], p[2]])
    }
}

fn close(a: [u8; 3], b: [u8; 3], tolerance: i32) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| (i32::from(*x) - i32::from(*y)).abs() <= tolerance)
}

fn luminance(p: [u8; 3]) -> f32 {
    0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])
}

fn hex(p: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", p[0], p[1], p[2])
}

/// One named check and what it saw.
#[derive(Clone, Debug)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

/// The verdict on one frame.
#[derive(Clone, Debug)]
pub struct FrameReport {
    pub ok: bool,
    /// The scale factor found from the red block, if it was found.
    pub scale: Option<f32>,
    pub checks: Vec<Check>,
}

/// Checks a captured frame against the scene. `state_clicked` says which colour the state block should show.
pub fn check_frame(frame: &Rgba8<'_>, state_clicked: bool) -> FrameReport {
    let mut checks = Vec::new();
    let mut push =
        |name: &'static str, ok: bool, detail: String| checks.push(Check { name, ok, detail });

    // Distinct colours: a blank frame has one or two.
    let mut distinct = HashSet::new();
    for p in frame.rgb_pixels() {
        distinct.insert(p);
        if distinct.len() > 4096 {
            break;
        }
    }
    push(
        "not_uniform",
        distinct.len() >= 6,
        format!("{} distinct colours seen", distinct.len()),
    );

    // Find the red block: the bounding box of every pixel close to red.
    let red = rgb_bytes(RED);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut red_count = 0u64;
    for (i, p) in frame.rgb_pixels().enumerate() {
        if close(p, red, COLOUR_TOLERANCE) {
            let (x, y) = (
                (i % frame.width as usize) as u32,
                (i / frame.width as usize) as u32,
            );
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
            "no pixel close to the red block's colour".into(),
        );
        return FrameReport {
            ok: false,
            scale: None,
            checks,
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
        format!("box {min_x},{min_y} to {max_x},{max_y}, {red_count} pixels, scale {scale}"),
    );
    let origin = (
        min_x as f32 - RED_AT.0 * scale,
        min_y as f32 - RED_AT.1 * scale,
    );
    let at = |lx: f32, ly: f32| -> (i64, i64) {
        (
            (origin.0 + lx * scale).round() as i64,
            (origin.1 + ly * scale).round() as i64,
        )
    };
    let r = (3.0 * scale).round().max(1.0) as i64;

    let mut sample = |name: &'static str, l: (f32, f32), expected: u32, tolerance: i32| {
        let (x, y) = at(l.0, l.1);
        let got = frame.patch_mean(x, y, r);
        let ok = got.is_some_and(|g| close(g, rgb_bytes(expected), tolerance));
        let seen = got.map_or("outside the frame".to_string(), hex);
        push(
            name,
            ok,
            format!(
                "at {x},{y}: expected {}, got {seen}",
                hex(rgb_bytes(expected))
            ),
        );
    };
    let half = BLOCK_SIDE / 2.0;
    let centre = |corner: (f32, f32)| (corner.0 + half, corner.1 + half);
    sample("green_block", centre(GREEN_AT), GREEN, COLOUR_TOLERANCE);
    sample("blue_block", centre(BLUE_AT), BLUE, COLOUR_TOLERANCE);
    let state = if state_clicked {
        STATE_CLICKED
    } else {
        STATE_IDLE
    };
    sample("state_block", centre(STATE_AT), state, COLOUR_TOLERANCE);
    sample("background", EMPTY_AT, BACKGROUND, 20);
    // Inside the button, clear of its label.
    let inside_button = (BUTTON_RECT.0 + 8.0, BUTTON_RECT.1 + 8.0);
    sample("button_fill", inside_button, BUTTON, COLOUR_TOLERANCE);

    // Text: dark pixels inside the line's region. Glyph shapes differ per platform and rasteriser, so this asks only
    // that a plausible amount of ink is there and that the region is not filled solid.
    let (x0, y0) = at(TEXT_REGION.0, TEXT_REGION.1);
    let (x1, y1) = at(TEXT_REGION.2, TEXT_REGION.3);
    let (mut dark, mut total) = (0u64, 0u64);
    let (mut ink_min_x, mut ink_max_x) = (i64::MAX, i64::MIN);
    for y in y0..y1 {
        for x in x0..x1 {
            if let Some(p) = frame.pixel(x, y) {
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
    let ink_width = if dark > 0 {
        (ink_max_x - ink_min_x + 1) as f32 / scale
    } else {
        0.0
    };
    // A 25-character line at 20 px is roughly 200 to 300 logical pixels wide in any proportional UI font.
    push(
        "text_ink",
        total > 0 && (0.02..0.6).contains(&fraction) && ink_width >= 120.0,
        format!(
            "{dark} dark pixels ({fraction:.3} of the region), ink {ink_width} logical px wide"
        ),
    );

    FrameReport {
        ok: checks.iter().all(|c| c.ok),
        scale: Some(scale),
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic capture: the scene's solid parts at a scale and offset, on black, as a renderer would draw them.
    struct Canvas {
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    }

    impl Canvas {
        fn frame(&self) -> Rgba8<'_> {
            Rgba8::new(self.width, self.height, &self.pixels).unwrap()
        }
    }

    fn synthetic(scale: f32, offset: (u32, u32), clicked: bool, with_text: bool) -> Canvas {
        let width = (WINDOW_W * scale) as u32 + offset.0 + 10;
        let height = (WINDOW_H * scale) as u32 + offset.1 + 10;
        let mut pixels = [0u8, 0, 0, 255].repeat((width * height) as usize);
        let mut fill = |l: f32, t: f32, w: f32, h: f32, colour: u32| {
            let c = rgb_bytes(colour);
            for y in (t * scale) as u32..((t + h) * scale) as u32 {
                for x in (l * scale) as u32..((l + w) * scale) as u32 {
                    let at = (((y + offset.1) * width + x + offset.0) * 4) as usize;
                    pixels[at..at + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
                }
            }
        };
        fill(0.0, 0.0, WINDOW_W, WINDOW_H, BACKGROUND);
        fill(RED_AT.0, RED_AT.1, BLOCK_SIDE, BLOCK_SIDE, RED);
        fill(GREEN_AT.0, GREEN_AT.1, BLOCK_SIDE, BLOCK_SIDE, GREEN);
        fill(BLUE_AT.0, BLUE_AT.1, BLOCK_SIDE, BLOCK_SIDE, BLUE);
        let state = if clicked { STATE_CLICKED } else { STATE_IDLE };
        fill(STATE_AT.0, STATE_AT.1, BLOCK_SIDE, BLOCK_SIDE, state);
        let (bl, bt, bw, bh) = BUTTON_RECT;
        fill(bl, bt, bw, bh, BUTTON);
        if with_text {
            // Fake glyphs: vertical strokes along the line.
            for i in 0..25 {
                fill(TEXT_AT.0 + i as f32 * 9.0, TEXT_AT.1 + 4.0, 3.0, 14.0, TEXT);
            }
        }
        Canvas {
            width,
            height,
            pixels,
        }
    }

    fn failed(report: &FrameReport) -> Vec<&'static str> {
        report
            .checks
            .iter()
            .filter(|c| !c.ok)
            .map(|c| c.name)
            .collect()
    }

    #[test]
    fn passes_a_correct_frame_at_several_scales_and_offsets() {
        for (scale, offset) in [
            (1.0, (0, 0)),
            (2.0, (0, 0)),
            (1.0, (37, 51)),
            (1.5, (8, 30)),
        ] {
            let canvas = synthetic(scale, offset, false, true);
            let report = check_frame(&canvas.frame(), false);
            assert!(
                report.ok,
                "scale {scale} offset {offset:?}: {:?}",
                report.checks
            );
            assert_eq!(report.scale, Some(scale));
        }
    }

    #[test]
    fn fails_without_text() {
        let canvas = synthetic(1.0, (0, 0), false, false);
        assert_eq!(
            failed(&check_frame(&canvas.frame(), false)),
            vec!["text_ink"]
        );
    }

    #[test]
    fn fails_on_the_wrong_state_colour() {
        let canvas = synthetic(1.0, (0, 0), true, true);
        assert_eq!(
            failed(&check_frame(&canvas.frame(), false)),
            vec!["state_block"]
        );
        assert!(check_frame(&canvas.frame(), true).ok);
    }

    #[test]
    fn fails_on_a_blank_frame() {
        let pixels = [245u8, 245, 245, 255].repeat(480 * 320);
        let report = check_frame(&Rgba8::new(480, 320, &pixels).unwrap(), false);
        assert!(!report.ok);
        assert_eq!(report.scale, None);
    }

    #[test]
    fn fails_on_swapped_channels() {
        let mut canvas = synthetic(1.0, (0, 0), false, true);
        for pixel in canvas.pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        assert!(!check_frame(&canvas.frame(), false).ok);
    }

    #[test]
    fn a_buffer_of_the_wrong_length_is_not_a_frame() {
        assert!(Rgba8::new(2, 2, &[0; 15]).is_none());
        assert!(Rgba8::new(u32::MAX, u32::MAX, &[]).is_none());
    }
}
