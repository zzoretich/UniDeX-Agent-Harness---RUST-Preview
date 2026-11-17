//! Stand-in frame source for the browser trial (Task 12). Throwaway.
//!
//! Started by `spike-app` as its helper process; not meant to be run by hand. It paints a moving
//! test pattern (scrolling stripes, a bouncing square, a frame counter, a pointer crosshair, the
//! text typed into the app, a fixed green marker in the top-left corner) into the shared frame
//! slots at `--fps` frames a second, answers `Page.navigate` and `Runtime.evaluate` with
//! labelled echoes, and logs every input event it receives to stderr. With `--demo-popup` it
//! shows and hides a popup widget every 1.5 seconds, the way CEF reports an open `<select>`.
//!
//! It exits on `Shutdown` or when its stdin closes (the app is gone).

use anyhow::{Context as _, Result};
use spike_protocol::shm::SharedFrames;
use spike_protocol::usage;
use spike_protocol::view::{CursorKind, FrameKind, InputEvent, Rect};
use spike_protocol::wire::{self, FromHelper, ToHelper};
use std::io::{BufReader, BufWriter, Write};
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const GREEN: u32 = 0xFF00_FF00;

/// The fake text field and link area, in logical pixels.
const FIELD: Rect = Rect {
    x: 40,
    y: 60,
    width: 640,
    height: 48,
};
const LINK: Rect = Rect {
    x: 40,
    y: 130,
    width: 320,
    height: 32,
};
const POPUP: Rect = Rect {
    x: 40,
    y: 190,
    width: 220,
    height: 160,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("standin: error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

struct State {
    width: u32,
    height: u32,
    scale: f32,
    pointer: Option<(f32, f32)>,
    typed: String,
    composition: Option<String>,
    cursor: CursorKind,
    url: String,
    focused: bool,
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let shm = SharedFrames::open_inherited(&args).context("opening the frame slots")?;
    let fps: u32 = arg(&args, "--fps")
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let demo_popup = args.iter().any(|a| a == "--demo-popup");
    let mut free = vec![true; shm.slots() as usize];
    eprintln!(
        "standin: started, {} slots for up to {}x{} device pixels, {fps} fps",
        shm.slots(),
        shm.max_size().0,
        shm.max_size().1
    );

    let (inbox_tx, inbox) = mpsc::channel::<Option<ToHelper>>();
    std::thread::spawn(move || {
        let mut stdin = BufReader::new(std::io::stdin().lock());
        let mut buffer = Vec::new();
        loop {
            match wire::read_line::<ToHelper>(&mut stdin, &mut buffer) {
                Ok(Some(message)) => {
                    if inbox_tx.send(Some(message)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    eprintln!("standin: bad message from app: {error}");
                    break;
                }
            }
        }
        let _ = inbox_tx.send(None);
    });

    let mut out = BufWriter::new(std::io::stdout().lock());
    wire::write_line(
        &mut out,
        &FromHelper::Ready {
            engine: "stand-in test pattern (no Chromium)".into(),
        },
    )?;

    let mut state = State {
        width: 0,
        height: 0,
        scale: 1.0,
        pointer: None,
        typed: String::new(),
        composition: None,
        cursor: CursorKind::Arrow,
        url: "about:blank".into(),
        focused: false,
    };
    let started = Instant::now();
    let period = Duration::from_secs_f64(1.0 / fps.max(1) as f64);
    let mut deadline = Instant::now();
    let mut seq = 0u64;
    let mut popup_shown = false;
    let mut popup_toggled = Instant::now();

    let mut window_started = Instant::now();
    let mut window_usage = usage::current();
    let mut produced = 0u32;
    let mut skipped = 0u32;
    let mut moves = 0u32;

    'frames: loop {
        loop {
            let message = match inbox.try_recv() {
                Ok(Some(message)) => message,
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => break 'frames,
                Err(mpsc::TryRecvError::Empty) => break,
            };
            match message {
                ToHelper::Resize {
                    width,
                    height,
                    scale_factor,
                } => {
                    eprintln!("standin: resize {width}x{height} logical at scale {scale_factor}");
                    state.width = width;
                    state.height = height;
                    state.scale = scale_factor;
                }
                ToHelper::Release { slot } => {
                    if let Some(entry) = free.get_mut(slot as usize) {
                        *entry = true;
                    }
                }
                ToHelper::Input { event } => {
                    if matches!(event, InputEvent::PointerMove { .. }) {
                        moves += 1;
                    } else {
                        eprintln!("standin: input {event:?}");
                    }
                    apply_input(&mut state, event, &mut out)?;
                }
                ToHelper::DevTools { id, method, params } => {
                    eprintln!("standin: devtools {method} {params}");
                    let message = devtools_answer(&mut state, id, &method, &params);
                    wire::write_line(&mut out, &FromHelper::DevTools { message })?;
                }
                ToHelper::Shutdown => {
                    eprintln!("standin: shutdown requested");
                    break 'frames;
                }
            }
        }

        let (dw, dh) = device_size(&state);
        let (dw, dh) = (dw.min(shm.max_size().0), dh.min(shm.max_size().1));
        if dw > 0 && dh > 0 {
            match free.iter().position(|f| *f) {
                Some(slot) => {
                    // SAFETY: the slot is free, so the app is not reading it.
                    let pixels = unsafe { shm.slot_mut(slot as u32, dw, dh) }?;
                    paint(pixels, dw, dh, &state, started.elapsed(), seq);
                    free[slot] = false;
                    seq += 1;
                    produced += 1;
                    wire::write_line(
                        &mut out,
                        &FromHelper::Frame {
                            slot: slot as u32,
                            kind: FrameKind::View,
                            seq,
                            width: dw,
                            height: dh,
                            dirty: vec![Rect {
                                x: 0,
                                y: 0,
                                width: dw as i32,
                                height: dh as i32,
                            }],
                            produced_unix_ns: wire::unix_ns(),
                        },
                    )?;
                }
                None => skipped += 1,
            }

            if demo_popup && popup_toggled.elapsed() >= Duration::from_millis(1500) {
                popup_toggled = Instant::now();
                popup_shown = !popup_shown;
                wire::write_line(
                    &mut out,
                    &FromHelper::Popup {
                        show: popup_shown,
                        rect: popup_shown.then_some(POPUP),
                    },
                )?;
                if popup_shown {
                    let pw = (POPUP.width as f32 * state.scale) as u32;
                    let ph = (POPUP.height as f32 * state.scale) as u32;
                    if let Some(slot) = free.iter().position(|f| *f) {
                        // SAFETY: the slot is free, so the app is not reading it.
                        let pixels = unsafe { shm.slot_mut(slot as u32, pw, ph) }?;
                        paint_popup(pixels, pw, ph, state.scale);
                        free[slot] = false;
                        wire::write_line(
                            &mut out,
                            &FromHelper::Frame {
                                slot: slot as u32,
                                kind: FrameKind::Popup,
                                seq,
                                width: pw,
                                height: ph,
                                dirty: Vec::new(),
                                produced_unix_ns: wire::unix_ns(),
                            },
                        )?;
                    }
                }
            }
        }

        if window_started.elapsed() >= Duration::from_secs(1) {
            let now = usage::current();
            let elapsed = window_started.elapsed();
            eprintln!(
                "standin: stats frames={produced} skipped_no_free_slot={skipped} pointer_moves={moves} cpu={:.1}% resident={:.0}MB footprint={:.0}MB",
                now.cpu_percent_since(&window_usage, elapsed),
                now.resident_bytes as f64 / 1e6,
                now.footprint_bytes as f64 / 1e6,
            );
            window_started = Instant::now();
            window_usage = now;
            produced = 0;
            skipped = 0;
            moves = 0;
        }

        deadline += period;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        } else {
            deadline = now;
        }
    }
    out.flush()?;
    eprintln!("standin: exiting");
    Ok(())
}

fn device_size(state: &State) -> (u32, u32) {
    (
        (state.width as f32 * state.scale).round() as u32,
        (state.height as f32 * state.scale).round() as u32,
    )
}

fn apply_input(state: &mut State, event: InputEvent, out: &mut impl Write) -> Result<()> {
    match event {
        InputEvent::PointerMove { x, y, .. } | InputEvent::PointerDown { x, y, .. } => {
            state.pointer = Some((x, y));
            let cursor = if contains(FIELD, x, y) {
                CursorKind::IBeam
            } else if contains(LINK, x, y) {
                CursorKind::PointingHand
            } else {
                CursorKind::Arrow
            };
            if cursor != state.cursor {
                state.cursor = cursor;
                wire::write_line(out, &FromHelper::Cursor { cursor })?;
            }
        }
        InputEvent::PointerLeave { .. } => state.pointer = None,
        InputEvent::Text { text } => {
            state.composition = None;
            state.typed.push_str(&text);
        }
        InputEvent::KeyDown { key, .. } if key == "backspace" => {
            state.typed.pop();
        }
        InputEvent::ImeSetComposition { text, .. } => {
            // Report where each composed character sits so the candidate window can follow.
            let (glyph_w, _) = glyph_size(1.0);
            let start_x = FIELD.x + 8 + (state.typed.chars().count() as i32) * glyph_w;
            let characters = (0..text.chars().count() as i32)
                .map(|i| Rect {
                    x: start_x + i * glyph_w,
                    y: FIELD.y + 8,
                    width: glyph_w,
                    height: FIELD.height - 16,
                })
                .collect();
            let len = text.encode_utf16().count() as u32;
            wire::write_line(
                out,
                &FromHelper::ImeBounds {
                    range_start: 0,
                    range_end: len,
                    characters,
                },
            )?;
            state.composition = Some(text);
        }
        InputEvent::ImeFinish => {
            if let Some(text) = state.composition.take() {
                state.typed.push_str(&text);
            }
        }
        InputEvent::ImeCancel => state.composition = None,
        InputEvent::Focus { focused } => state.focused = focused,
        _ => {}
    }
    let keep = 40;
    let count = state.typed.chars().count();
    if count > keep {
        state.typed = state.typed.chars().skip(count - keep).collect();
    }
    Ok(())
}

fn devtools_answer(
    state: &mut State,
    id: u64,
    method: &str,
    params: &serde_json::Value,
) -> serde_json::Value {
    match method {
        "Page.navigate" => {
            state.url = params["url"].as_str().unwrap_or("").to_string();
            serde_json::json!({ "id": id, "result": { "frameId": "stand-in", "loaderId": "stand-in" } })
        }
        "Runtime.evaluate" => {
            let expression = params["expression"].as_str().unwrap_or("");
            serde_json::json!({
                "id": id,
                "result": { "result": { "type": "string", "value": format!("stand-in echo: {expression}") } }
            })
        }
        other => serde_json::json!({
            "id": id,
            "error": { "code": -32601, "message": format!("the stand-in does not implement {other}") }
        }),
    }
}

fn contains(r: Rect, x: f32, y: f32) -> bool {
    x >= r.x as f32 && y >= r.y as f32 && x < (r.x + r.width) as f32 && y < (r.y + r.height) as f32
}

/// BGRA bytes in memory are a little-endian 0xAARRGGBB.
fn as_u32(pixels: &mut [u8]) -> &mut [u32] {
    // SAFETY: slots are page aligned and their length is a multiple of four.
    let (head, body, tail) = unsafe { pixels.align_to_mut::<u32>() };
    assert!(head.is_empty() && tail.is_empty());
    body
}

fn paint(pixels: &mut [u8], w: u32, h: u32, state: &State, t: Duration, seq: u64) {
    let px = as_u32(pixels);
    let (w, h) = (w as usize, h as usize);
    let s = state.scale;
    let t = t.as_secs_f32();

    // Stripes scrolling up at 240 logical pixels a second, like a page being scrolled.
    let band = (40.0 * s) as usize;
    let offset = (t * 240.0 * s) as usize;
    for y in 0..h {
        let b = (y + offset) / band.max(1);
        let shade = if b.is_multiple_of(2) { 0xF4 } else { 0xE2 };
        let tint = ((b * 37) % 64) as u32;
        let color = 0xFF00_0000 | (shade << 16) | ((shade - tint / 2) << 8) | (shade - tint);
        px[y * w..(y + 1) * w].fill(color);
    }

    // A bouncing square.
    let side = (120.0 * s) as usize;
    if w > side && h > side {
        let span_x = (w - side) as f32;
        let span_y = (h - side) as f32;
        let x = triangle(t * 0.35) * span_x;
        let y = triangle(t * 0.27) * span_y;
        fill(px, w, h, x as usize, y as usize, side, side, 0xFF25_63EB);
    }

    // The fake link and text field, the typed text and any composition.
    let si = |v: i32| (v as f32 * s) as usize;
    fill(
        px,
        w,
        h,
        si(LINK.x),
        si(LINK.y),
        si(LINK.width),
        si(LINK.height),
        0xFF1D_4ED8,
    );
    text(
        px,
        w,
        h,
        si(LINK.x + 8),
        si(LINK.y + 6),
        "A LINK AREA",
        s * 3.0,
        0xFFFF_FFFF,
    );
    fill(
        px,
        w,
        h,
        si(FIELD.x),
        si(FIELD.y),
        si(FIELD.width),
        si(FIELD.height),
        0xFF33_3333,
    );
    fill(
        px,
        w,
        h,
        si(FIELD.x + 2),
        si(FIELD.y + 2),
        si(FIELD.width - 4),
        si(FIELD.height - 4),
        if state.focused {
            0xFFFF_FFFF
        } else {
            0xFFF0_F0F0
        },
    );
    let (glyph_w, _) = glyph_size(1.0);
    text(
        px,
        w,
        h,
        si(FIELD.x + 8),
        si(FIELD.y + 10),
        &state.typed,
        s * 4.0,
        0xFF11_1111,
    );
    if let Some(composition) = &state.composition {
        let x = FIELD.x + 8 + state.typed.chars().count() as i32 * glyph_w;
        text(
            px,
            w,
            h,
            si(x),
            si(FIELD.y + 10),
            composition,
            s * 4.0,
            0xFF9A_3412,
        );
        let underline_w = composition.chars().count() as i32 * glyph_w;
        fill(
            px,
            w,
            h,
            si(x),
            si(FIELD.y + 40),
            si(underline_w),
            si(2),
            0xFF9A_3412,
        );
    }

    // The frame counter and the current URL.
    let label = format!("FRAME {seq}");
    text(
        px,
        w,
        h,
        w.saturating_sub(si(260)),
        si(20),
        &label,
        s * 3.0,
        0xFF11_1111,
    );
    text(px, w, h, si(40), si(20), &state.url, s * 2.0, 0xFF55_5555);

    // The pointer as the helper last saw it.
    if let Some((x, y)) = state.pointer {
        let (x, y) = ((x * s) as usize, (y * s) as usize);
        let arm = (14.0 * s) as usize;
        let thick = (2.0 * s).max(1.0) as usize;
        fill(
            px,
            w,
            h,
            x.saturating_sub(arm),
            y,
            arm * 2,
            thick,
            0xFFDC_2626,
        );
        fill(
            px,
            w,
            h,
            x,
            y.saturating_sub(arm),
            thick,
            arm * 2,
            0xFFDC_2626,
        );
    }

    // A fixed marker the app's self-check looks for.
    fill(px, w, h, 0, 0, si(16), si(16), GREEN);
}

fn paint_popup(pixels: &mut [u8], w: u32, h: u32, s: f32) {
    let px = as_u32(pixels);
    let (w, h) = (w as usize, h as usize);
    px.fill(0xFFFF_FFFF);
    let row = (h / 5).max(1);
    for i in 0..5 {
        let color = if i == 1 { 0xFF25_63EB } else { 0xFFFF_FFFF };
        fill(px, w, h, 0, i * row, w, row, color);
        let ink = if i == 1 { 0xFFFF_FFFF } else { 0xFF11_1111 };
        let label = format!("OPTION {}", i + 1);
        text(
            px,
            w,
            h,
            (8.0 * s) as usize,
            i * row + (8.0 * s) as usize,
            &label,
            s * 2.5,
            ink,
        );
    }
}

fn triangle(v: f32) -> f32 {
    let f = v.fract();
    if f < 0.5 { f * 2.0 } else { 2.0 - f * 2.0 }
}

#[allow(clippy::too_many_arguments)]
fn fill(px: &mut [u32], w: usize, h: usize, x: usize, y: usize, fw: usize, fh: usize, color: u32) {
    let x1 = (x + fw).min(w);
    let y1 = (y + fh).min(h);
    if x >= x1 {
        return;
    }
    for row in y.min(h)..y1 {
        px[row * w + x..row * w + x1].fill(color);
    }
}

/// Logical width and height of one glyph cell at text scale 4 (the field's size).
fn glyph_size(scale: f32) -> (i32, i32) {
    ((6.0 * 4.0 * scale) as i32, (8.0 * 4.0 * scale) as i32)
}

#[allow(clippy::too_many_arguments)]
fn text(px: &mut [u32], w: usize, h: usize, x: usize, y: usize, s: &str, scale: f32, color: u32) {
    let dot = scale.max(1.0) as usize;
    for (i, c) in s.chars().enumerate() {
        let rows = glyph(c);
        let gx = x + i * 6 * dot;
        for (r, bits) in rows.iter().enumerate() {
            for col in 0..5 {
                if bits & (0x10 >> col) != 0 {
                    fill(px, w, h, gx + col * dot, y + r * dot, dot, dot, color);
                }
            }
        }
    }
}

/// A 5x7 bitmap font: digits, letters (lower case drawn as upper case), a few marks, and a
/// hollow box for anything else (CJK included, so composed text still shows up).
fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        ' ' => [0; 7],
        '.' => [0, 0, 0, 0, 0, 0x0C, 0x0C],
        '-' => [0, 0, 0, 0x1F, 0, 0, 0],
        ':' => [0, 0x0C, 0x0C, 0, 0x0C, 0x0C, 0],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        _ => [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F],
    }
}

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}
