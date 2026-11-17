//! The fixed scene the rendering trial drew so a captured frame could be checked automatically (decision 0013).
//!
//! Run it with `cargo run -p udx-cookbook --example frame_check_scene`. The window shows red, green and blue blocks, a
//! state block that turns from grey to orange when "Count" is clicked, a line of text and a click counter, all at the
//! fixed logical positions in `udx_cookbook::frame_check`. `frame_check::check_frame` checks a capture of this window:
//! it finds the red block, derives the origin and the scale from it, then samples the other blocks, the background,
//! the button and the ink of the text line. Its unit tests show what it catches and what it does not.
//!
//! How the trial captured frames, none of which the cookbook can do without adding a dependency or a GPUI feature:
//! `Window::render_to_image` on macOS and Windows, which needs GPUI's `test-support` feature (so release builds must
//! not have it); the app asking the X server for its own window's pixels on Linux X11; and captures from outside the
//! app (`xwd` or `grim` on Linux, a capture of the window's client area on Windows). On the hosted runners, which have
//! no GPU, the window rendered through software rasterisers and passed the check on X11 and on Windows. Decision 0013
//! keeps this scene-plus-check approach for the real test harness (design 10.3), which captures through its
//! automation socket.
//!
//! Elements a test must find carry an id and a role; the colour blocks have neither, so GPUI's accessibility tree
//! leaves them out, and the trial recorded their bounds itself.

use gpui::{
    App, Bounds, Context, Role, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size, text,
};
use udx_cookbook::frame_check as scene;

struct Scene {
    clicks: u32,
}

fn block(at: (f32, f32), colour: u32) -> impl IntoElement {
    div()
        .absolute()
        .left(px(at.0))
        .top(px(at.1))
        .size(px(scene::BLOCK_SIDE))
        .bg(rgb(colour))
}

impl Render for Scene {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = if self.clicks > 0 {
            scene::STATE_CLICKED
        } else {
            scene::STATE_IDLE
        };
        let (left, top, width, height) = scene::BUTTON_RECT;
        div()
            .size_full()
            .relative()
            .bg(rgb(scene::BACKGROUND))
            .text_color(rgb(scene::TEXT))
            .child(block(scene::RED_AT, scene::RED))
            .child(block(scene::GREEN_AT, scene::GREEN))
            .child(block(scene::BLUE_AT, scene::BLUE))
            .child(block(scene::STATE_AT, state))
            .child(
                div()
                    .id("line")
                    .role(Role::Label)
                    .absolute()
                    .left(px(scene::TEXT_AT.0))
                    .top(px(scene::TEXT_AT.1))
                    .text_size(px(scene::TEXT_SIZE))
                    .child(text!(scene::TEXT_LINE)),
            )
            .child(
                div()
                    .id("count")
                    .role(Role::Button)
                    .aria_label("Count")
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(width))
                    .h(px(height))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(scene::BUTTON))
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.clicks += 1;
                        cx.notify();
                    }))
                    .child(gpui::Text::new_inaccessible("Count".into())),
            )
            .child(
                div()
                    .id("clicks")
                    .role(Role::Label)
                    .absolute()
                    .left(px(scene::LABEL_AT.0))
                    .top(px(scene::LABEL_AT.1))
                    .text_size(px(18.))
                    .child(text!(format!("Clicks: {}", self.clicks))),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let content = size(px(scene::WINDOW_W), px(scene::WINDOW_H));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, content, cx))),
            titlebar: Some(TitlebarOptions {
                title: Some("UniDeX cookbook: frame check scene".into()),
                ..Default::default()
            }),
            is_resizable: false,
            ..Default::default()
        };
        if let Err(error) = cx.open_window(options, |_, cx| cx.new(|_| Scene { clicks: 0 })) {
            eprintln!("frame_check_scene: could not open the window: {error:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}
