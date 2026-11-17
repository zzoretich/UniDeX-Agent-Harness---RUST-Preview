//! The application shell: a native menu with shortcuts, key bindings, more than one window, window state saved and
//! restored without the window growing, `unidex://` links, and accessible buttons (decision 0010).
//!
//! Run it with `cargo run -p udx-cookbook --example app_shell`, or with a link as an argument:
//! `cargo run -p udx-cookbook --example app_shell -- unidex://thread/42`.
//!
//! - `cx.set_menus` builds the native menu bar on macOS only. On Linux and Windows GPUI stores the menus and shows
//!   nothing, so the application draws its own menu bar there (E11, E12). The shortcuts work everywhere: they are
//!   GPUI key bindings, dispatched through the focused element's key contexts.
//! - "Reopen from saved state" saves this window's origin from `window_bounds()` and its size from `viewport_size()`,
//!   closes it and opens a new one from the saved line. Saving the size from `window_bounds()` instead makes the window
//!   grow by its title bar at every restore on macOS (E14; Linux and Windows not measured). stderr shows both sizes.
//! - Links: `on_open_urls` is registered on the `Application` before `run`; macOS calls it, Linux and Windows never do,
//!   so the app also reads its arguments (E19, E20). A second launch handing a link to the running instance is the
//!   app's own work on Linux and Windows and is not shown here.
//! - Buttons: an id, the `Button` role, a label, a tab stop, and a visible label kept out of the accessibility tree so
//!   a screen reader does not read it twice. Visible text goes through `text!`: plain strings publish nothing.
//! - On macOS an application keeps running with no window by default; this one quits when its last window closes.
//!   GPUI's macOS delegate has no `applicationShouldTerminate:`, so Quit from the Dock or at log-out cannot be refused
//!   by a quit gate (E16).

use gpui::{
    Action, AnyWindowHandle, App, Bounds, Context, FocusHandle, Global, KeyBinding, Menu, MenuItem,
    Pixels, Role, SharedString, Subscription, Text, TitlebarOptions, Window, WindowBounds,
    WindowOptions, actions, div, point, prelude::*, px, rgb, size, text,
};
use udx_cookbook::{channel, links, window_state::SavedWindow};

actions!(app_shell, [Quit, NewWindow, Reopen, SayHello]);

/// The platform's main shortcut modifier. The trap: `KeyBinding::new("secondary-q", ..)` compiles and then panics,
/// because `KeyBinding::new` parses keystrokes without knowing the OS and only `KeyBinding::load(.., cx)` resolves
/// `secondary-`. Spell the modifier out per platform instead.
macro_rules! sec {
    ($key:literal) => {
        if cfg!(target_os = "macos") {
            concat!("cmd-", $key)
        } else {
            concat!("ctrl-", $key)
        }
    };
}

/// State every window shows.
#[derive(Default)]
struct Shell {
    hello: u32,
    links: Vec<String>,
    opened: usize,
}

impl Global for Shell {}

struct ShellWindow {
    number: usize,
    /// Focused when the window opens, so its key context ("ShellWindow") is active for key bindings.
    focus: FocusHandle,
    /// Re-renders when the window moves or is resized, so the bounds shown stay current.
    _bounds: Subscription,
}

fn button(id: &'static str, label: &'static str, action: impl Action) -> impl IntoElement {
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .focusable()
        .tab_stop(true)
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(0x2563eb))
        .hover(|s| s.bg(rgb(0x1d4ed8)))
        .focus_visible(|s| s.border_2().border_color(rgb(0xf59e0b)))
        .text_color(rgb(0xffffff))
        .cursor_pointer()
        .on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx))
        .child(Text::new_inaccessible(label.into()))
}

impl Render for ShellWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.global::<Shell>();
        let bounds = window.bounds();
        let content = window.viewport_size();
        let links = if shell.links.is_empty() {
            "none".to_string()
        } else {
            shell.links.join("  ")
        };
        div()
            .key_context("ShellWindow")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &Reopen, window, cx| reopen(window, cx)))
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(rgb(0xf3f4f6))
            .text_color(rgb(0x111827))
            .text_size(px(14.))
            .child(
                div()
                    .id("heading")
                    .role(Role::Heading)
                    .aria_level(1)
                    .text_size(px(18.))
                    .child(text!(format!("Window {}", self.number))),
            )
            .child(text!(format!("Open windows: {}", cx.windows().len())))
            .child(text!(format!(
                "Frame origin {:.0},{:.0}; content {:.0} x {:.0}",
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(content.width),
                f32::from(content.height),
            )))
            .child(text!(format!("Say Hello ran {} times", shell.hello)))
            .child(text!(format!("Links received: {links}")))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(button("new", "New window", NewWindow))
                    .child(button("reopen", "Reopen from saved state", Reopen))
                    .child(button("hello", "Say Hello", SayHello)),
            )
    }
}

fn open_shell_window(bounds: Bounds<Pixels>, cx: &mut App) -> Option<AnyWindowHandle> {
    let number = {
        let shell = cx.global_mut::<Shell>();
        shell.opened += 1;
        shell.opened
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(SharedString::from(format!(
                "UniDeX cookbook: shell {number}"
            ))),
            ..Default::default()
        }),
        ..Default::default()
    };
    let opened = cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| ShellWindow {
            number,
            focus: cx.focus_handle(),
            _bounds: cx.observe_window_bounds(window, |_, _, cx| cx.notify()),
        });
        window.focus(&view.read(cx).focus.clone(), cx);
        view
    });
    opened
        .map_err(|error| eprintln!("app_shell: could not open a window: {error:#}"))
        .ok()
        .map(Into::into)
}

/// Saves this window's state, closes it, and opens a new window from what was saved.
fn reopen(window: &mut Window, cx: &mut App) {
    let WindowBounds::Windowed(frame) = window.window_bounds() else {
        eprintln!("app_shell: maximized or full screen; nothing to show");
        return;
    };
    let content = window.viewport_size();
    let saved = SavedWindow::new(
        (frame.origin.x.into(), frame.origin.y.into()),
        (content.width.into(), content.height.into()),
    );
    let line = saved.encode();
    eprintln!(
        "app_shell: saved {line:?}; the frame from window_bounds() is {:.0} x {:.0}, which would grow the window",
        f32::from(frame.size.width),
        f32::from(frame.size.height)
    );
    // What a later start would read back from its settings file.
    let Some(restored) = SavedWindow::decode(&line) else {
        return;
    };
    let bounds = Bounds::new(
        point(px(restored.x), px(restored.y)),
        size(px(restored.width), px(restored.height)),
    );
    // Updating another window: use the untyped handle. A typed `WindowHandle::update` leases the root view, and
    // dispatching a keystroke inside it then panics, because GPUI renders that view during dispatch.
    if let Some(handle) = open_shell_window(bounds, cx) {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }
    window.remove_window();
}

fn main() {
    let links_at_start = links::links_in_args(std::env::args().skip(1));
    let (link_tx, link_rx) = channel::unbounded::<Vec<String>>();
    let app = gpui_platform::application();
    // Belongs to the Application, so it is registered before `run`. The callback gets no context: it forwards into a
    // channel that a foreground task awaits.
    app.on_open_urls(move |urls| {
        let _ = link_tx.send(urls);
    });
    app.run(move |cx: &mut App| {
        cx.set_global(Shell {
            links: links_at_start,
            ..Shell::default()
        });
        cx.bind_keys([
            KeyBinding::new(sec!("q"), Quit, None),
            KeyBinding::new(sec!("n"), NewWindow, None),
            KeyBinding::new(sec!("shift-r"), Reopen, Some("ShellWindow")),
            KeyBinding::new(sec!("shift-h"), SayHello, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &NewWindow, cx| {
            let bounds = Bounds::centered(None, size(px(480.), px(300.)), cx);
            open_shell_window(bounds, cx);
        });
        cx.on_action(|_: &SayHello, cx| {
            cx.global_mut::<Shell>().hello += 1;
            cx.refresh_windows();
        });
        cx.set_menus(vec![
            Menu::new("UniDeX cookbook").items([
                MenuItem::action("Say Hello", SayHello),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ]),
            Menu::new("File").items([MenuItem::action("New Window", NewWindow)]),
            Menu::new("Window").items([MenuItem::action("Reopen From Saved State", Reopen)]),
        ]);
        cx.spawn(async move |cx| {
            while let Some(urls) = link_rx.recv().await {
                cx.update(|cx| {
                    cx.global_mut::<Shell>().links.extend(urls);
                    cx.refresh_windows();
                });
            }
        })
        .detach();
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(480.), px(300.)), cx);
        open_shell_window(bounds, cx);
        cx.activate(true);
    });
}
