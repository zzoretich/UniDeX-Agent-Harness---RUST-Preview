//! `--selfcheck`: everything a program can verify on itself, run for a few seconds and then cleaned up.
//!
//! Each check says how it was verified, in the trial's terms:
//! - `a/os`: the app called the OS-facing entry point of its own objects (the method the OS calls) and read the
//!   result back through the OS library or the framework;
//! - `a/gpui`: the app drove the framework's own entry point (key dispatch, the input-handler trait the platform
//!   layer wraps, window and menu APIs) on the real platform window and read the result back;
//! - `file`: a check of the generated link-scheme metadata or of argument parsing;
//! - `info`: a reading, recorded but not judged.
//!
//! Nothing here opens a file dialog, registers a link scheme, changes a setting or touches another application.

use std::{cell::RefCell, fmt::Write as _, rc::Rc, time::Duration};

use gpui::{
    AnyWindowHandle, App, AsyncApp, ElementInputHandler, InputHandler, Keystroke, WindowHandle,
};

#[cfg(target_os = "macos")]
use crate::popover_bounds;
use crate::{TrialState, TrialView, text_field::TextField};

pub struct Options {
    pub out: Option<String>,
}

#[derive(Clone)]
struct Check {
    item: &'static str,
    name: String,
    level: &'static str,
    ok: bool,
    detail: String,
}

#[derive(Default)]
struct Report(Vec<Check>);

impl Report {
    fn add(
        &mut self,
        item: &'static str,
        level: &'static str,
        name: impl Into<String>,
        ok: bool,
        detail: impl Into<String>,
    ) {
        let check = Check {
            item,
            name: name.into(),
            level,
            ok,
            detail: detail.into(),
        };
        println!(
            "{} [{}] ({}) {}: {}",
            if check.level == "info" {
                "INFO"
            } else if check.ok {
                "PASS"
            } else {
                "FAIL"
            },
            check.item,
            check.level,
            check.name,
            check.detail
        );
        self.0.push(check);
    }

    fn failures(&self) -> usize {
        self.0.iter().filter(|c| c.level != "info" && !c.ok).count()
    }

    fn json(&self) -> String {
        fn esc(s: &str) -> String {
            let mut out = String::new();
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    c if (c as u32) < 0x20 => {
                        let _ = write!(out, "\\u{:04x}", c as u32);
                    }
                    c => out.push(c),
                }
            }
            out
        }
        let mut out = String::from("{\n  \"platform\": \"");
        out.push_str(std::env::consts::OS);
        out.push_str("\",\n  \"checks\": [\n");
        for (i, c) in self.0.iter().enumerate() {
            let _ = writeln!(
                out,
                "    {{\"item\": \"{}\", \"level\": \"{}\", \"ok\": {}, \"name\": \"{}\", \"detail\": \"{}\"}}{}",
                esc(c.item),
                c.level,
                c.ok,
                esc(&c.name),
                esc(&c.detail),
                if i + 1 < self.0.len() { "," } else { "" }
            );
        }
        out.push_str("  ]\n}\n");
        out
    }
}

pub fn start(options: Options, cx: &mut App) {
    cx.spawn(async move |cx| {
        let report = Rc::new(RefCell::new(Report::default()));
        run(&report, cx).await;
        cleanup(&report, cx).await;
        let report = report.borrow();
        let failures = report.failures();
        println!(
            "selfcheck: {} checks, {} failed",
            report.0.iter().filter(|c| c.level != "info").count(),
            failures
        );
        if let Some(path) = &options.out
            && let Err(e) = std::fs::write(path, report.json())
        {
            eprintln!("selfcheck: could not write {path}: {e}");
        }
        // Everything is closed and the tray icon is gone; exit with the result.
        std::process::exit(i32::from(failures > 0));
    })
    .detach();
}

async fn wait(cx: &AsyncApp, ms: u64) {
    cx.background_executor()
        .timer(Duration::from_millis(ms))
        .await;
}

fn main_window(cx: &AsyncApp) -> WindowHandle<TrialView> {
    cx.read_global::<TrialState, _>(|s, _| s.main.expect("main window"))
}

fn field_state(
    field: &gpui::Entity<TextField>,
    cx: &AsyncApp,
) -> (String, Option<std::ops::Range<usize>>) {
    cx.update(|cx| {
        let f = field.read(cx);
        (f.text.clone(), f.marked.clone())
    })
}

/// Window updates go through the untyped handle: a typed `WindowHandle::update` leases the root view, and GPUI
/// renders that view during key dispatch, which would then find it already borrowed.
fn any<V: 'static>(handle: WindowHandle<V>) -> AnyWindowHandle {
    handle.into()
}

fn focus_field(
    main: WindowHandle<TrialView>,
    which: fn(&TrialView) -> gpui::Entity<TextField>,
    cx: &mut AsyncApp,
) -> gpui::Entity<TextField> {
    let field = cx.update(|cx| which(main.read(cx).expect("main window")));
    any(main)
        .update(cx, |_, window, cx| {
            let focus = field.read(cx).focus.clone();
            window.focus(&focus, cx);
        })
        .expect("main window");
    field
}

fn press(main: WindowHandle<TrialView>, key: &'static str, cx: &mut AsyncApp) -> bool {
    any(main)
        .update(cx, |_, window, cx| {
            window.dispatch_keystroke(Keystroke::parse(key).expect("keystroke"), cx)
        })
        .unwrap_or(false)
}

async fn run(report: &Rc<RefCell<Report>>, cx: &mut AsyncApp) {
    let r = |item, level, name: &str, ok: bool, detail: String| {
        report.borrow_mut().add(item, level, name, ok, detail)
    };
    let main = main_window(cx);
    wait(cx, 700).await;

    // ---------------------------------------------------------------- 1. text entry
    let layout = cx.update(|cx| cx.keyboard_layout().name().to_string());
    r(
        "1 text",
        "info",
        "keyboard layout reported by GPUI",
        true,
        layout,
    );

    let name = focus_field(main, |v| v.name.clone(), cx);
    wait(cx, 200).await;
    // Committed text through GPUI's key path: unbound keystrokes go to the platform window's registered handler.
    let typed = press(main, "a", cx) & press(main, "b", cx);
    let (text, _) = field_state(&name, cx);
    r(
        "1 text",
        "a/gpui",
        "single-line: typed keys reach the field through the platform window's input handler",
        typed && text == "ab",
        format!("text {text:?}"),
    );

    // Key event versus text: a bound shortcut runs its action and inserts nothing.
    let before = cx.read_global::<TrialState, _>(|s, _| s.hello_count);
    press(main, crate::sec!("shift-h"), cx);
    let after = cx.read_global::<TrialState, _>(|s, _| s.hello_count);
    let (text, _) = field_state(&name, cx);
    r(
        "3 menu",
        "a/gpui",
        "shortcut (cmd or ctrl)-shift-h dispatches its action through the keymap",
        after == before + 1 && text == "ab",
        format!("hello count {before} -> {after}, field still {text:?}"),
    );

    wait(cx, 150).await;
    #[cfg(target_os = "macos")]
    {
        // The same composition an input method performs, through GPUI's NSTextInputClient methods on its own view.
        let view = any(main)
            .update(cx, |_, window, _| crate::mac::View::from_window(window))
            .ok()
            .flatten()
            .expect("NSView");
        view.set_marked_text("k", (1, 0));
        let m1 = view.marked_range();
        view.set_marked_text("か", (1, 0));
        view.set_marked_text("かん", (2, 0));
        // Let GPUI paint the marked text before the candidate window's position is asked for, as an input method
        // does after `invalidateCharacterCoordinates`.
        wait(cx, 120).await;
        let has = view.has_marked_text();
        let m3 = view.marked_range();
        let sel = view.selected_range();
        let (mid_text, mid_marked) = field_state(&name, cx);
        let rect = view.first_rect_for_range((2, 2));
        let frame = view.window_frame();
        let inside = frame.is_some_and(|f| {
            rect.size.width > 0.0
                && rect.origin.x >= f.origin.x
                && rect.origin.x + rect.size.width <= f.origin.x + f.size.width
                && rect.origin.y >= f.origin.y
                && rect.origin.y + rect.size.height <= f.origin.y + f.size.height
        });
        view.insert_text("漢");
        let (text, marked) = field_state(&name, cx);
        let has_after = view.has_marked_text();
        r(
            "1 text",
            "a/os",
            "single-line: composition via NSTextInputClient (setMarkedText x3, insertText)",
            m1 == Some((2, 1))
                && has
                && m3 == Some((2, 2))
                && sel == Some((4, 0))
                && mid_text == "abかん"
                && mid_marked == Some(2..8)
                && text == "ab漢"
                && marked.is_none()
                && !has_after,
            format!(
                "markedRange after begin {m1:?}, after updates {m3:?}, selectedRange {sel:?}, text while composing {mid_text:?}; after commit {text:?}, hasMarkedText {has_after}"
            ),
        );
        r(
            "1 text",
            "a/os",
            "candidate window rectangle (firstRectForCharacterRange) lies inside the window",
            inside,
            format!(
                "rect origin {:.0},{:.0} size {:.0}x{:.0}; window frame {:?}",
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height,
                frame.map(|f| (
                    f.origin.x.round(),
                    f.origin.y.round(),
                    f.size.width.round(),
                    f.size.height.round()
                ))
            ),
        );

        // Dead key as AppKit's input context delivers it: the accent as marked text, then the composed letter.
        view.set_marked_text("´", (1, 0));
        let accent = view.marked_range();
        view.insert_text("é");
        let (text, marked) = field_state(&name, cx);
        r(
            "1 text",
            "a/os",
            "single-line: dead-key sequence via NSTextInputClient (marked accent, then é)",
            accent == Some((3, 1)) && text == "ab漢é" && marked.is_none(),
            format!("accent marked at {accent:?}, then text {text:?}"),
        );

        // Not checked here: a dead key typed on the keyboard. A synthetic key event sent to the window carries
        // fixed characters, and AppKit's input context does not run the keyboard layout's dead-key translation on
        // it (tried in this trial: Option-E inserted an empty string, then E inserted "e"). The translation belongs
        // to the OS input layer, so that step is in CHECKLIST.md.

        // Cancelled composition: empty marked text, then unmarkText.
        view.set_marked_text("にほ", (2, 0));
        let during = field_state(&name, cx).0;
        view.set_marked_text("", (0, 0));
        view.unmark_text();
        let (text, marked) = field_state(&name, cx);
        r(
            "1 text",
            "a/os",
            "single-line: cancelled composition via NSTextInputClient leaves the text as it was",
            during == "ab漢éにほ" && text == "ab漢é" && marked.is_none() && !view.has_marked_text(),
            format!("while composing {during:?}, after cancel {text:?}"),
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        let (ok, detail) = compose_via_handler(main, &name, cx).await;
        r(
            "1 text",
            "a/gpui",
            "single-line: composition, dead key and cancel via the InputHandler trait",
            ok,
            detail,
        );
    }

    // The multi-line field: the same sequence through the InputHandler trait GPUI wraps for every platform, then
    // Enter (bound to an action in this field) for a newline.
    let notes = focus_field(main, |v| v.notes.clone(), cx);
    wait(cx, 200).await;
    press(main, "a", cx);
    press(main, "b", cx);
    let (ok, detail) = compose_via_handler(main, &notes, cx).await;
    r(
        "1 text",
        "a/gpui",
        "multi-line: composition, dead key and cancel via the InputHandler trait",
        ok,
        detail,
    );
    press(main, "enter", cx);
    press(main, "x", cx);
    wait(cx, 100).await;
    let (text, _) = field_state(&notes, cx);
    let rects = any(main)
        .update(cx, |_, window, cx| {
            let bounds = notes.read(cx).last_bounds()?;
            let mut h = ElementInputHandler::new(bounds, notes.clone());
            Some((
                h.bounds_for_range(0..1, window, cx)?,
                h.bounds_for_range(5..6, window, cx)?,
            ))
        })
        .ok()
        .flatten();
    r(
        "1 text",
        "a/gpui",
        "multi-line: Enter makes a new line; the second line's bounds are below the first",
        text == "ab漢é\nx" && rects.is_some_and(|(a, b)| b.top() > a.top()),
        format!(
            "text {text:?}, line rects {:?}",
            rects.map(|(a, b)| (f32::from(a.top()), f32::from(b.top())))
        ),
    );

    // Keyboard: Tab moves the focus from the single-line field to the multi-line one, Shift-Tab back (goal G10).
    let name = focus_field(main, |v| v.name.clone(), cx);
    wait(cx, 100).await;
    press(main, "tab", cx);
    wait(cx, 100).await;
    let on_notes = any(main)
        .update(cx, |_, w, cx| notes.read(cx).focus.is_focused(w))
        .unwrap_or(false);
    press(main, "shift-tab", cx);
    wait(cx, 100).await;
    let back = any(main)
        .update(cx, |_, w, cx| name.read(cx).focus.is_focused(w))
        .unwrap_or(false);
    r(
        "1 text",
        "a/gpui",
        "Tab moves the focus to the next field and Shift-Tab back",
        on_notes && back,
        format!("after Tab on Notes: {on_notes}; after Shift-Tab on Name: {back}"),
    );

    // ---------------------------------------------------------------- 3. application menu
    let menus = cx.update(|cx| cx.get_menus());
    let names: Vec<String> = menus.iter().flatten().map(|m| m.name.to_string()).collect();
    r(
        "3 menu",
        "a/gpui",
        "menus read back from GPUI (get_menus)",
        names == ["UniDeX Trial", "File", "Trial"],
        format!("{names:?}"),
    );
    #[cfg(target_os = "macos")]
    {
        let entries = crate::mac::main_menu_entries();
        let listing: Vec<String> = entries
            .iter()
            .map(|e| {
                format!(
                    "{} > {} [{}{}]",
                    e.menu, e.title, e.modifiers, e.key_equivalent
                )
            })
            .collect();
        let has = |title: &str, key: &str| {
            entries
                .iter()
                .any(|e| e.title == title && format!("{}{}", e.modifiers, e.key_equivalent) == key)
        };
        r(
            "3 menu",
            "a/os",
            "native menu bar (NSApp.mainMenu) holds the items with their shortcuts",
            has("Show Popover", "shift-cmd-p")
                && has("Open...", "cmd-o")
                && has("Quit UniDeX Trial", "cmd-q"),
            listing.join("; "),
        );
        let before = cx.read_global::<TrialState, _>(|s, _| s.hello_count);
        let found = crate::mac::perform_menu_item("Trial", "Say Hello");
        wait(cx, 50).await;
        let after = cx.read_global::<TrialState, _>(|s, _| s.hello_count);
        r(
            "3 menu",
            "a/os",
            "performing the native menu item runs the GPUI action",
            found && after == before + 1,
            format!("item found {found}, hello count {before} -> {after}"),
        );
    }

    // ---------------------------------------------------------------- 4. windows
    let second = cx.update(|cx| crate::open_second_window(None, None, cx));
    wait(cx, 400).await;
    let count = cx.update(|cx| cx.windows().len());
    r(
        "4 windows",
        "a/gpui",
        "a second window opens (cx.windows())",
        second.is_some() && count == 2,
        format!("{count} windows"),
    );
    let stack = cx.update(|cx| cx.window_stack().map(|s| s.len()));
    r(
        "4 windows",
        "info",
        "window stack front to back (cx.window_stack())",
        true,
        format!("{stack:?}"),
    );
    if let Some(second) = second {
        let state = any(second).update(cx, |_, window, _| {
            window.activate_window();
            (window.bounds(), window.window_bounds())
        });
        let content = any(second)
            .update(cx, |_, window, _| window.viewport_size())
            .ok();
        wait(cx, 300).await;
        let active = any(second)
            .update(cx, |_, w, _| w.is_window_active())
            .unwrap_or(false);
        r(
            "4 windows",
            "info",
            "the second window is active after activate_window()",
            true,
            format!("{active}"),
        );
        if let Ok((bounds, restore)) = state {
            r(
                "4 windows",
                "a/gpui",
                "window position and size readable for restoring (bounds, window_bounds)",
                f32::from(bounds.size.width) > 0.0,
                format!("bounds {:?}, restore {:?}", bounds, restore),
            );
            // Restoring: open a window with the bounds just read back and compare.
            let reopened = cx.update(|cx| crate::open_second_window(None, Some(restore), cx));
            wait(cx, 300).await;
            let again = reopened.and_then(|h| any(h).update(cx, |_, w, _| w.window_bounds()).ok());
            let size_of = |b: gpui::WindowBounds| b.get_bounds().size;
            let same = again.is_some_and(|a| size_of(a) == size_of(restore));
            r(
                "4 windows",
                "info",
                "window_bounds() read back, then used to open a window: same size?",
                same,
                format!(
                    "{same}: asked {:?}, got {:?} (macOS: the read-back size is the frame with the title bar, while \
                     WindowOptions takes it as the content size, so a naive restore grows by the title bar)",
                    size_of(restore),
                    again.map(size_of)
                ),
            );
            if let Some(h) = reopened {
                any(h).update(cx, |_, w, _| w.remove_window()).ok();
                wait(cx, 200).await;
            }
            // The same, restoring the origin from window_bounds() and the size from viewport_size() (the content).
            if let Some(content) = content {
                let fixed = gpui::WindowBounds::Windowed(gpui::Bounds::new(
                    restore.get_bounds().origin,
                    content,
                ));
                let reopened = cx.update(|cx| crate::open_second_window(None, Some(fixed), cx));
                wait(cx, 300).await;
                let again =
                    reopened.and_then(|h| any(h).update(cx, |_, w, _| w.window_bounds()).ok());
                let same = again.is_some_and(|a| a.get_bounds() == restore.get_bounds());
                r(
                    "4 windows",
                    "a/gpui",
                    "restoring with window_bounds() origin and viewport_size() gives the same window",
                    same,
                    format!(
                        "first window {:?}, restored {:?}",
                        restore.get_bounds(),
                        again.map(|a| a.get_bounds())
                    ),
                );
                if let Some(h) = reopened {
                    any(h).update(cx, |_, w, _| w.remove_window()).ok();
                    wait(cx, 200).await;
                }
            }
        }
    }
    let displays = cx.update(|cx| {
        cx.displays()
            .iter()
            .map(|d| (d.id(), d.bounds()))
            .collect::<Vec<_>>()
    });
    r(
        "4 windows",
        "info",
        "displays reported",
        true,
        format!("{displays:?}"),
    );
    if displays.len() > 1 {
        let other = displays[1].0;
        let handle = cx.update(|cx| crate::open_second_window(Some(other), None, cx));
        wait(cx, 400).await;
        let on = handle.and_then(|h| {
            any(h)
                .update(cx, |_, w, cx| w.display(cx).map(|d| d.id()))
                .ok()
                .flatten()
        });
        r(
            "4 windows",
            "a/gpui",
            "a window opened on the second display reports that display",
            on == Some(other),
            format!("asked {other:?}, got {on:?}"),
        );
        if let Some(h) = handle {
            any(h).update(cx, |_, w, _| w.remove_window()).ok();
        }
    } else {
        r(
            "4 windows",
            "info",
            "moving between displays",
            true,
            "only one display reported; not exercised".to_string(),
        );
    }
    if let Some(second) = second {
        any(second).update(cx, |_, w, _| w.remove_window()).ok();
        wait(cx, 300).await;
        let count = cx.update(|cx| cx.windows().len());
        r(
            "4 windows",
            "a/gpui",
            "closing the second window leaves one",
            count == 1,
            format!("{count} windows"),
        );
    }

    // ---------------------------------------------------------------- 2. tray icon and popover
    let created = cx.read_global::<TrialState, _>(|s, _| s.tray.is_some());
    let rect = cx.read_global::<TrialState, _>(|s, _| s.tray.as_ref().and_then(|t| t.rect()));
    r(
        "2 tray",
        // On Linux the AppIndicator object is created even with no tray host, so success proves little there.
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            "a/os"
        } else {
            "info"
        },
        "tray icon created (the OS call returned success)",
        created,
        format!("icon rect from the OS {rect:?}"),
    );
    #[cfg(target_os = "windows")]
    {
        // tray-icon's own callback message (WM_USER_TRAYICON, 6002 in its source) with the mouse message in lparam,
        // posted to the icon's hidden window: what the shell sends on a click. GPUI's message loop dispatches it.
        let hwnd = cx.read_global::<TrialState, _>(|s, _| {
            s.tray
                .as_ref()
                .and_then(|t| t.local())
                .map(|t| t.window_handle())
        });
        if let Some(hwnd) = hwnd {
            let before = cx.read_global::<TrialState, _>(|s, _| s.tray_log.len());
            let posted = unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                    hwnd,
                    6002,
                    0,
                    windows_sys::Win32::UI::WindowsAndMessaging::WM_LBUTTONDOWN as isize,
                )
            } != 0;
            wait(cx, 300).await;
            let log = cx.read_global::<TrialState, _>(|s, _| s.tray_log[before..].to_vec());
            let got_click = log.iter().any(|l| l.contains("button: \"left\""));
            r(
                "2 tray",
                "a/os",
                "a click message posted to the icon's window reaches GPUI's event loop",
                posted && got_click,
                format!("posted {posted}; events {log:?}"),
            );
        }
    }
    #[cfg(target_os = "macos")]
    {
        let item = cx.read_global::<TrialState, _>(|s, _| {
            s.tray
                .as_ref()
                .and_then(|t| t.local())
                .and_then(|t| t.ns_status_item())
        });
        if let Some(item) = &item {
            let before = cx.read_global::<TrialState, _>(|s, _| s.tray_log.len());
            let clicked = crate::mac::click_status_item(item);
            wait(cx, 300).await;
            let log = cx.read_global::<TrialState, _>(|s, _| s.tray_log[before..].to_vec());
            let got_click = log
                .iter()
                .any(|l| l.contains("button: \"left\"") && l.contains("rect: Some"));
            r(
                "2 tray",
                "a/os",
                "a synthetic click on the icon's view reaches GPUI's event loop with the icon rect",
                clicked.is_ok() && got_click,
                format!("{clicked:?}; events {log:?}"),
            );
        }
        let scale = cx.update(crate::tray_scale);
        let rect = cx
            .read_global::<TrialState, _>(|s, _| s.last_tray_rect)
            .or(rect);
        if rect.is_some() {
            let (want, display) = cx.update(|cx| popover_bounds(rect, scale, cx));
            cx.update(|cx| crate::open_popover(want, display, cx));
            wait(cx, 400).await;
            let popover = cx.read_global::<TrialState, _>(|s, _| s.popover);
            let got = popover.and_then(|p| {
                any(p)
                    .update(cx, |_, w, cx| (w.bounds(), w.display(cx).map(|d| d.id())))
                    .ok()
            });
            let close = |a: f32, b: f32| (a - b).abs() <= 2.0;
            let ok = got.is_some_and(|(g, on)| {
                close(f32::from(g.origin.x), f32::from(want.origin.x))
                    && close(f32::from(g.origin.y), f32::from(want.origin.y))
                    && (display.is_none() || on == display)
            });
            r(
                "2 tray",
                "a/os",
                "a popover window opens under the icon (position read back)",
                ok,
                format!(
                    "tray rect {rect:?} (physical, scale {scale}); asked {:?} on {display:?}; window at {got:?}",
                    want.origin
                ),
            );
        }
    }

    // ---------------------------------------------------------------- 6. link scheme
    #[cfg(target_os = "macos")]
    {
        let before = cx.read_global::<TrialState, _>(|s, _| s.links.len());
        let delivered = crate::mac::deliver_open_urls(&["unidex://thread/42?from=selfcheck"]);
        wait(cx, 200).await;
        let links = cx.read_global::<TrialState, _>(|s, _| s.links[before..].to_vec());
        r(
            "6 link",
            "a/os",
            "a link handed to the app delegate (application:openURLs:) reaches on_open_urls",
            delivered && links == ["unidex://thread/42?from=selfcheck"],
            format!("{links:?}"),
        );
    }
    for (name, ok, detail) in crate::link_scheme::headless_checks() {
        r("6 link", "file", &name, ok, detail);
    }

    // ---------------------------------------------------------------- 7. accessibility
    let (enabled, active_before) = any(main)
        .update(cx, |_, w, _| (w.is_a11y_enabled(), w.is_a11y_active()))
        .unwrap_or((false, false));
    r(
        "7 a11y",
        "info",
        "GPUI accessibility enabled / active before any assistive technology asked",
        true,
        format!("enabled {enabled}, active {active_before}"),
    );
    #[cfg(target_os = "macos")]
    {
        let view = any(main)
            .update(cx, |_, window, _| crate::mac::View::from_window(window))
            .ok()
            .flatten()
            .expect("NSView");
        let view = view.content_view().expect("content view");
        // Bring the main window to the front and focus the single-line field, so the focused element can be checked
        // (AccessKit reports focus only inside the key window).
        cx.update(|cx| cx.activate(true));
        any(main).update(cx, |_, w, _| w.activate_window()).ok();
        focus_field(main, |v| v.name.clone(), cx);
        wait(cx, 500).await;
        let key_window = any(main)
            .update(cx, |_, w, _| w.is_window_active())
            .unwrap_or(false);
        let first = view.ax_children().len();
        wait(cx, 600).await;
        let active = any(main)
            .update(cx, |_, w, _| w.is_a11y_active())
            .unwrap_or(false);
        r(
            "7 a11y",
            "a/os",
            "asking the view for its accessibility children activates GPUI's tree",
            active,
            format!("children on the first ask {first}; GPUI active afterwards {active}"),
        );
        let tree = view.ax_tree(8);
        let mut all = Vec::new();
        for n in &tree {
            n.flatten(&mut all);
        }
        let listing: Vec<String> = all.iter().map(|n| n.short()).collect();
        r(
            "7 a11y",
            "info",
            "elements the window publishes (role, label, value)",
            true,
            listing.join(" | "),
        );
        let find = |role: &str, label: &str| {
            all.iter()
                .find(|n| {
                    n.role == role
                        && (n.label.as_deref() == Some(label) || n.title.as_deref() == Some(label))
                })
                .cloned()
        };
        let count = find("AXButton", "Count");
        r(
            "7 a11y",
            "a/os",
            "the labelled button is published as AXButton \"Count\"",
            count.is_some(),
            format!("{:?}", count.map(|n| n.short())),
        );
        let name_node = find("AXTextField", "Name");
        r(
            "7 a11y",
            "a/os",
            "the single-line field is an AXTextField labelled \"Name\" whose value is its text",
            name_node
                .as_ref()
                .is_some_and(|n| n.value.as_deref() == Some("ab漢é")),
            format!("{:?}", name_node.as_ref().map(|n| n.short())),
        );
        let notes_node = find("AXTextArea", "Notes");
        r(
            "7 a11y",
            "a/os",
            "the multi-line field is an AXTextArea labelled \"Notes\" whose value is its text",
            notes_node
                .as_ref()
                .is_some_and(|n| n.value.as_deref() == Some("ab漢é\nx")),
            format!("{:?}", notes_node.map(|n| n.short())),
        );
        let runs = name_node.as_ref().and_then(|n| n.text_range);
        r(
            "7 a11y",
            "a/os",
            "with text runs, the field reports its characters and caret (AXNumberOfCharacters, AXSelectedTextRange)",
            runs == Some((4, (4, 0))),
            format!("{runs:?} for \"ab漢é\" with the caret at the end"),
        );
        let focused = view.ax_focused();
        // Someone working at this Mac can keep another app in front; then there is no key window to judge.
        let focus_level = if key_window { "a/os" } else { "info" };
        r(
            "7 a11y",
            focus_level,
            "the focused element reported to the OS is the focused text field",
            focused.as_ref().is_some_and(|n| {
                n.role == "AXTextField" && n.label.as_deref().or(n.title.as_deref()) == Some("Name")
            }),
            format!(
                "{:?}; main window key: {key_window}",
                focused.map(|n| n.short())
            ),
        );
        let label = all
            .iter()
            .find(|n| {
                n.role == "AXStaticText" && n.value.as_deref() == Some("UniDeX platform trial")
            })
            .cloned();
        r(
            "7 a11y",
            "a/os",
            "plain text (text! macro) is published as AXStaticText with its words",
            label.is_some(),
            format!("{:?}", label.map(|n| n.short())),
        );
        let plain = all.iter().any(|n| {
            n.value
                .as_deref()
                .is_some_and(|v| v.contains("Plain string child"))
                || n.label
                    .as_deref()
                    .is_some_and(|v| v.contains("Plain string child"))
        });
        r(
            "7 a11y",
            "a/os",
            "a plain string child (not text!) is absent from the published tree",
            !plain,
            format!("found: {plain}"),
        );
        let doubled = all
            .iter()
            .filter(|n| n.role == "AXStaticText" && n.value.as_deref() == Some("Count"))
            .count();
        r(
            "7 a11y",
            "a/os",
            "the button's visible text is not published a second time (Text::new_inaccessible)",
            doubled == 0,
            format!("AXStaticText \"Count\" nodes: {doubled}"),
        );
        let json = any(main)
            .update(cx, |_, w, _| w.debug_a11y_tree_json())
            .ok()
            .flatten()
            .unwrap_or_default();
        r(
            "7 a11y",
            "info",
            "GPUI's own record of the last tree it sent (debug_a11y_tree_json), first 600 bytes",
            true,
            json.chars().take(600).collect::<String>(),
        );
        let title = view.window_title();
        r(
            "7 a11y",
            "info",
            "the main window's title (what VoiceOver says for the window)",
            true,
            title,
        );

        // An unlabelled window: the popover has no title.
        let popover = cx.read_global::<TrialState, _>(|s, _| s.popover);
        if let Some(p) = popover
            && let Some(pview) = any(p)
                .update(cx, |_, w, _| crate::mac::View::from_window(w))
                .ok()
                .flatten()
                .and_then(|v| v.content_view())
        {
            pview.ax_children();
            wait(cx, 500).await;
            let mut pall = Vec::new();
            for n in pview.ax_tree(6) {
                n.flatten(&mut pall);
            }
            r(
                "7 a11y",
                "info",
                "the untitled popover window: its title and what it publishes",
                true,
                format!(
                    "title {:?}; {}",
                    pview.window_title(),
                    pall.iter()
                        .map(|n| n.short())
                        .collect::<Vec<_>>()
                        .join(" | ")
                ),
            );
        }
    }

    // ---------------------------------------------------------------- 8. other readings
    let (appearance, thermal) = cx.update(|cx| (cx.window_appearance(), cx.thermal_state()));
    r(
        "8 other",
        "info",
        "appearance and thermal state readable",
        true,
        format!("{appearance:?}, {thermal:?}"),
    );
}

/// Composition, a dead key and a cancelled composition through `ElementInputHandler`, the `InputHandler` GPUI's
/// `PlatformInputHandler` wraps on every platform. The field must already hold "ab" with the caret at the end.
async fn compose_via_handler(
    main: WindowHandle<TrialView>,
    field: &gpui::Entity<TextField>,
    cx: &mut AsyncApp,
) -> (bool, String) {
    let step =
        |cx: &mut AsyncApp,
         f: &dyn Fn(&mut ElementInputHandler<TextField>, &mut gpui::Window, &mut App)| {
            let field = field.clone();
            any(main)
                .update(cx, |_, window, cx| {
                    let bounds = field.read(cx).last_bounds()?;
                    let mut h = ElementInputHandler::new(bounds, field.clone());
                    f(&mut h, window, cx);
                    let f = field.read(cx);
                    Some((f.text.clone(), f.marked.clone()))
                })
                .ok()
                .flatten()
        };
    let mut log = Vec::new();
    log.push(step(cx, &|h, w, cx| {
        h.replace_and_mark_text_in_range(None, "k", Some(1..1), w, cx)
    }));
    step(cx, &|h, w, cx| {
        h.replace_and_mark_text_in_range(None, "か", Some(1..1), w, cx)
    });
    log.push(step(cx, &|h, w, cx| {
        h.replace_and_mark_text_in_range(None, "かん", Some(2..2), w, cx)
    }));
    // The candidate window's position is asked for after the marked text was painted.
    wait(cx, 120).await;
    let field2 = field.clone();
    let (marked, rect) = any(main)
        .update(cx, |_, window, cx| {
            let bounds = field2.read(cx).last_bounds()?;
            let mut h = ElementInputHandler::new(bounds, field2.clone());
            let marked = h.marked_text_range(window, cx);
            let rect = marked
                .clone()
                .and_then(|m| h.bounds_for_range(m, window, cx));
            Some((marked, rect))
        })
        .ok()
        .flatten()
        .unwrap_or((None, None));
    log.push(step(cx, &|h, w, cx| {
        h.replace_text_in_range(None, "漢", w, cx)
    }));
    log.push(step(cx, &|h, w, cx| {
        h.replace_and_mark_text_in_range(None, "´", Some(1..1), w, cx)
    }));
    log.push(step(cx, &|h, w, cx| {
        h.replace_text_in_range(None, "é", w, cx)
    }));
    step(cx, &|h, w, cx| {
        h.replace_and_mark_text_in_range(None, "にほ", Some(2..2), w, cx)
    });
    log.push(step(cx, &|h, w, cx| {
        h.replace_and_mark_text_in_range(None, "", Some(0..0), w, cx);
        h.unmark_text(w, cx);
    }));
    let expected = [
        ("abk", Some(2..3)),
        ("abかん", Some(2..8)),
        ("ab漢", None),
        ("ab漢´", Some(5..7)),
        ("ab漢é", None),
        ("ab漢é", None),
    ];
    let ok = log.len() == expected.len()
        && log
            .iter()
            .zip(expected.iter())
            .all(|(got, (et, em))| got.as_ref().is_some_and(|(t, m)| t == et && m == em))
        && marked == Some(2..4)
        && rect.is_some_and(|r| f32::from(r.size.width) > 0.0);
    (
        ok,
        format!("states {log:?}; marked (UTF-16) {marked:?}; candidate rect {rect:?}"),
    )
}

async fn cleanup(report: &Rc<RefCell<Report>>, cx: &mut AsyncApp) {
    // The tray icon goes first, and its removal is observed.
    #[cfg(target_os = "macos")]
    let (before, weak) = {
        let item = cx.read_global::<TrialState, _>(|s, _| {
            s.tray
                .as_ref()
                .and_then(|t| t.local())
                .and_then(|t| t.ns_status_item())
        });
        (
            item.as_ref().map(|i| crate::mac::status_item_state(i)),
            item.as_ref().map(crate::mac::weak),
        )
        // The strong reference ends here, so only tray-icon and AppKit hold the item.
    };
    let had = cx.update(|cx| cx.global_mut::<TrialState>().tray.take().is_some());
    wait(cx, 300).await;
    #[cfg(target_os = "macos")]
    {
        let alive = weak.as_ref().and_then(|w| w.load());
        let after = alive.as_ref().map(|i| crate::mac::status_item_state(i));
        report.borrow_mut().add(
            "2 tray",
            "a/os",
            "tray icon removed before exit",
            had && (alive.is_none()
                || after.as_deref().is_some_and(|a| {
                    a.contains("trayIconViews=0") && a.contains("statusWindowVisible=false")
                })),
            format!(
                "before {before:?}; after: status item {}",
                match &after {
                    None => "deallocated".to_string(),
                    Some(a) => format!("still allocated, {a}"),
                }
            ),
        );
    }
    #[cfg(not(target_os = "macos"))]
    report.borrow_mut().add(
        "2 tray",
        "a/gpui",
        "tray icon dropped before exit",
        had,
        String::new(),
    );

    // Then every window. Removing the last one would quit through on_window_closed, so the main window stays and
    // the process exits right after with the result.
    cx.update(|cx| {
        let main = cx.global::<TrialState>().main;
        for handle in cx.windows() {
            if Some(handle) != main.map(Into::into) {
                handle.update(cx, |_, w, _| w.remove_window()).ok();
            }
        }
    });
    wait(cx, 200).await;
    let left = cx.update(|cx| cx.windows().len());
    report.borrow_mut().add(
        "4 windows",
        "info",
        "windows left open just before exit (the main window)",
        true,
        format!("{left}"),
    );
}
