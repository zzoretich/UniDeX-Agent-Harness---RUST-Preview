//! The tray icon, through tray-icon 0.26.1 (decision 0011), beside GPUI's event loop.
//!
//! - macOS: an `NSStatusItem`. It must be created on the main thread after the application finished launching,
//!   which is when GPUI's `run` callback runs. AppKit delivers its mouse events through the same `NSApplication`
//!   run loop GPUI uses, so nothing else is needed.
//! - Windows: `Shell_NotifyIcon` with a hidden message window. Created on GPUI's main thread, its messages are
//!   dispatched by GPUI's own `GetMessageW`/`DispatchMessageW` loop, which serves every window of the thread.
//! - Linux, AppIndicator backend (default): GTK 3 needs its own main loop, which GPUI does not run, so the icon lives
//!   on a separate thread that runs `gtk::main()`. This backend emits no click events at all (tray-icon documents
//!   it); only the icon's menu works. Linux, KSNI backend (`--features tray-ksni`): a D-Bus StatusNotifierItem on a
//!   worker thread; it emits left-click activation, without the icon's position.
//!
//! Events reach GPUI without polling: tray-icon's handlers push into an `async_channel`, and a GPUI foreground
//! task awaits it.

use tray_icon::{
    Icon, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

/// What the GPUI side receives from the tray.
#[derive(Debug, Clone)]
pub enum TrayMessage {
    /// A click on the icon: button, whether it was the release, and the icon's rectangle in physical pixels
    /// (x, y from the top-left of the main display, width, height), if the platform reports it.
    Click {
        button: &'static str,
        up: bool,
        rect: Option<(f64, f64, f64, f64)>,
    },
    /// An item of the icon's own menu was chosen.
    Menu(String),
}

pub const MENU_SHOW_POPOVER: &str = "trial-show-popover";
pub const MENU_QUIT: &str = "trial-quit";

/// A 32x32 icon drawn here: a filled circle, so no image file or decoder is needed.
fn icon() -> Icon {
    let size = 32u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - 15.5;
            let dy = y as f32 - 15.5;
            let inside = dx * dx + dy * dy <= 13.0 * 13.0;
            rgba.extend_from_slice(if inside {
                &[37, 99, 235, 255]
            } else {
                &[0, 0, 0, 0]
            });
        }
    }
    Icon::from_rgba(rgba, size, size).expect("valid icon")
}

fn menu() -> Menu {
    let menu = Menu::new();
    let _ = menu.append(&MenuItem::with_id(
        MENU_SHOW_POPOVER,
        "Show popover",
        true,
        None,
    ));
    let _ = menu.append(&MenuItem::with_id(MENU_QUIT, "Quit the trial", true, None));
    menu
}

/// Route tray-icon's event handlers into a channel GPUI awaits. Call once per process.
pub fn forward_events(tx: async_channel::Sender<TrayMessage>) {
    let click_tx = tx.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            rect,
            button,
            button_state,
            ..
        } = event
        {
            let rect = (rect.size.width > 0).then_some((
                rect.position.x,
                rect.position.y,
                rect.size.width as f64,
                rect.size.height as f64,
            ));
            let _ = click_tx.try_send(TrayMessage::Click {
                button: match button {
                    tray_icon::MouseButton::Left => "left",
                    tray_icon::MouseButton::Right => "right",
                    tray_icon::MouseButton::Middle => "middle",
                },
                up: button_state == tray_icon::MouseButtonState::Up,
                rect,
            });
        }
    }));
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = tx.try_send(TrayMessage::Menu(event.id().0.clone()));
    }));
}

/// The icon, owned by whichever thread its platform needs.
pub enum Tray {
    /// macOS, Windows and the KSNI backend: owned by GPUI's main thread.
    Local(tray_icon::TrayIcon),
    /// Linux AppIndicator backend: owned by the GTK thread; dropping this asks that thread to remove it and stop.
    #[cfg(all(
        target_os = "linux",
        feature = "tray-appindicator",
        not(feature = "tray-ksni")
    ))]
    Gtk(gtk_thread::Handle),
}

impl Tray {
    /// Create the icon. Left click is left to the app (it opens the popover); right click shows the menu.
    pub fn create() -> Result<Tray, String> {
        #[cfg(all(
            target_os = "linux",
            feature = "tray-appindicator",
            not(feature = "tray-ksni")
        ))]
        {
            gtk_thread::start().map(Tray::Gtk)
        }
        #[cfg(not(all(
            target_os = "linux",
            feature = "tray-appindicator",
            not(feature = "tray-ksni")
        )))]
        {
            tray_icon::TrayIconBuilder::new()
                .with_id("unidex-trial")
                .with_icon(icon())
                .with_tooltip("UniDeX platform trial")
                .with_menu(Box::new(menu()))
                .with_menu_on_left_click(false)
                .with_menu_on_right_click(true)
                .build()
                .map(Tray::Local)
                .map_err(|e| e.to_string())
        }
    }

    pub fn local(&self) -> Option<&tray_icon::TrayIcon> {
        match self {
            Tray::Local(t) => Some(t),
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// The icon's rectangle in physical pixels, where the platform reports it (macOS and Windows).
    pub fn rect(&self) -> Option<(f64, f64, f64, f64)> {
        let r = self.local()?.rect()?;
        Some((
            r.position.x,
            r.position.y,
            r.size.width as f64,
            r.size.height as f64,
        ))
    }
}

#[cfg(all(
    target_os = "linux",
    feature = "tray-appindicator",
    not(feature = "tray-ksni")
))]
mod gtk_thread {
    //! GTK 3 on its own thread: `gtk::init`, the icon, then `gtk::main()` until GPUI drops the handle.

    use gtk::glib;

    pub struct Handle {
        stop: async_channel::Sender<()>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            let _ = self.stop.try_send(());
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    pub fn start() -> Result<Handle, String> {
        let (stop, stopped) = async_channel::bounded::<()>(1);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let thread = std::thread::Builder::new()
            .name("gtk-tray".into())
            .spawn(move || {
                if let Err(e) = gtk::init() {
                    let _ = ready_tx.send(Err(format!("gtk::init failed: {e}")));
                    return;
                }
                let tray = tray_icon::TrayIconBuilder::new()
                    .with_id("unidex-trial")
                    .with_icon(super::icon())
                    .with_tooltip("UniDeX platform trial")
                    .with_menu(Box::new(super::menu()))
                    .build();
                let tray = match tray {
                    Ok(t) => t,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let context = glib::MainContext::default();
                let _guard = context.acquire();
                context.spawn_local(async move {
                    let _ = stopped.recv().await;
                    drop(tray);
                    gtk::main_quit();
                });
                let _ = ready_tx.send(Ok(()));
                gtk::main();
            })
            .map_err(|e| e.to_string())?;
        ready_rx.recv().map_err(|e| e.to_string())??;
        Ok(Handle {
            stop,
            thread: Some(thread),
        })
    }
}
