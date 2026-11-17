//! Reading the app's own window back, in-app.
//!
//! - macOS and Windows: `Window::render_to_image`, GPUI's own readback (feature `capture`, which turns on GPUI's
//!   `test-support`). It renders the current frame's scene again into an offscreen texture of the same renderer
//!   (Metal; Direct3D 11) and copies it to the CPU. It is what the renderer draws, not the pixels the compositor
//!   shows.
//! - Linux: GPUI's X11 and Wayland windows do not implement `render_to_image` (it returns "not implemented for this
//!   platform"). On X11 the app asks the X server for its own window's pixels instead (core protocol GetImage), which
//!   are the pixels the software rasteriser presented. Wayland has no equivalent for a client.

#[cfg(target_os = "linux")]
use anyhow::Context as _;
use anyhow::Result;
#[cfg(any(target_os = "linux", not(feature = "capture")))]
use anyhow::anyhow;
use gpui::Window;
use image::RgbaImage;

/// Returns the frame and a short description of how it was read.
pub fn capture_frame(window: &mut Window) -> Result<(RgbaImage, String)> {
    #[cfg(feature = "capture")]
    let framework = window.render_to_image();
    #[cfg(not(feature = "capture"))]
    let framework: Result<RgbaImage> = Err(anyhow!("built without the `capture` feature"));

    match framework {
        Ok(img) => Ok((img, "gpui Window::render_to_image".to_string())),
        Err(framework_error) => {
            #[cfg(target_os = "linux")]
            {
                let img = x11_own_window(window).with_context(|| {
                    format!("render_to_image failed ({framework_error:#}) and the X11 fallback failed too")
                })?;
                Ok((
                    img,
                    format!(
                        "x11 GetImage on the app's own window (render_to_image: {framework_error:#})"
                    ),
                ))
            }
            #[cfg(not(target_os = "linux"))]
            {
                Err(framework_error.context("render_to_image"))
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn x11_own_window(window: &Window) -> Result<RgbaImage> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, ImageOrder};

    let raw = HasWindowHandle::window_handle(window)
        .map_err(|e| anyhow!("no window handle: {e}"))?
        .as_raw();
    let xid: u32 = match raw {
        RawWindowHandle::Xlib(h) => h.window as u32,
        RawWindowHandle::Xcb(h) => h.window.get(),
        other => {
            return Err(anyhow!(
                "not an X11 window ({other:?}); no in-app readback on this display server"
            ));
        }
    };
    let (conn, _screen) = x11rb::connect(None).context("connecting to the X server")?;
    let geometry = conn.get_geometry(xid)?.reply().context("GetGeometry")?;
    let (w, h) = (geometry.width, geometry.height);
    let reply = conn
        .get_image(ImageFormat::Z_PIXMAP, xid, 0, 0, w, h, !0)?
        .reply()
        .context("GetImage")?;
    let setup = conn.setup();
    let format = setup
        .pixmap_formats
        .iter()
        .find(|f| f.depth == reply.depth)
        .ok_or_else(|| anyhow!("no pixmap format for depth {}", reply.depth))?;
    log::info!(
        "x11 GetImage: window 0x{xid:x}, {w}x{h}, depth {}, {} bits per pixel, byte order {:?}",
        reply.depth,
        format.bits_per_pixel,
        setup.image_byte_order
    );
    if format.bits_per_pixel != 32 || setup.image_byte_order != ImageOrder::LSB_FIRST {
        return Err(anyhow!(
            "unsupported pixel layout: {} bits per pixel, byte order {:?}",
            format.bits_per_pixel,
            setup.image_byte_order
        ));
    }
    let expected = w as usize * h as usize * 4;
    if reply.data.len() != expected {
        return Err(anyhow!(
            "GetImage returned {} bytes, expected {expected}",
            reply.data.len()
        ));
    }
    // 32 bits per pixel, least significant byte first: B, G, R, X.
    let mut rgba = Vec::with_capacity(expected);
    for px in reply.data.chunks_exact(4) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
    }
    RgbaImage::from_raw(w as u32, h as u32, rgba).ok_or_else(|| anyhow!("bad image size"))
}
