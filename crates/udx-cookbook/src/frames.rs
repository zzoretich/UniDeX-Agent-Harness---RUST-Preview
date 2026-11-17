//! The window-free half of drawing frames that another process produces (decision 0012).
//!
//! In the browser trial a helper process paints each frame into one of a few shared-memory slots and names the slot in
//! a message on a pipe; the app checks the message, copies the pixels out, hands the slot back with a message of its
//! own, and keeps only the newest frame for the UI thread. This module holds the parts of that path that need no
//! window and no operating-system memory: the size check made before any pixel is touched, the message, and the
//! single slot where the newest frame waits.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

/// How the shared slots are laid out: how many there are and the largest frame each holds, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotLayout {
    pub slots: u32,
    pub max_width: u32,
    pub max_height: u32,
}

/// Why a frame description was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameError {
    SlotOutOfRange { slot: u32, slots: u32 },
    BadSize { width: u32, height: u32 },
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::SlotOutOfRange { slot, slots } => {
                write!(f, "slot {slot} out of range (the region has {slots})")
            }
            FrameError::BadSize { width, height } => {
                write!(f, "frame size {width}x{height} refused")
            }
        }
    }
}

impl SlotLayout {
    /// Bytes in one slot: the largest frame, four bytes a pixel (BGRA); `None` if that does not fit in `usize`.
    pub fn slot_bytes(&self) -> Option<usize> {
        frame_bytes(self.max_width, self.max_height)
    }

    /// Checks a frame description from the other side before any pixel is touched, and returns the frame's length
    /// in bytes. Nothing the peer says is trusted: the slot must exist and the size must fit in it.
    pub fn check(&self, slot: u32, width: u32, height: u32) -> Result<usize, FrameError> {
        if slot >= self.slots {
            return Err(FrameError::SlotOutOfRange {
                slot,
                slots: self.slots,
            });
        }
        if width == 0 || height == 0 || width > self.max_width || height > self.max_height {
            return Err(FrameError::BadSize { width, height });
        }
        frame_bytes(width, height).ok_or(FrameError::BadSize { width, height })
    }
}

/// Four bytes a pixel, with every step checked so a large size is refused instead of wrapping or panicking.
fn frame_bytes(width: u32, height: u32) -> Option<usize> {
    (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)
}

/// "A frame is ready in this slot": `frame <slot> <seq> <width> <height>`, one line, decimal numbers.
///
/// The trial used JSON lines with a size limit; a real channel uses a fixed binary schema with a fuzzed decoder
/// (threat model P53). This text form keeps the example free of a serialisation crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameMsg {
    pub slot: u32,
    pub seq: u64,
    pub width: u32,
    pub height: u32,
}

impl FrameMsg {
    pub fn to_line(&self) -> String {
        format!(
            "frame {} {} {} {}",
            self.slot, self.seq, self.width, self.height
        )
    }

    /// Parses exactly the form [`Self::to_line`] writes; anything else is `None`.
    pub fn parse(line: &str) -> Option<Self> {
        let mut fields = line.split(' ');
        if fields.next()? != "frame" {
            return None;
        }
        let msg = FrameMsg {
            slot: decimal(fields.next()?)?,
            seq: decimal(fields.next()?)?,
            width: decimal(fields.next()?)?,
            height: decimal(fields.next()?)?,
        };
        fields.next().is_none().then_some(msg)
    }
}

/// The newest frame, waiting for the UI thread.
///
/// A frame that arrives while the previous one is still waiting replaces it, so a UI that falls behind drops frames
/// instead of queueing them, and the reader wakes the UI only when the slot was empty: one wake-up per frame the UI
/// takes, however many arrive in between.
#[derive(Debug, Default)]
pub struct LatestFrame<T> {
    slot: Mutex<Option<T>>,
    superseded: AtomicU64,
}

impl<T> LatestFrame<T> {
    pub fn new() -> Self {
        LatestFrame {
            slot: Mutex::new(None),
            superseded: AtomicU64::new(0),
        }
    }

    /// Stores `frame`. Returns true when the UI must be woken (the slot was empty); false when a waiting frame was
    /// replaced, because the UI is already about to take one.
    pub fn put(&self, frame: T) -> bool {
        let previous = self
            .slot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(frame);
        if previous.is_some() {
            self.superseded.fetch_add(1, Ordering::Relaxed);
        }
        previous.is_none()
    }

    pub fn take(&self) -> Option<T> {
        self.slot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// Frames replaced before the UI took them.
    pub fn superseded(&self) -> u64 {
        self.superseded.load(Ordering::Relaxed)
    }
}

/// A plain decimal number: digits only, no sign, no exponent, no surrounding space.
fn decimal<N: std::str::FromStr>(field: &str) -> Option<N> {
    if field.is_empty() || !field.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    field.parse().ok()
}

/// Wraps BGRA pixels (packed rows of `width * 4` bytes) as a binary PPM image (P6, alpha dropped).
///
/// Only for the cookbook: GPUI's `RenderImage::new` takes frames of the `image` crate, which the cookbook does not
/// depend on, so the `frame_pane` example hands GPUI this uncompressed image to decode. The trial and the real
/// application wrap the BGRA buffer directly and skip this extra pass over the pixels.
pub fn bgra_to_ppm(bgra: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let pixels = (width as usize).checked_mul(height as usize)?;
    if bgra.len() != pixels.checked_mul(4)? {
        return None;
    }
    let header = format!("P6\n{width} {height}\n255\n");
    let mut ppm = Vec::with_capacity(header.len() + pixels * 3);
    ppm.extend_from_slice(header.as_bytes());
    for pixel in bgra.as_chunks::<4>().0 {
        ppm.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
    }
    Some(ppm)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYOUT: SlotLayout = SlotLayout {
        slots: 3,
        max_width: 640,
        max_height: 400,
    };

    #[test]
    fn a_frame_that_fits_gives_its_length() {
        assert_eq!(LAYOUT.slot_bytes(), Some(640 * 400 * 4));
        assert_eq!(LAYOUT.check(0, 640, 400), Ok(640 * 400 * 4));
        assert_eq!(LAYOUT.check(2, 1, 1), Ok(4));
    }

    #[test]
    fn a_layout_too_large_to_measure_is_refused_not_overflowed() {
        let huge = SlotLayout {
            slots: 1,
            max_width: u32::MAX,
            max_height: u32::MAX,
        };
        assert_eq!(huge.slot_bytes(), None);
        assert_eq!(
            huge.check(0, u32::MAX, u32::MAX),
            Err(FrameError::BadSize {
                width: u32::MAX,
                height: u32::MAX
            })
        );
    }

    #[test]
    fn refuses_a_missing_slot_and_sizes_that_do_not_fit() {
        assert_eq!(
            LAYOUT.check(3, 10, 10),
            Err(FrameError::SlotOutOfRange { slot: 3, slots: 3 })
        );
        for (width, height) in [(0, 10), (10, 0), (641, 10), (10, 401), (u32::MAX, u32::MAX)] {
            assert_eq!(
                LAYOUT.check(0, width, height),
                Err(FrameError::BadSize { width, height })
            );
        }
    }

    #[test]
    fn frame_messages_round_trip_and_nothing_else_parses() {
        let msg = FrameMsg {
            slot: 2,
            seq: 12_345_678_901,
            width: 640,
            height: 400,
        };
        assert_eq!(msg.to_line(), "frame 2 12345678901 640 400");
        assert_eq!(FrameMsg::parse(&msg.to_line()), Some(msg));
        for line in [
            "",
            "frame 2 1 640",
            "frame 2 1 640 400 9",
            "frames 2 1 640 400",
            "frame -2 1 640 400",
            "frame +2 1 640 400",
            "frame 2 1 640 4e2",
            "frame 4294967296 1 640 400",
            "frame 2 1 640 400\n",
        ] {
            assert_eq!(FrameMsg::parse(line), None, "{line:?}");
        }
    }

    #[test]
    fn the_newest_frame_replaces_one_still_waiting() {
        let latest = LatestFrame::new();
        assert!(latest.put(1));
        assert!(!latest.put(2));
        assert_eq!(latest.superseded(), 1);
        assert_eq!(latest.take(), Some(2));
        assert_eq!(latest.take(), None);
        assert!(latest.put(3));
        assert_eq!(latest.take(), Some(3));
    }

    #[test]
    fn ppm_carries_the_pixels_in_rgb_order() {
        // Two pixels, BGRA: pure blue, then pure red.
        let bgra = [255, 0, 0, 255, 0, 0, 255, 255];
        let ppm = bgra_to_ppm(&bgra, 2, 1).unwrap();
        let header = b"P6\n2 1\n255\n";
        assert_eq!(&ppm[..header.len()], header);
        assert_eq!(&ppm[header.len()..], &[0, 0, 255, 255, 0, 0]);
        assert_eq!(bgra_to_ppm(&bgra, 3, 1), None);
        assert_eq!(bgra_to_ppm(&bgra, u32::MAX, u32::MAX), None);
    }

    /// What the `frame_pane` example relies on: GPUI decodes the PPM into a `RenderImage` holding the producer's BGRA
    /// pixels unchanged. No window is needed (GPUI is this crate's dev-dependency).
    #[test]
    fn gpui_decodes_the_ppm_back_to_the_same_bgra_pixels() {
        use gpui::{DevicePixels, Image, ImageFormat, SvgRenderer, size};
        let bgra: Vec<u8> = (0..6u8)
            .flat_map(|i| [i * 40, 255 - i * 40, i * 7, 255])
            .collect();
        let ppm = bgra_to_ppm(&bgra, 3, 2).unwrap();
        let decoder = SvgRenderer::new(std::sync::Arc::new(()));
        let image = Image::from_bytes(ImageFormat::Pnm, ppm)
            .to_image_data(decoder)
            .unwrap();
        assert_eq!(image.size(0), size(DevicePixels(3), DevicePixels(2)));
        assert_eq!(image.as_bytes(0), Some(&bgra[..]));
    }
}
