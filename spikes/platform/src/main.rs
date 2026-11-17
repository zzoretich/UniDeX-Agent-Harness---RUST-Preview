//! Task 11 trial: platform integration on the pinned GPUI snapshot. Throwaway code.
//!
//! Modes:
//! - no flag: the trial app for the maintainer's checklist (CHECKLIST.md). One window with a single-line and a
//!   multi-line text field, a labelled button, and buttons for the dialogs, a second window and the popover; an
//!   application menu with shortcuts; a tray icon whose click opens a popover window under it. Links given as
//!   arguments (`unidex://...`) or delivered by the OS are shown in the window.
//! - `--selfcheck [--out FILE]`: opens the same windows and tray icon for a few seconds, drives every check that a
//!   program can drive on itself, prints one line per check, removes the tray icon, closes everything and exits
//!   (status 0 when every check passed). It never opens a file dialog.
//! - `--headless`: the checks that need no window (link metadata, argument parsing); CI runs it everywhere.
//! - `--write-metadata DIR`: writes the `unidex://` bundle metadata (Info.plist, desktop entry, registry file).

/// The platform's main shortcut modifier: cmd on macOS, ctrl elsewhere. `KeyBinding::new` parses keystrokes without
/// an OS, so GPUI's `secondary-` prefix panics there (it is resolved only by `KeyBinding::load`).
#[macro_export]
macro_rules! sec {
    ($key:literal) => {
        if cfg!(target_os = "macos") {
            concat!("cmd-", $key)
        } else {
            concat!("ctrl-", $key)
        }
    };
}

mod link_scheme;
#[cfg(target_os = "macos")]
mod mac;
mod selfcheck;
mod text_field;
mod tray;

use std::time::Duration;

use gpui::{
    App, AppContext, Bounds, Context, DisplayId, Entity, FocusHandle, Global, KeyBinding, Menu,
    MenuItem, PathPromptOptions, Pixels, Role, SharedString, TitlebarOptions, Window, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, actions, div, point, prelude::*, px, rgb, size, text,
};

use text_field::TextField;

actions!(
    trial,
    [
        Quit,
        OpenFile,
        SaveFile,
        NewWindow,
        ShowPopover,
        SayHello,
        FocusNext,
        FocusPrev,
        ClosePopover
    ]
);

/// Shared state of the trial app.
pub struct TrialState {
    pub main: Option<WindowHandle<TrialView>>,
    pub second: Option<WindowHandle<SecondView>>,
    pub popover: Option<WindowHandle<PopoverView>>,
    pub tray: Option<tray::Tray>,
    pub hello_count: u32,
    pub links: Vec<String>,
    pub tray_log: Vec<String>,
    pub last_tray_rect: Option<(f64, f64, f64, f64)>,
}

impl Global for TrialState {}

pub struct TrialView {
    pub name: Entity<TextField>,
    pub notes: Entity<TextField>,
    pub clicks: u32,
    pub status: SharedString,
    focus: FocusHandle,
}

impl TrialView {
    fn set_status(&mut self, status: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.status = status.into();
        cx.notify();
    }
}

/// A button: an element with an id, the `Button` role, a label, a focus stop and a click handler.
fn button(
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
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
        .on_click(move |_, window, cx| on_click(window, cx))
        // The label is on the button node; the visible text is kept out of the tree so it is not read twice.
        .child(gpui::Text::new_inaccessible(label.into()))
}

impl Render for TrialView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = cx.global::<TrialState>();
        let links = if state.links.is_empty() {
            "none yet".to_string()
        } else {
            state.links.join("  ")
        };
        let tray = state
            .tray_log
            .last()
            .cloned()
            .unwrap_or_else(|| "none yet".into());
        let name_log = self
            .name
            .read(cx)
            .input_log
            .last()
            .cloned()
            .unwrap_or_default();
        let notes_log = self
            .notes
            .read(cx)
            .input_log
            .last()
            .cloned()
            .unwrap_or_default();
        let layout = cx.keyboard_layout().name().to_string();
        div()
            .id("trial-root")
            .key_context("TrialView")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &FocusNext, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrev, window, cx| window.focus_prev(cx)))
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
                    .child(text!("UniDeX platform trial")),
            )
            .child(text!("Name (single line)"))
            .child(self.name.clone())
            .child(text!(format!("  last input call: {name_log}")))
            .child(text!("Notes (multi-line, Enter makes a new line)"))
            .child(self.notes.clone())
            .child(text!(format!("  last input call: {notes_log}")))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .child({
                        let view = cx.entity().downgrade();
                        button("count", "Count", move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.clicks += 1;
                                let clicks = this.clicks;
                                this.set_status(format!("Count pressed {clicks} times"), cx);
                            })
                            .ok();
                        })
                    })
                    .child(button("open", "Open file", |window, cx| {
                        window.dispatch_action(Box::new(OpenFile), cx)
                    }))
                    .child(button("save", "Save file", |window, cx| {
                        window.dispatch_action(Box::new(SaveFile), cx)
                    }))
                    .child(button("second", "Second window", |window, cx| {
                        window.dispatch_action(Box::new(NewWindow), cx)
                    }))
                    .child(button("popover", "Popover", |window, cx| {
                        window.dispatch_action(Box::new(ShowPopover), cx)
                    })),
            )
            .child(
                div()
                    .id("status")
                    .role(Role::Status)
                    .child(text!(format!("Status: {}", self.status))),
            )
            .child(text!(format!("Links received: {links}")))
            // A plain string child: GPUI publishes no accessibility node for `&str`, `String`, `SharedString` or
            // `StyledText`, only for `Text` (the `text!` macro) and `InteractiveText`. The self-check confirms it.
            .child("Plain string child (not published)")
            .child(text!(format!("Last tray event: {tray}")))
            .child(text!(format!(
                "Keyboard layout: {layout}   Hello count: {}",
                state.hello_count
            )))
    }
}

pub struct SecondView {
    focus: FocusHandle,
    /// Redraws the window when it moves or is resized, so its bounds and display stay current.
    _bounds: gpui::Subscription,
}

impl Render for SecondView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bounds = window.bounds();
        let display = window
            .display(cx)
            .map(|d| format!("{:?}", d.id()))
            .unwrap_or_else(|| "unknown".into());
        let count = cx.windows().len();
        div()
            .id("second-root")
            .track_focus(&self.focus)
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(0xecfdf5))
            .text_color(rgb(0x111827))
            .child(text!("Second window"))
            .child(text!(format!("Open windows: {count}")))
            .child(text!(format!(
                "Bounds: origin {:.0},{:.0} size {:.0}x{:.0}",
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height)
            )))
            .child(text!(format!("Display: {display}")))
            .child(button("close-second", "Close this window", |window, _| {
                window.remove_window()
            }))
    }
}

pub struct PopoverView {
    focus: FocusHandle,
}

impl Render for PopoverView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hello = cx.global::<TrialState>().hello_count;
        div()
            .id("popover-root")
            .key_context("Popover")
            .track_focus(&self.focus)
            .on_action(|_: &ClosePopover, window, _| window.remove_window())
            .size_full()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(0xfffbeb))
            .border_1()
            .border_color(rgb(0xd97706))
            .text_color(rgb(0x111827))
            .child(text!("Tray popover"))
            .child(text!(format!("Hello count: {hello}")))
            .child(text!("Escape closes it"))
            // A button with no aria label: only its visible text (a `text!` child) can name it.
            .child(
                div()
                    .id("unlabelled-button")
                    .role(Role::Button)
                    .focusable()
                    .child(text!("Unlabelled button")),
            )
            .child(button("close-popover", "Close", |window, _| {
                window.remove_window()
            }))
    }
}

/// Bounds for the popover: under the tray icon if its rectangle is known, else near the primary display's top right.
/// `scale` turns tray-icon's physical pixels into GPUI's points.
pub fn popover_bounds(
    rect: Option<(f64, f64, f64, f64)>,
    scale: f64,
    cx: &App,
) -> (Bounds<Pixels>, Option<DisplayId>) {
    let popover_size = size(px(260.), px(150.));
    let display = cx.primary_display();
    let display_bounds = display.as_ref().map(|d| d.visible_bounds());
    match rect {
        Some((x, y, w, h)) => {
            // tray-icon reports the icon in global coordinates (from the main display's top left), while GPUI places
            // a window relative to one display, and GPUI's display bounds carry no global position (on macOS every
            // display's origin is 0,0). So the display holding the icon is found through AppKit.
            let center_x = (x + w / 2.0) / scale;
            let below = (y + h) / scale;
            #[cfg(target_os = "macos")]
            if let Some((id, rel_x, rel_y)) = mac::locate_on_display(center_x, below) {
                let origin = point(
                    px(rel_x as f32) - popover_size.width / 2.0,
                    px(rel_y as f32),
                );
                return (
                    Bounds::new(origin, popover_size),
                    Some(DisplayId::new(id as u64)),
                );
            }
            let origin = point(
                px(center_x as f32) - popover_size.width / 2.0,
                px(below as f32),
            );
            (Bounds::new(origin, popover_size), display.map(|d| d.id()))
        }
        None => {
            let b = display_bounds.unwrap_or(Bounds::new(
                point(px(0.), px(0.)),
                size(px(1280.), px(800.)),
            ));
            let origin = point(b.right() - popover_size.width - px(16.), b.top() + px(8.));
            (Bounds::new(origin, popover_size), display.map(|d| d.id()))
        }
    }
}

pub fn open_popover(bounds: Bounds<Pixels>, display_id: Option<DisplayId>, cx: &mut App) {
    if let Some(old) = cx.global_mut::<TrialState>().popover.take() {
        old.update(cx, |_, window, _| window.remove_window()).ok();
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: true,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id,
        ..Default::default()
    };
    match cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| PopoverView {
            focus: cx.focus_handle(),
        });
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
        view
    }) {
        Ok(handle) => cx.global_mut::<TrialState>().popover = Some(handle),
        Err(e) => eprintln!("popover: {e:#}"),
    }
}

pub fn open_second_window(
    display_id: Option<DisplayId>,
    bounds: Option<WindowBounds>,
    cx: &mut App,
) -> Option<WindowHandle<SecondView>> {
    let options = WindowOptions {
        window_bounds: Some(bounds.unwrap_or(WindowBounds::Windowed(Bounds::new(
            point(px(120.), px(120.)),
            size(px(360.), px(220.)),
        )))),
        titlebar: Some(TitlebarOptions {
            title: Some("Second window".into()),
            ..Default::default()
        }),
        display_id,
        ..Default::default()
    };
    let handle = cx
        .open_window(options, |window, cx| {
            cx.new(|cx| SecondView {
                focus: cx.focus_handle(),
                _bounds: cx.observe_window_bounds(window, |_, _, cx| cx.notify()),
            })
        })
        .map_err(|e| eprintln!("second window: {e:#}"))
        .ok()?;
    cx.global_mut::<TrialState>().second = Some(handle);
    Some(handle)
}

fn set_status(status: String, cx: &mut App) {
    if let Some(main) = cx.global::<TrialState>().main {
        main.update(cx, |view, _, cx| view.set_status(status, cx))
            .ok();
    }
}

/// Remove the tray icon and quit. The icon must go before the process does.
pub fn quit(cx: &mut App) {
    cx.global_mut::<TrialState>().tray.take();
    cx.quit();
}

pub fn tray_scale(cx: &mut App) -> f64 {
    #[cfg(target_os = "macos")]
    if let Some(item) = cx
        .global::<TrialState>()
        .tray
        .as_ref()
        .and_then(|t| t.local())
        .and_then(|t| t.ns_status_item())
    {
        return mac::status_item_scale(&item).unwrap_or(1.0);
    }
    // Windows and Linux: approximated by the main window's scale factor.
    let Some(main) = cx.global::<TrialState>().main else {
        return 1.0;
    };
    cx.update_window(main.into(), |_, w, _| w.scale_factor() as f64)
        .unwrap_or(1.0)
}

fn menus() -> Vec<Menu> {
    vec![
        Menu::new("UniDeX Trial").items([
            MenuItem::action("Say Hello", SayHello),
            MenuItem::separator(),
            MenuItem::action("Quit UniDeX Trial", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Open...", OpenFile),
            MenuItem::action("Save...", SaveFile),
            MenuItem::action("New Window", NewWindow),
        ]),
        Menu::new("Trial").items([
            MenuItem::action("Show Popover", ShowPopover),
            MenuItem::action("Say Hello", SayHello),
        ]),
    ]
}

fn setup(
    cx: &mut App,
    selfcheck: Option<selfcheck::Options>,
    links: Vec<String>,
    link_rx: async_channel::Receiver<Vec<String>>,
) {
    cx.set_global(TrialState {
        main: None,
        second: None,
        popover: None,
        tray: None,
        hello_count: 0,
        links,
        tray_log: Vec::new(),
        last_tray_rect: None,
    });

    text_field::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new(sec!("q"), Quit, None),
        KeyBinding::new(sec!("o"), OpenFile, None),
        KeyBinding::new(sec!("s"), SaveFile, None),
        KeyBinding::new(sec!("n"), NewWindow, None),
        KeyBinding::new(sec!("shift-p"), ShowPopover, None),
        KeyBinding::new(sec!("shift-h"), SayHello, None),
        KeyBinding::new("tab", FocusNext, Some("TrialView")),
        KeyBinding::new("shift-tab", FocusPrev, Some("TrialView")),
        KeyBinding::new("escape", ClosePopover, Some("Popover")),
    ]);

    cx.on_action(|_: &Quit, cx| quit(cx));
    cx.on_action(|_: &SayHello, cx| {
        let n = {
            let state = cx.global_mut::<TrialState>();
            state.hello_count += 1;
            state.hello_count
        };
        set_status(format!("Say Hello ran ({n})"), cx);
        cx.refresh_windows();
    });
    cx.on_action(|_: &NewWindow, cx| {
        open_second_window(None, None, cx);
    });
    cx.on_action(|_: &ShowPopover, cx| {
        let rect = cx.global::<TrialState>().last_tray_rect.or_else(|| {
            cx.global::<TrialState>()
                .tray
                .as_ref()
                .and_then(|t| t.rect())
        });
        let scale = tray_scale(cx);
        let (bounds, display) = popover_bounds(rect, scale, cx);
        open_popover(bounds, display, cx);
    });
    // The dialogs are never opened by the self-check: a modal panel would wait for a person.
    cx.on_action(|_: &OpenFile, cx| {
        let answer = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        cx.spawn(async move |cx| {
            let status = match answer.await {
                Ok(Ok(Some(paths))) => format!("Open dialog returned {paths:?}"),
                Ok(Ok(None)) => "Open dialog cancelled".into(),
                Ok(Err(e)) => format!("Open dialog failed: {e:#}"),
                Err(_) => "Open dialog dropped".into(),
            };
            cx.update(|cx| set_status(status, cx));
        })
        .detach();
    });
    cx.on_action(|_: &SaveFile, cx| {
        let directory = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
        let answer = cx.prompt_for_new_path(&directory, Some("trial-note.txt"));
        cx.spawn(async move |cx| {
            // The trial only shows the chosen path; it writes no file.
            let status = match answer.await {
                Ok(Ok(Some(path))) => format!("Save dialog returned {path:?} (nothing written)"),
                Ok(Ok(None)) => "Save dialog cancelled".into(),
                Ok(Err(e)) => format!("Save dialog failed: {e:#}"),
                Err(_) => "Save dialog dropped".into(),
            };
            cx.update(|cx| set_status(status, cx));
        })
        .detach();
    });

    cx.set_menus(menus());

    // Links the OS hands over while running (macOS: application:openURLs:). GPUI's callback gets no context, so it
    // forwards into a channel this foreground task awaits.
    // The callback itself is registered on the Application before `run` (see main).
    cx.spawn(async move |cx| {
        while let Ok(urls) = link_rx.recv().await {
            cx.update(|cx| {
                cx.global_mut::<TrialState>().links.extend(urls.clone());
                set_status(format!("Link received: {urls:?}"), cx);
                cx.refresh_windows();
            });
        }
    })
    .detach();

    // Quit when the last window closes, on every platform (on macOS GPUI's default keeps the app running).
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            quit(cx);
        }
    })
    .detach();

    let main_bounds = Bounds::centered(None, size(px(560.), px(640.)), cx);
    let main = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(main_bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("UniDeX platform trial".into()),
                ..Default::default()
            }),
            app_id: Some(link_scheme::EXECUTABLE.into()),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| TrialView {
                name: cx.new(|cx| TextField::new("name-field", "Name", false, cx)),
                notes: cx.new(|cx| TextField::new("notes-field", "Notes", true, cx)),
                clicks: 0,
                status: "ready".into(),
                focus: cx.focus_handle(),
            });
            let name_focus = view.read(cx).name.read(cx).focus.clone();
            window.focus(&name_focus, cx);
            view
        },
    );
    let main = match main {
        Ok(main) => main,
        Err(e) => {
            eprintln!("could not open the main window: {e:#}");
            std::process::exit(2);
        }
    };
    cx.global_mut::<TrialState>().main = Some(main);
    cx.activate(true);

    // The tray icon, and its events forwarded into GPUI.
    let (tray_tx, tray_rx) = async_channel::unbounded::<tray::TrayMessage>();
    tray::forward_events(tray_tx);
    match tray::Tray::create() {
        Ok(t) => cx.global_mut::<TrialState>().tray = Some(t),
        Err(e) => eprintln!("tray icon: {e}"),
    }
    let interactive = selfcheck.is_none();
    cx.spawn(async move |cx| {
        while let Ok(message) = tray_rx.recv().await {
            cx.update(|cx| {
                cx.global_mut::<TrialState>()
                    .tray_log
                    .push(format!("{message:?}"));
                match &message {
                    tray::TrayMessage::Click {
                        button: "left",
                        up: false,
                        rect,
                    } => {
                        if let Some(rect) = rect {
                            cx.global_mut::<TrialState>().last_tray_rect = Some(*rect);
                        }
                        // In the self-check the popover is opened by the check itself, to compare its position.
                        if interactive {
                            cx.dispatch_action(&ShowPopover);
                        }
                    }
                    tray::TrayMessage::Menu(id) if id == tray::MENU_SHOW_POPOVER => {
                        cx.dispatch_action(&ShowPopover)
                    }
                    tray::TrayMessage::Menu(id) if id == tray::MENU_QUIT => quit(cx),
                    _ => {}
                }
                cx.refresh_windows();
            });
        }
    })
    .detach();

    if let Some(options) = selfcheck {
        selfcheck::start(options, cx);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--write-metadata") {
        let dir = args.get(i + 1).map(String::as_str).unwrap_or("bundle");
        link_scheme::write_all(std::path::Path::new(dir)).expect("write metadata");
        println!(
            "wrote {dir}/{{{}, {}, {}}}",
            link_scheme::INFO_PLIST_FILE,
            link_scheme::DESKTOP_FILE,
            link_scheme::REG_FILE
        );
        return;
    }
    if args.iter().any(|a| a == "--headless") {
        let mut failed = 0;
        for (name, ok, detail) in link_scheme::headless_checks() {
            println!(
                "{} [6 link] {name}: {detail}",
                if ok { "PASS" } else { "FAIL" }
            );
            failed += usize::from(!ok);
        }
        std::process::exit(i32::from(failed > 0));
    }
    let selfcheck = args
        .iter()
        .any(|a| a == "--selfcheck")
        .then(|| selfcheck::Options {
            out: args
                .iter()
                .position(|a| a == "--out")
                .and_then(|i| args.get(i + 1).cloned()),
        });
    if selfcheck.is_some() {
        // A watchdog, so a hung check can never leave the app (or its tray icon) behind.
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(90));
            eprintln!("selfcheck: watchdog fired after 90 s; exiting");
            std::process::exit(3);
        });
    }
    let links = link_scheme::links_in_args(args.iter().cloned());
    // `on_open_urls` belongs to the Application, so it is registered before the event loop starts.
    let (link_tx, link_rx) = async_channel::unbounded::<Vec<String>>();
    let app = gpui_platform::application();
    app.on_open_urls(move |urls| {
        let _ = link_tx.try_send(urls);
    });
    app.run(move |cx: &mut App| setup(cx, selfcheck, links, link_rx));
}
