//! Frames produced outside the UI thread, drawn into an element, with a GPUI card drawn above them: the browser
//! trial's transport and paint path, with a stand-in frame source in place of Chromium (decision 0012).
//!
//! Run it with `cargo run -p udx-cookbook --example frame_pane --release`. A moving pattern fills the pane; the card at
//! the top right stays above it. The pointer is a crosshair over the frames and an arrow over the card, and clicks on
//! the card do not reach the page: the card occludes the pane's hitbox.
//!
//! The path, as the trial built it:
//! - A producer (the browser helper, here a thread) paints into one of a few frame slots it owns, then names the slot
//!   in a one-line message on a pipe. The app's reader thread, not the UI thread, reads the pipe with a line limit,
//!   checks the slot and the frame size before touching any pixel, copies the pixels out and hands the slot back.
//!   Ownership of each slot passes by messages. In the trial the slots were an unnamed shared-memory region the
//!   helper inherited, and the pipes were its stdin and stdout; here they are a `Vec` and `std::io::pipe`.
//! - The newest frame waits in one slot (`LatestFrame`): a slow UI drops frames instead of queueing them, and the UI
//!   is woken once per frame it takes.
//! - The pane paints the frame with `paint_image` and drops the image it replaced from GPUI's sprite atlas: every
//!   frame is a new image, and the atlas would otherwise keep them all.
//! - The pane's hitbox is inserted in prepaint, like every hitbox, so the card, inserted later, sits above it.
//!
//! One step is the cookbook's own: GPUI's `RenderImage::new` takes frames of the `image` crate, which the cookbook does
//! not depend on, so the reader wraps each frame as an uncompressed PPM and lets GPUI decode it. That is an extra pass
//! over every pixel; the trial wrapped the BGRA buffer directly, and so should real code.

use std::cell::RefCell;
use std::io::{BufReader, PipeReader, PipeWriter, Write as _};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, Corners, CursorStyle, HitboxBehavior, Image, ImageFormat, RenderImage,
    Role, SvgRenderer, Text, TitlebarOptions, Window, WindowBounds, WindowOptions, canvas, div,
    prelude::*, px, rgb, rgba, size, text,
};
use udx_cookbook::channel;
use udx_cookbook::frames::{FrameMsg, LatestFrame, SlotLayout, bgra_to_ppm};
use udx_cookbook::wire::read_line_limited;

const LAYOUT: SlotLayout = SlotLayout {
    slots: 3,
    max_width: 1280,
    max_height: 800,
};
/// The longest message line the reader accepts; a frame message is about 30 bytes.
const MAX_LINE: usize = 256;
const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const CARD_TEXT: &str = "A GPUI card, drawn above the frames. Click it.";

type Slots = Arc<Vec<Mutex<Vec<u8>>>>;

/// The stand-in for the browser helper: waits for a slot the app has handed back, paints into it, names it on the
/// pipe. `pane` holds the pane's size in device pixels, which the UI stores in prepaint.
fn stand_in(mut pipe: PipeWriter, slots: Slots, free: mpsc::Receiver<u32>, pane: Arc<AtomicU64>) {
    let start = Instant::now();
    for seq in 0.. {
        std::thread::sleep(FRAME_INTERVAL);
        let Ok(slot) = free.recv() else { return };
        let packed = pane.load(Ordering::Relaxed);
        let width = ((packed >> 32) as u32).clamp(1, LAYOUT.max_width);
        let height = (packed as u32).clamp(1, LAYOUT.max_height);
        let elapsed = start.elapsed();
        paint(
            &mut slots[slot as usize].lock().unwrap(),
            width,
            height,
            elapsed,
        );
        let message = FrameMsg {
            slot,
            seq,
            width,
            height,
        }
        .to_line();
        if writeln!(pipe, "{message}").is_err() {
            return; // The reader is gone.
        }
    }
}

/// BGRA rows: a scrolling diagonal gradient and a white square that moves, so a stale or torn frame shows.
fn paint(pixels: &mut [u8], width: u32, height: u32, elapsed: Duration) {
    let (w, h) = (width as usize, height as usize);
    let t = (elapsed.as_millis() / 8) as usize;
    let wide: Vec<u8> = (0..w + 256)
        .flat_map(|i| {
            let v = ((i + t) % 256) as u8;
            [v, 0x40, 255 - v, 255]
        })
        .collect();
    for y in 0..h {
        let from = (y % 256) * 4;
        pixels[y * w * 4..(y + 1) * w * 4].copy_from_slice(&wide[from..from + w * 4]);
    }
    let side = 80.min(w).min(h);
    let x0 = (t * 2) % (w - side + 1);
    for y in 0..side {
        pixels[(y * w + x0) * 4..(y * w + x0 + side) * 4].fill(255);
    }
}

/// The app's reader thread. A message that breaks the protocol closes the channel: the peer is misbehaving.
fn reader(
    pipe: PipeReader,
    slots: Slots,
    free: mpsc::Sender<u32>,
    latest: Arc<LatestFrame<Arc<RenderImage>>>,
    wake: channel::Sender<()>,
) {
    let mut pipe = BufReader::new(pipe);
    // Needed only to decode the cookbook's PPM stand-in; nothing here is an SVG.
    let decoder = SvgRenderer::new(Arc::new(()));
    loop {
        let checked = match read_line_limited(&mut pipe, MAX_LINE) {
            Ok(None) => return,
            Ok(Some(line)) => FrameMsg::parse(&line)
                .ok_or_else(|| format!("not a frame message: {line:?}"))
                .and_then(|msg| match LAYOUT.check(msg.slot, msg.width, msg.height) {
                    Ok(len) => Ok((msg, len)),
                    Err(error) => Err(error.to_string()),
                }),
            Err(error) => Err(error.to_string()),
        };
        let (msg, len) = match checked {
            Ok(frame) => frame,
            Err(why) => {
                eprintln!("frame_pane: closing the channel: {why}");
                return;
            }
        };
        let bgra = slots[msg.slot as usize].lock().unwrap()[..len].to_vec();
        let _ = free.send(msg.slot);
        // The size was checked, so the conversion cannot fail; a decoding error would be a bug, and is reported.
        let Some(ppm) = bgra_to_ppm(&bgra, msg.width, msg.height) else {
            continue;
        };
        let image = match Image::from_bytes(ImageFormat::Pnm, ppm).to_image_data(decoder.clone()) {
            Ok(image) => image,
            Err(error) => {
                eprintln!(
                    "frame_pane: GPUI could not decode frame {}: {error:#}",
                    msg.seq
                );
                continue;
            }
        };
        if latest.put(image) && wake.send(()).is_err() {
            return; // The window is gone.
        }
    }
}

struct Pane {
    latest: Arc<LatestFrame<Arc<RenderImage>>>,
    frame: Option<Arc<RenderImage>>,
    /// The image painted last, dropped from the atlas when a newer one replaces it.
    painted: Rc<RefCell<Option<Arc<RenderImage>>>>,
    pane_size: Arc<AtomicU64>,
    taken: u64,
    page_clicks: u32,
    card_clicks: u32,
}

impl Pane {
    fn new(reader_end: PipeReader, writer_end: PipeWriter, cx: &mut Context<Self>) -> Self {
        let slots: Slots = Arc::new(
            (0..LAYOUT.slots)
                .map(|_| Mutex::new(vec![0; LAYOUT.slot_bytes()]))
                .collect(),
        );
        let (free_tx, free_rx) = mpsc::channel();
        for slot in 0..LAYOUT.slots {
            let _ = free_tx.send(slot);
        }
        let latest = Arc::new(LatestFrame::new());
        let pane_size = Arc::new(AtomicU64::new((640 << 32) | 400));
        let (wake_tx, wake_rx) = channel::unbounded();
        let (s, p) = (slots.clone(), pane_size.clone());
        std::thread::spawn(move || stand_in(writer_end, s, free_rx, p));
        let l = latest.clone();
        std::thread::spawn(move || reader(reader_end, slots, free_tx, l, wake_tx));
        cx.spawn(async move |this, cx| {
            while wake_rx.recv().await.is_some() {
                let alive = this.update(cx, |pane, cx| {
                    if let Some(frame) = pane.latest.take() {
                        pane.frame = Some(frame);
                        pane.taken += 1;
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    return;
                }
            }
        })
        .detach();
        Pane {
            latest,
            frame: None,
            painted: Rc::default(),
            pane_size,
            taken: 0,
            page_clicks: 0,
            card_clicks: 0,
        }
    }
}

impl Render for Pane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (frame, painted, pane_size) = (
            self.frame.clone(),
            self.painted.clone(),
            self.pane_size.clone(),
        );
        let page = canvas(
            move |bounds, window, _| {
                let scale = window.scale_factor();
                let w = (f32::from(bounds.size.width) * scale).round() as u64;
                let h = (f32::from(bounds.size.height) * scale).round() as u64;
                pane_size.store((w << 32) | h, Ordering::Relaxed);
                window.insert_hitbox(bounds, HitboxBehavior::Normal)
            },
            move |bounds, hitbox, window, _| {
                window.set_cursor_style(CursorStyle::Crosshair, &hitbox);
                let Some(image) = frame else { return };
                let (device, scale) = (image.size(0), window.scale_factor());
                let logical = size(
                    px(device.width.0 as f32 / scale),
                    px(device.height.0 as f32 / scale),
                );
                let rect = Bounds::new(bounds.origin, logical);
                let _ = window.paint_image(rect, rect, Corners::default(), image.clone(), 0, false);
                let mut painted = painted.borrow_mut();
                if painted.as_ref().is_none_or(|old| old.id != image.id)
                    && let Some(old) = painted.replace(image)
                {
                    let _ = window.drop_image(old);
                }
            },
        );
        let status = format!(
            "Frames taken {}, replaced before the UI took them {}. Page clicks {}, card clicks {}.",
            self.taken,
            self.latest.superseded(),
            self.page_clicks,
            self.card_clicks
        );
        div()
            .size_full()
            .relative()
            .bg(rgb(0x101010))
            .text_color(rgb(0xf9fafb))
            .text_size(px(14.))
            .child(
                div()
                    .id("page")
                    .size_full()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page_clicks += 1;
                        cx.notify();
                    }))
                    .child(page.size_full()),
            )
            .child(
                div()
                    .id("card")
                    .role(Role::Button)
                    .aria_label(CARD_TEXT)
                    .absolute()
                    .top(px(16.))
                    .right(px(16.))
                    .w(px(260.))
                    .p_3()
                    .rounded_md()
                    .bg(rgba(0x111827e6))
                    .border_1()
                    .border_color(rgb(0xf59e0b))
                    // A card over the page takes the pointer itself; without this, clicks also reach the page.
                    .occlude()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.card_clicks += 1;
                        cx.notify();
                    }))
                    // The label is on the card; the visible text is kept out of the tree so it is not read twice.
                    .child(Text::new_inaccessible(CARD_TEXT.into())),
            )
            .child(
                div()
                    .absolute()
                    .bottom(px(12.))
                    .left(px(12.))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgba(0x000000b0))
                    .child(text!(status)),
            )
    }
}

fn main() {
    let (reader_end, writer_end) = match std::io::pipe() {
        Ok(ends) => ends,
        Err(error) => {
            eprintln!("frame_pane: could not create a pipe: {error}");
            return;
        }
    };
    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(720.), px(450.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("UniDeX cookbook: frame pane".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let opened = cx.open_window(options, |_, cx| {
            cx.new(|cx| Pane::new(reader_end, writer_end, cx))
        });
        if let Err(error) = opened {
            eprintln!("frame_pane: could not open the window: {error:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}
