//! Messages between the app and a helper process.
//!
//! One JSON object per line. The app writes [`ToHelper`] to the helper's stdin and reads
//! [`FromHelper`] from its stdout; the helper's stderr is its log. Both pipes are anonymous and
//! inherited, so no other local process can reach the channel (threat model P53). Pixels never
//! travel on the pipe: a [`FromHelper::Frame`] names a shared-memory slot (see [`crate::shm`]).
//!
//! JSON keeps the trial readable. A real `udx-browser` would use a fixed binary schema with a
//! fuzzed decoder (P53); the limits below stand in for its checks.

use crate::view::{CursorKind, FrameKind, InputEvent, Rect};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Read as _, Write};

/// The longest line either side accepts. DevTools results (a big `Runtime.evaluate` value, a
/// screenshot) are the only large messages.
pub const MAX_LINE: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "msg")]
pub enum ToHelper {
    /// The view's size in logical pixels and the scale factor.
    Resize {
        width: u32,
        height: u32,
        scale_factor: f32,
    },
    Input {
        event: InputEvent,
    },
    /// The app has copied the frame out of this slot; the helper may paint into it again.
    Release {
        slot: u32,
    },
    /// A Chrome DevTools Protocol call, relayed to the engine's own DevTools entry point
    /// (CEF's `SendDevToolsMessage`), never a remote-debugging port (P32).
    DevTools {
        id: u64,
        method: String,
        params: serde_json::Value,
    },
    Shutdown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "msg")]
pub enum FromHelper {
    Ready {
        engine: String,
    },
    /// A frame is ready in `slot`. `width` and `height` are device pixels; the pixels are BGRA,
    /// packed rows. The slot stays the app's until it sends `Release`.
    Frame {
        slot: u32,
        kind: FrameKind,
        seq: u64,
        width: u32,
        height: u32,
        dirty: Vec<Rect>,
        produced_unix_ns: u64,
    },
    Popup {
        show: bool,
        rect: Option<Rect>,
    },
    Cursor {
        cursor: CursorKind,
    },
    ImeBounds {
        range_start: u32,
        range_end: u32,
        characters: Vec<Rect>,
    },
    /// A DevTools message (a result carrying the call's id, or an event), passed through as is.
    DevTools {
        message: serde_json::Value,
    },
    /// Something the engine reports that the app only logs or shows: a load finished, a
    /// renderer ended, a popup, download, file dialog or permission prompt was refused.
    Status {
        text: String,
    },
}

pub fn write_line<T: Serialize>(out: &mut impl Write, message: &T) -> io::Result<()> {
    let mut line = serde_json::to_vec(message).map_err(io::Error::other)?;
    line.push(b'\n');
    out.write_all(&line)?;
    out.flush()
}

/// Reads one message. `Ok(None)` at end of stream. A line longer than [`MAX_LINE`] is an error:
/// the peer is misbehaving and the channel is closed.
pub fn read_line<T: for<'de> Deserialize<'de>>(
    input: &mut impl BufRead,
    buffer: &mut Vec<u8>,
) -> io::Result<Option<T>> {
    buffer.clear();
    let read = input
        .by_ref()
        .take(MAX_LINE as u64 + 1)
        .read_until(b'\n', buffer)?;
    if read == 0 {
        return Ok(None);
    }
    if buffer.len() > MAX_LINE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message longer than the limit",
        ));
    }
    serde_json::from_slice(buffer)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Unix time in nanoseconds, for latency between the two processes on one machine.
pub fn unix_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}
