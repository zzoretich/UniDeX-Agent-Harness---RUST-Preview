//! What to save so a window reopens where it was, at the same size.
//!
//! The trap (decision 0010, E14, measured on macOS): `Window::window_bounds()` reports the frame including the title
//! bar, while `WindowOptions::window_bounds` takes the size as the content size. Saving the size from
//! `window_bounds()` makes the window grow by the title bar every time it is restored (252 to 284 points in the
//! trial). Save the origin from `window_bounds()` and the size from `Window::viewport_size()`; restoring that gave the
//! same window. Linux and Windows were not measured.

/// A saved window: the origin of its frame and the size of its content, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SavedWindow {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// The smallest and largest side a saved window may have; anything outside is treated as damage.
pub const MIN_SIDE: f32 = 100.0;
pub const MAX_SIDE: f32 = 32_768.0;
/// How far from the main display's origin a saved window may sit. Displays to the left or above give negative
/// coordinates.
pub const MAX_OFFSET: f32 = 1_000_000.0;

impl SavedWindow {
    /// `frame_origin` from `window_bounds()`, `content_size` from `viewport_size()`.
    pub fn new(frame_origin: (f32, f32), content_size: (f32, f32)) -> Self {
        SavedWindow {
            x: frame_origin.0,
            y: frame_origin.1,
            width: content_size.0,
            height: content_size.1,
        }
    }

    /// One line of text, for a settings file.
    pub fn encode(&self) -> String {
        format!("v1 {} {} {} {}", self.x, self.y, self.width, self.height)
    }

    /// Reads a line written by [`Self::encode`]. Anything else, or a window outside the limits above, gives `None`, so
    /// a damaged settings file opens the window at its default place instead.
    pub fn decode(line: &str) -> Option<Self> {
        let mut fields = line.split(' ');
        if fields.next()? != "v1" {
            return None;
        }
        let mut number = || fields.next()?.parse::<f32>().ok().filter(|n| n.is_finite());
        let saved = SavedWindow {
            x: number()?,
            y: number()?,
            width: number()?,
            height: number()?,
        };
        let side = MIN_SIDE..=MAX_SIDE;
        let offset = -MAX_OFFSET..=MAX_OFFSET;
        let plausible = side.contains(&saved.width)
            && side.contains(&saved.height)
            && offset.contains(&saved.x)
            && offset.contains(&saved.y);
        (fields.next().is_none() && plausible).then_some(saved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_including_a_display_left_of_the_main_one() {
        for saved in [
            SavedWindow::new((120.0, 80.0), (640.0, 480.0)),
            SavedWindow::new((-1700.5, -40.0), (900.0, 600.25)),
        ] {
            assert_eq!(SavedWindow::decode(&saved.encode()), Some(saved));
        }
    }

    #[test]
    fn refuses_damaged_or_implausible_lines() {
        for line in [
            "",
            "window v1 1 2 300 200",
            "v2 1 2 300 200",
            "v1 1 2 300",
            "v1 1 2 300 200 9",
            "v1 a 2 300 200",
            "v1 NaN 2 300 200",
            "v1 1 inf 300 200",
            "v1 1 2 99 200",
            "v1 1 2 300 40000",
            "v1 2000000 2 300 200",
        ] {
            assert_eq!(SavedWindow::decode(line), None, "{line:?}");
        }
    }
}
