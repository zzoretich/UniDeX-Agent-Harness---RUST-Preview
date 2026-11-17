//! A window titled "UniDeX cookbook" with one label and one button that counts clicks.
//!
//! Run it with `cargo run -p udx-cookbook --example hello_window`.
//!
//! The smallest complete GPUI program on the pinned snapshot: start the application, open a window with a title,
//! render a view from its state, and change that state from a click handler. `cx.notify()` tells GPUI the view
//! changed, so it renders the window again.

use gpui::{
    App, Bounds, Context, Role, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size, text,
};

/// The window's one view. Its state is the number of clicks so far.
struct HelloWindow {
    clicks: u32,
}

impl Render for HelloWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .justify_center()
            .items_center()
            .bg(rgb(0xf5f5f5))
            .text_color(rgb(0x1f1f1f))
            .child(
                div()
                    .id("clicks")
                    .role(Role::Label)
                    .child(text!(format!("Clicks: {}", self.clicks))),
            )
            .child(
                div()
                    .id("count")
                    .role(Role::Button)
                    .aria_label("Count")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x2563eb))
                    .hover(|style| style.bg(rgb(0x1d4ed8)))
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.clicks += 1;
                        cx.notify();
                    }))
                    .child(text!("Count")),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        // On macOS an application keeps running when its last window closes; this example quits instead.
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(360.), px(200.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("UniDeX cookbook".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        match cx.open_window(options, |_window, cx| {
            cx.new(|_cx| HelloWindow { clicks: 0 })
        }) {
            Ok(_) => eprintln!("hello_window: window opened"),
            Err(error) => {
                eprintln!("hello_window: could not open the window: {error:#}");
                cx.quit();
            }
        }
        cx.activate(true);
    });
}
