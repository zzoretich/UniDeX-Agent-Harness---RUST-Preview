//! macOS only: the self-check's probes into this process's own AppKit objects.
//!
//! Each probe calls the same Objective-C entry point the OS calls, on an object this process owns, so the
//! framework's real code path runs and its result is read back through AppKit:
//! - the `NSTextInputClient` methods of GPUI's own view (what an input method calls through `NSTextInputContext`);
//! - the `NSAccessibility` methods of that view and of the elements AccessKit publishes under it (what VoiceOver
//!   calls); asking for the children is also what activates GPUI's accessibility tree;
//! - the application's main menu (`NSApp.mainMenu`), read back and performed (what a click on a menu item does);
//! - the application delegate's `application:openURLs:` (what LaunchServices calls for a `unidex://` link);
//! - the tray icon's own view, sent a synthetic mouse-down and mouse-up (what a click on the icon delivers).
//!
//! Nothing here touches another application, the screen, or any system setting. Every function must run on the
//! main thread and outside a GPUI update, because GPUI's handlers borrow the app themselves.

use objc2::{
    MainThreadMarker, msg_send,
    rc::Retained,
    runtime::{AnyObject, Bool},
};
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSScreen, NSStatusItem, NSView,
};
use objc2_foundation::{NSArray, NSPoint, NSRange, NSRect, NSString, NSURL};

/// `NSNotFound`: "no replacement range" in `NSTextInputClient` calls.
const NOT_FOUND: usize = isize::MAX as usize;

fn mtm() -> MainThreadMarker {
    MainThreadMarker::new().expect("main thread")
}

/// GPUI's content view of a window, from the window's raw handle.
#[derive(Clone, Copy)]
pub struct View(pub *mut AnyObject);

impl View {
    pub fn from_window(window: &gpui::Window) -> Option<Self> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::AppKit(handle) => Some(View(handle.ns_view.as_ptr().cast())),
            _ => None,
        }
    }

    fn obj(&self) -> &AnyObject {
        // SAFETY: the view outlives the self-check, which closes its windows only at the end.
        unsafe { &*self.0 }
    }

    // NSTextInputClient, as NSTextInputContext calls it.

    pub fn set_marked_text(&self, text: &str, selected: (usize, usize)) {
        let text = NSString::from_str(text);
        let selected = NSRange::new(selected.0, selected.1);
        let replacement = NSRange::new(NOT_FOUND, 0);
        unsafe {
            let _: () = msg_send![self.obj(), setMarkedText: &*text, selectedRange: selected, replacementRange: replacement];
        }
    }

    pub fn insert_text(&self, text: &str) {
        let text = NSString::from_str(text);
        let replacement = NSRange::new(NOT_FOUND, 0);
        unsafe {
            let _: () = msg_send![self.obj(), insertText: &*text, replacementRange: replacement];
        }
    }

    pub fn unmark_text(&self) {
        unsafe {
            let _: () = msg_send![self.obj(), unmarkText];
        }
    }

    pub fn has_marked_text(&self) -> bool {
        unsafe {
            let b: Bool = msg_send![self.obj(), hasMarkedText];
            b.as_bool()
        }
    }

    /// `None` when AppKit's "not found" location comes back.
    pub fn marked_range(&self) -> Option<(usize, usize)> {
        let r: NSRange = unsafe { msg_send![self.obj(), markedRange] };
        (r.location != NOT_FOUND).then_some((r.location, r.length))
    }

    pub fn selected_range(&self) -> Option<(usize, usize)> {
        let r: NSRange = unsafe { msg_send![self.obj(), selectedRange] };
        (r.location != NOT_FOUND).then_some((r.location, r.length))
    }

    /// Screen rectangle (bottom-left origin) where the candidate window goes for a range.
    pub fn first_rect_for_range(&self, range: (usize, usize)) -> NSRect {
        let mut actual = NSRange::new(0, 0);
        unsafe {
            msg_send![self.obj(), firstRectForCharacterRange: NSRange::new(range.0, range.1), actualRange: &mut actual]
        }
    }

    /// The window's frame in screen coordinates.
    pub fn window_frame(&self) -> Option<NSRect> {
        let view: &NSView = unsafe { &*(self.0 as *const NSView) };
        view.window().map(|w| w.frame())
    }

    pub fn window_title(&self) -> String {
        let view: &NSView = unsafe { &*(self.0 as *const NSView) };
        view.window()
            .map(|w| w.title().to_string())
            .unwrap_or_default()
    }

    /// The window's content view. GPUI's own view is a subview of it, and AccessKit's adapter subclasses the content
    /// view (`SubclassingAdapter::for_window`), so accessibility questions go there.
    pub fn content_view(&self) -> Option<View> {
        let view: &NSView = unsafe { &*(self.0 as *const NSView) };
        let content = view.window()?.contentView()?;
        Some(View(Retained::as_ptr(&content) as *mut AnyObject))
    }

    // NSAccessibility, as VoiceOver calls it.

    /// Ask for the view's accessibility children, as assistive technology does. The first call activates
    /// AccessKit, which asks GPUI for its tree and makes GPUI build one on the next frame.
    pub fn ax_children(&self) -> Vec<Retained<AnyObject>> {
        children_of(self.obj())
    }

    pub fn ax_focused(&self) -> Option<AxNode> {
        let focused: Option<Retained<AnyObject>> =
            unsafe { msg_send![self.obj(), accessibilityFocusedUIElement] };
        focused.map(|f| AxNode::read(&f, 0))
    }

    pub fn ax_tree(&self, depth: usize) -> Vec<AxNode> {
        self.ax_children()
            .iter()
            .map(|c| AxNode::read(c, depth))
            .collect()
    }
}

fn children_of(obj: &AnyObject) -> Vec<Retained<AnyObject>> {
    let array: Option<Retained<NSArray<AnyObject>>> =
        unsafe { msg_send![obj, accessibilityChildren] };
    array.map(|a| a.to_vec()).unwrap_or_default()
}

fn string_attr(obj: &AnyObject, which: &str) -> Option<String> {
    let value: Option<Retained<AnyObject>> = unsafe {
        match which {
            "role" => msg_send![obj, accessibilityRole],
            "subrole" => msg_send![obj, accessibilitySubrole],
            "label" => msg_send![obj, accessibilityLabel],
            "title" => msg_send![obj, accessibilityTitle],
            "value" => msg_send![obj, accessibilityValue],
            _ => None,
        }
    };
    // `description` gives the text of an NSString and a readable form of an NSNumber.
    value.map(|v| {
        let d: Retained<NSString> = unsafe { msg_send![&*v, description] };
        d.to_string()
    })
}

/// What one published accessibility element says about itself.
#[derive(Debug, Clone)]
pub struct AxNode {
    pub role: String,
    pub subrole: Option<String>,
    pub label: Option<String>,
    pub title: Option<String>,
    pub value: Option<String>,
    pub focused: bool,
    /// For text fields: `accessibilityNumberOfCharacters` and `accessibilitySelectedTextRange`, which a screen
    /// reader needs to read by character and to follow the caret.
    pub text_range: Option<(isize, (usize, usize))>,
    pub children: Vec<AxNode>,
}

impl AxNode {
    fn read(obj: &AnyObject, depth: usize) -> Self {
        let focused: Bool = unsafe { msg_send![obj, isAccessibilityFocused] };
        let role = string_attr(obj, "role").unwrap_or_default();
        let text_range = (role == "AXTextField" || role == "AXTextArea").then(|| unsafe {
            let count: isize = msg_send![obj, accessibilityNumberOfCharacters];
            let range: NSRange = msg_send![obj, accessibilitySelectedTextRange];
            (count, (range.location, range.length))
        });
        AxNode {
            text_range,
            role,
            subrole: string_attr(obj, "subrole"),
            label: string_attr(obj, "label").filter(|s| !s.is_empty()),
            title: string_attr(obj, "title").filter(|s| !s.is_empty()),
            value: string_attr(obj, "value"),
            focused: focused.as_bool(),
            children: if depth == 0 {
                Vec::new()
            } else {
                children_of(obj)
                    .iter()
                    .map(|c| AxNode::read(c, depth - 1))
                    .collect()
            },
        }
    }

    pub fn flatten(&self, out: &mut Vec<AxNode>) {
        out.push(AxNode {
            children: Vec::new(),
            ..self.clone()
        });
        for c in &self.children {
            c.flatten(out);
        }
    }

    pub fn short(&self) -> String {
        format!(
            "{}{} label={:?} value={:?}{}",
            self.role,
            self.subrole
                .as_ref()
                .map(|s| format!("/{s}"))
                .unwrap_or_default(),
            self.label.as_deref().or(self.title.as_deref()),
            self.value,
            if self.focused { " focused" } else { "" },
        ) + &self
            .text_range
            .map(|(n, r)| format!(" characters={n} selectedTextRange={r:?}"))
            .unwrap_or_default()
    }
}

// The main menu, as AppKit holds it.

pub struct MenuEntry {
    pub menu: String,
    pub title: String,
    pub key_equivalent: String,
    pub modifiers: String,
}

pub fn main_menu_entries() -> Vec<MenuEntry> {
    let app = NSApplication::sharedApplication(mtm());
    let mut out = Vec::new();
    let Some(main) = app.mainMenu() else {
        return out;
    };
    for i in 0..main.numberOfItems() {
        let Some(top) = main.itemAtIndex(i) else {
            continue;
        };
        let Some(sub) = top.submenu() else { continue };
        let menu_title = sub.title().to_string();
        for j in 0..sub.numberOfItems() {
            let Some(item) = sub.itemAtIndex(j) else {
                continue;
            };
            if item.isSeparatorItem() {
                continue;
            }
            let flags = item.keyEquivalentModifierMask();
            let mut modifiers = String::new();
            for (flag, name) in [
                (NSEventModifierFlags::Control, "ctrl-"),
                (NSEventModifierFlags::Option, "alt-"),
                (NSEventModifierFlags::Shift, "shift-"),
                (NSEventModifierFlags::Command, "cmd-"),
            ] {
                if flags.contains(flag) {
                    modifiers.push_str(name);
                }
            }
            out.push(MenuEntry {
                menu: menu_title.clone(),
                title: item.title().to_string(),
                key_equivalent: item.keyEquivalent().to_string(),
                modifiers,
            });
        }
    }
    out
}

/// Perform a menu item the way a click on it does. Returns false if it was not found.
pub fn perform_menu_item(menu_title: &str, item_title: &str) -> bool {
    let app = NSApplication::sharedApplication(mtm());
    let Some(main) = app.mainMenu() else {
        return false;
    };
    for i in 0..main.numberOfItems() {
        let Some(sub) = main.itemAtIndex(i).and_then(|t| t.submenu()) else {
            continue;
        };
        if sub.title().to_string() != menu_title {
            continue;
        }
        let index = sub.indexOfItemWithTitle(&NSString::from_str(item_title));
        if index >= 0 {
            sub.performActionForItemAtIndex(index);
            return true;
        }
    }
    false
}

/// Hand links to the application delegate the way LaunchServices does.
pub fn deliver_open_urls(urls: &[&str]) -> bool {
    let app = NSApplication::sharedApplication(mtm());
    let urls: Vec<Retained<NSURL>> = urls
        .iter()
        .filter_map(|u| NSURL::URLWithString(&NSString::from_str(u)))
        .collect();
    let array = NSArray::from_retained_slice(&urls);
    unsafe {
        let delegate: *mut AnyObject = msg_send![&*app, delegate];
        if delegate.is_null() {
            return false;
        }
        let _: () = msg_send![&*delegate, application: &*app, openURLs: &*array];
    }
    true
}

// The tray icon.

/// Send a synthetic left click (down, then up) to the tray icon's own view, the one tray-icon installs inside the
/// status item's button. Returns what was found on the way.
pub fn click_status_item(item: &NSStatusItem) -> Result<String, String> {
    let mtm = mtm();
    let button = item.button(mtm).ok_or("status item has no button")?;
    let subviews = button.subviews();
    let target = subviews
        .firstObject()
        .ok_or("status item button has no subview (tray-icon's target view)")?;
    let window = button.window().ok_or("status item button has no window")?;
    let frame = button.frame();
    let location = NSPoint::new(frame.size.width / 2.0, frame.size.height / 2.0);
    for (kind, down) in [
        (NSEventType::LeftMouseDown, true),
        (NSEventType::LeftMouseUp, false),
    ] {
        let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            kind,
            location,
            NSEventModifierFlags::empty(),
            0.0,
            window.windowNumber(),
            None,
            0,
            1,
            1.0,
        )
        .ok_or("could not create the mouse event")?;
        if down {
            target.mouseDown(&event);
        } else {
            target.mouseUp(&event);
        }
    }
    Ok(format!(
        "target view class {}, status window number {}",
        target.class().name().to_string_lossy(),
        window.windowNumber()
    ))
}

/// The scale factor of the status item's window, to turn tray-icon's physical rectangle into points.
pub fn status_item_scale(item: &NSStatusItem) -> Option<f64> {
    let button = item.button(mtm())?;
    button.window().map(|w| w.backingScaleFactor())
}

/// The display holding a point given in global points from the main display's top left (tray-icon's frame of
/// reference), and the point relative to that display's top left (GPUI's frame of reference for window bounds).
pub fn locate_on_display(x: f64, y_from_top: f64) -> Option<(u32, f64, f64)> {
    let screens = NSScreen::screens(mtm());
    let main_height = screens.firstObject()?.frame().size.height;
    // AppKit's global y grows upwards from the main display's bottom.
    let y = main_height - y_from_top;
    for screen in screens.iter() {
        let f = screen.frame();
        let inside_x = x >= f.origin.x && x < f.origin.x + f.size.width;
        let inside_y = y > f.origin.y && y <= f.origin.y + f.size.height;
        if inside_x && inside_y {
            let id: u32 = unsafe {
                let description: Retained<AnyObject> = msg_send![&*screen, deviceDescription];
                let key = NSString::from_str("NSScreenNumber");
                let number: Option<Retained<AnyObject>> =
                    msg_send![&*description, objectForKey: &*key];
                let number = number?;
                msg_send![&*number, unsignedIntValue]
            };
            return Some((id, x - f.origin.x, f.origin.y + f.size.height - y));
        }
    }
    None
}

/// What the status item looks like now: tray-icon's view inside the button, and whether its window is on screen.
pub fn status_item_state(item: &NSStatusItem) -> String {
    let mtm = mtm();
    let button = item.button(mtm);
    let subviews = button.as_ref().map_or(0, |b| b.subviews().count());
    let window_visible = button
        .as_ref()
        .and_then(|b| b.window())
        .is_some_and(|w| w.isVisible());
    format!("trayIconViews={subviews} statusWindowVisible={window_visible}")
}

/// A weak reference, to see whether AppKit lets the status item go once tray-icon removed it.
pub fn weak(item: &Retained<NSStatusItem>) -> objc2::rc::Weak<NSStatusItem> {
    objc2::rc::Weak::from_retained(item)
}
