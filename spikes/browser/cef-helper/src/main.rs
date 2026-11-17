//! The CEF helper for the browser trial (Task 12, decision 0012). Throwaway; phase B runs it.
//!
//! One executable, two roles:
//!
//! - Started by `spike-app` (no `--type=` switch), it is CEF's browser process. It owns CEF
//!   entirely: on macOS its own `NSApplication` subclass (see `mac.rs`), and CEF's own message
//!   loop (`CefRunMessageLoop`), so nothing has to be pumped from outside. It renders one
//!   windowless browser, copies each `OnPaint` frame into a shared-memory slot, maps the
//!   forwarded input onto CEF's off-screen input calls, and relays DevTools calls through
//!   `SendDevToolsMessage` (no remote-debugging port, threat model P32).
//! - Started by Chromium with `--type=renderer`, `--type=gpu-process` and so on, it runs that
//!   sub-process and exits. On macOS those copies live in the helper app bundles Chromium
//!   requires (`<name> Helper (Renderer).app` and so on); elsewhere Chromium starts this same
//!   file again.
//!
//! The protocol is the stand-in's (`spike-protocol`). Arguments from the app:
//! `--shm-fd`/`--shm-handle`, `--shm-size`, `--profile-dir DIR` (the dedicated profile).

mod keys;
#[cfg(target_os = "macos")]
mod mac;

use cef::args::Args;
use cef::*;
use spike_protocol::shm::SharedFrames;
use spike_protocol::view::{self as proto, CursorKind, FrameKind, InputEvent};
use spike_protocol::wire::{self, FromHelper, ToHelper};
use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const EVENTFLAG_SHIFT_DOWN: u32 = 1 << 1;
const EVENTFLAG_CONTROL_DOWN: u32 = 1 << 2;
const EVENTFLAG_ALT_DOWN: u32 = 1 << 3;
const EVENTFLAG_LEFT_MOUSE_BUTTON: u32 = 1 << 4;
const EVENTFLAG_MIDDLE_MOUSE_BUTTON: u32 = 1 << 5;
const EVENTFLAG_RIGHT_MOUSE_BUTTON: u32 = 1 << 6;
const EVENTFLAG_COMMAND_DOWN: u32 = 1 << 7;
const EVENTFLAG_IS_REPEAT: u32 = 1 << 13;

/// State shared between CEF's UI thread and the thread reading the app's messages.
struct Shared {
    shm: SharedFrames,
    free: Vec<AtomicBool>,
    repaint_wanted: AtomicBool,
    out: Mutex<BufWriter<File>>,
    view: Mutex<(i32, i32, f32)>,
    popup: Mutex<Option<proto::Rect>>,
    seq: AtomicU64,
}

impl Shared {
    fn send(&self, message: &FromHelper) {
        if let Ok(mut out) = self.out.lock()
            && wire::write_line(&mut *out, message).is_err()
        {
            eprintln!("cef-helper: the app stopped reading; quitting");
            quit_message_loop();
        }
    }
}

thread_local! {
    // CEF objects live on CEF's UI thread only.
    static BROWSER: RefCell<Option<Browser>> = const { RefCell::new(None) };
    static DEVTOOLS: RefCell<Option<Registration>> = const { RefCell::new(None) };
    static COMPOSING: Cell<bool> = const { Cell::new(false) };
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().collect();
    let is_subprocess = raw.iter().any(|a| a.starts_with("--type="));
    let args = Args::new();

    #[cfg(target_os = "macos")]
    let _sandbox = is_subprocess.then(|| {
        // Chromium's own sandbox for the sub-processes; it stays on (design 5.8).
        let mut sandbox = cef::sandbox::Sandbox::new();
        sandbox.initialize(args.as_main_args());
        sandbox
    });
    #[cfg(target_os = "macos")]
    let _loader = {
        let exe = std::env::current_exe().expect("own path");
        let loader = library_loader::LibraryLoader::new(&exe, is_subprocess);
        if !loader.load() {
            eprintln!(
                "cef-helper: could not load the CEF framework next to {}",
                exe.display()
            );
            return ExitCode::FAILURE;
        }
        loader
    };
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);

    if is_subprocess {
        let code = execute_process(
            Some(args.as_main_args()),
            None::<&mut App>,
            std::ptr::null_mut(),
        );
        return ExitCode::from(code.clamp(0, 255) as u8);
    }

    match run_browser_process(&raw, &args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cef-helper: error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_browser_process(raw: &[String], args: &Args) -> anyhow::Result<()> {
    // P32: a remote-debugging switch would open a port any local process could drive.
    if raw.iter().any(|a| {
        a.starts_with("--remote-debugging-port") || a.starts_with("--remote-debugging-pipe")
    }) {
        anyhow::bail!("refusing to start with a remote-debugging switch");
    }
    let profile =
        arg(raw, "--profile-dir").ok_or_else(|| anyhow::anyhow!("missing --profile-dir"))?;
    let shm = SharedFrames::open_inherited(raw)?;
    let (input, output) = take_protocol_pipes()?;

    let shared = Arc::new(Shared {
        free: (0..shm.slots()).map(|_| AtomicBool::new(true)).collect(),
        shm,
        repaint_wanted: AtomicBool::new(false),
        out: Mutex::new(BufWriter::new(output)),
        view: Mutex::new((800, 600, 1.0)),
        popup: Mutex::new(None),
        seq: AtomicU64::new(0),
    });

    #[cfg(target_os = "macos")]
    mac::install_application();

    let mut app = HelperApp::new(shared.clone());
    let settings = Settings {
        windowless_rendering_enabled: 1,
        no_sandbox: 0,
        root_cache_path: CefString::from(profile),
        cache_path: CefString::from(profile),
        ..Default::default()
    };
    if initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(&mut app),
        std::ptr::null_mut(),
    ) != 1
    {
        anyhow::bail!("CefInitialize failed");
    }

    let reader_shared = shared.clone();
    std::thread::spawn(move || read_app_messages(input, reader_shared));

    run_message_loop();
    BROWSER.with_borrow_mut(|b| *b = None);
    DEVTOOLS.with_borrow_mut(|d| *d = None);
    shutdown();
    eprintln!("cef-helper: exiting");
    Ok(())
}

/// Keeps the protocol pipes for this process only. On Unix, file descriptors 0 and 1 are then
/// pointed at /dev/null and stderr, so the sub-processes Chromium starts can never inherit (and
/// write into) the protocol channel.
fn take_protocol_pipes() -> anyhow::Result<(File, File)> {
    #[cfg(unix)]
    {
        use std::os::fd::AsFd as _;
        let input = File::from(std::io::stdin().as_fd().try_clone_to_owned()?);
        let output = File::from(std::io::stdout().as_fd().try_clone_to_owned()?);
        // SAFETY: plain descriptor juggling on the process's own standard streams.
        unsafe {
            let null = libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY);
            if null >= 0 {
                libc::dup2(null, 0);
                libc::close(null);
            }
            libc::dup2(2, 1);
            // The shared memory region must not reach sub-processes either.
            libc::fcntl(3, libc::F_SETFD, libc::FD_CLOEXEC);
        }
        Ok((input, output))
    }
    #[cfg(windows)]
    {
        // Duplicated handles are not inheritable; Chromium starts its sub-processes with an
        // explicit handle list. Whether a stray inheritable standard handle still reaches them
        // is a phase B check.
        use std::os::windows::io::AsHandle as _;
        let input = File::from(std::io::stdin().as_handle().try_clone_to_owned()?);
        let output = File::from(std::io::stdout().as_handle().try_clone_to_owned()?);
        Ok((input, output))
    }
}

/// The thread that reads the app's messages. Slot releases are handled here; everything else
/// is posted to CEF's UI thread, where CEF's objects live.
fn read_app_messages(input: File, shared: Arc<Shared>) {
    let mut input = BufReader::new(input);
    let mut buffer = Vec::new();
    loop {
        let message = match wire::read_line::<ToHelper>(&mut input, &mut buffer) {
            Ok(Some(message)) => message,
            Ok(None) => ToHelper::Shutdown,
            Err(error) => {
                eprintln!("cef-helper: bad message from the app: {error}");
                ToHelper::Shutdown
            }
        };
        let last = matches!(message, ToHelper::Shutdown);
        match message {
            ToHelper::Release { slot } => {
                if let Some(free) = shared.free.get(slot as usize) {
                    free.store(true, Ordering::Release);
                }
                if shared.repaint_wanted.swap(false, Ordering::AcqRel) {
                    post(ToHelper::Release { slot });
                }
            }
            other => post(other),
        }
        if last {
            break;
        }
    }
}

fn post(message: ToHelper) {
    let mut task = MessageTask::new(message);
    post_task(ThreadId::UI, Some(&mut task));
}

wrap_task! {
    struct MessageTask {
        message: ToHelper,
    }

    impl Task {
        fn execute(&self) {
            handle_on_ui_thread(self.message.clone());
        }
    }
}

fn handle_on_ui_thread(message: ToHelper) {
    let host = BROWSER.with_borrow(|b| b.as_ref().and_then(|b| b.host()));
    match message {
        ToHelper::Shutdown => match host {
            Some(host) => host.close_browser(1),
            None => quit_message_loop(),
        },
        // A slot came free after a frame was dropped for lack of one: ask for a repaint.
        ToHelper::Release { .. } => {
            if let Some(host) = host {
                host.invalidate(PaintElementType::VIEW);
            }
        }
        ToHelper::Resize {
            width,
            height,
            scale_factor,
        } => {
            let Some(host) = host else { return };
            let changed_scale = {
                let shared = SHARED.with_borrow(|s| s.clone());
                let Some(shared) = shared else { return };
                let mut view = shared.view.lock().unwrap();
                let changed = (view.2 - scale_factor).abs() > f32::EPSILON;
                *view = (width as i32, height as i32, scale_factor);
                changed
            };
            if changed_scale {
                host.notify_screen_info_changed();
            }
            host.was_resized();
        }
        ToHelper::Input { event } => {
            if let Some(host) = host {
                forward_input(&host, event);
            }
        }
        ToHelper::DevTools { id, method, params } => {
            let Some(host) = host else { return };
            let message = serde_json::json!({ "id": id, "method": method, "params": params });
            let bytes = message.to_string().into_bytes();
            if host.send_dev_tools_message(Some(&bytes)) != 1 {
                eprintln!("cef-helper: DevTools call {id} ({method}) was not accepted");
            }
        }
    }
}

thread_local! {
    static SHARED: RefCell<Option<Arc<Shared>>> = const { RefCell::new(None) };
}

fn modifier_flags(m: &proto::Modifiers) -> u32 {
    let mut flags = 0;
    if m.shift {
        flags |= EVENTFLAG_SHIFT_DOWN;
    }
    if m.control {
        flags |= EVENTFLAG_CONTROL_DOWN;
    }
    if m.alt {
        flags |= EVENTFLAG_ALT_DOWN;
    }
    if m.platform {
        flags |= EVENTFLAG_COMMAND_DOWN;
    }
    flags
}

fn button_flag(button: Option<proto::PointerButton>) -> u32 {
    match button {
        Some(proto::PointerButton::Left) => EVENTFLAG_LEFT_MOUSE_BUTTON,
        Some(proto::PointerButton::Middle) => EVENTFLAG_MIDDLE_MOUSE_BUTTON,
        Some(proto::PointerButton::Right) => EVENTFLAG_RIGHT_MOUSE_BUTTON,
        _ => 0,
    }
}

fn mouse_button(button: proto::PointerButton) -> Option<MouseButtonType> {
    match button {
        proto::PointerButton::Left => Some(MouseButtonType::LEFT),
        proto::PointerButton::Middle => Some(MouseButtonType::MIDDLE),
        proto::PointerButton::Right => Some(MouseButtonType::RIGHT),
        // Back and forward buttons have no off-screen click type; they would map to history
        // navigation over DevTools.
        _ => None,
    }
}

/// Maps one forwarded event onto CEF's off-screen input calls. Positions arrive in logical
/// pixels relative to the view, which is what CEF's `MouseEvent` takes.
fn forward_input(host: &BrowserHost, event: InputEvent) {
    match event {
        InputEvent::PointerMove {
            x,
            y,
            pressed,
            modifiers,
        } => {
            let e = MouseEvent {
                x: x as i32,
                y: y as i32,
                modifiers: modifier_flags(&modifiers) | button_flag(pressed),
            };
            host.send_mouse_move_event(Some(&e), 0);
        }
        InputEvent::PointerLeave { modifiers } => {
            let e = MouseEvent {
                x: 0,
                y: 0,
                modifiers: modifier_flags(&modifiers),
            };
            host.send_mouse_move_event(Some(&e), 1);
        }
        InputEvent::PointerDown {
            x,
            y,
            button,
            click_count,
            modifiers,
        }
        | InputEvent::PointerUp {
            x,
            y,
            button,
            click_count,
            modifiers,
        } => {
            let up = matches!(event, InputEvent::PointerUp { .. });
            let Some(kind) = mouse_button(button) else {
                return;
            };
            let e = MouseEvent {
                x: x as i32,
                y: y as i32,
                modifiers: modifier_flags(&modifiers)
                    | if up { 0 } else { button_flag(Some(button)) },
            };
            host.send_mouse_click_event(Some(&e), kind, up as i32, click_count as i32);
        }
        InputEvent::Wheel {
            x,
            y,
            delta_x,
            delta_y,
            unit,
            modifiers,
        } => {
            // CEF takes pixels; a line is about 40 pixels in Chromium. The sign convention
            // (positive y scrolls towards the top) is checked by hand in phase B.
            let scale = match unit {
                proto::WheelUnit::Pixels => 1.0,
                proto::WheelUnit::Lines => 40.0,
            };
            let e = MouseEvent {
                x: x as i32,
                y: y as i32,
                modifiers: modifier_flags(&modifiers),
            };
            host.send_mouse_wheel_event(
                Some(&e),
                (delta_x * scale) as i32,
                (delta_y * scale) as i32,
            );
        }
        InputEvent::KeyDown {
            key,
            key_char,
            modifiers,
            is_held,
        } => {
            let Some(codes) = keys::key_codes(&key) else {
                eprintln!("cef-helper: no key code for {key:?}");
                return;
            };
            let character = key_char
                .as_deref()
                .and_then(|c| c.encode_utf16().next())
                .unwrap_or(0);
            let mut flags = modifier_flags(&modifiers);
            if is_held {
                flags |= EVENTFLAG_IS_REPEAT;
            }
            let down = KeyEvent {
                size: size_of::<KeyEvent>(),
                type_: KeyEventType::RAWKEYDOWN,
                modifiers: flags,
                windows_key_code: codes.windows,
                native_key_code: codes.mac,
                is_system_key: 0,
                character,
                unmodified_character: character,
                focus_on_editable_field: 0,
            };
            host.send_key_event(Some(&down));
            if let Some(c) = codes.char_event {
                let char_event = KeyEvent {
                    type_: KeyEventType::CHAR,
                    windows_key_code: c as i32,
                    character: c,
                    unmodified_character: c,
                    ..down
                };
                host.send_key_event(Some(&char_event));
            }
        }
        InputEvent::KeyUp { key, modifiers } => {
            let Some(codes) = keys::key_codes(&key) else {
                return;
            };
            let up = KeyEvent {
                size: size_of::<KeyEvent>(),
                type_: KeyEventType::KEYUP,
                modifiers: modifier_flags(&modifiers),
                windows_key_code: codes.windows,
                native_key_code: codes.mac,
                is_system_key: 0,
                character: 0,
                unmodified_character: 0,
                focus_on_editable_field: 0,
            };
            host.send_key_event(Some(&up));
        }
        InputEvent::Text { text } => {
            if COMPOSING.replace(false) {
                host.ime_commit_text(Some(&CefString::from(text.as_str())), None, 0);
            } else {
                for unit in text.encode_utf16() {
                    let e = KeyEvent {
                        size: size_of::<KeyEvent>(),
                        type_: KeyEventType::CHAR,
                        modifiers: 0,
                        windows_key_code: unit as i32,
                        native_key_code: 0,
                        is_system_key: 0,
                        character: unit,
                        unmodified_character: unit,
                        focus_on_editable_field: 0,
                    };
                    host.send_key_event(Some(&e));
                }
            }
        }
        InputEvent::ImeSetComposition {
            text,
            selected_start,
            selected_end,
        } => {
            COMPOSING.set(true);
            let len = text.encode_utf16().count() as u32;
            let underline = CompositionUnderline {
                size: size_of::<CompositionUnderline>(),
                range: Range { from: 0, to: len },
                color: 0xFF00_0000,
                background_color: 0,
                thick: 0,
                style: Default::default(),
            };
            let selection = Range {
                from: selected_start,
                to: selected_end,
            };
            host.ime_set_composition(
                Some(&CefString::from(text.as_str())),
                Some(&[underline]),
                None,
                Some(&selection),
            );
        }
        InputEvent::ImeFinish => {
            COMPOSING.set(false);
            host.ime_finish_composing_text(0);
        }
        InputEvent::ImeCancel => {
            COMPOSING.set(false);
            host.ime_cancel_composition();
        }
        InputEvent::Focus { focused } => host.set_focus(focused as i32),
    }
}

wrap_app! {
    struct HelperApp {
        shared: Arc<Shared>,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefStringUtf16>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(command_line) = command_line else { return };
            // No keychain prompt for the dedicated profile on macOS.
            command_line.append_switch(Some(&"use-mock-keychain".into()));
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(HelperBrowserProcess::new(self.shared.clone()))
        }
    }
}

wrap_browser_process_handler! {
    struct HelperBrowserProcess {
        shared: Arc<Shared>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            SHARED.with_borrow_mut(|s| *s = Some(self.shared.clone()));
            let window_info = WindowInfo {
                windowless_rendering_enabled: 1,
                ..Default::default()
            };
            let browser_settings = BrowserSettings {
                windowless_frame_rate: 60,
                ..Default::default()
            };
            let mut client = HelperClient::new(
                HelperRender::new(self.shared.clone()),
                HelperDisplay::new(self.shared.clone()),
                HelperLifeSpan::new(self.shared.clone()),
            );
            let created = browser_host_create_browser(
                Some(&window_info),
                Some(&mut client),
                Some(&CefString::from("about:blank")),
                Some(&browser_settings),
                None,
                None,
            );
            if created != 1 {
                eprintln!("cef-helper: could not create the browser");
                quit_message_loop();
            }
        }
    }
}

wrap_client! {
    struct HelperClient {
        render: RenderHandler,
        display: DisplayHandler,
        life_span: LifeSpanHandler,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render.clone())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.display.clone())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life_span.clone())
        }
    }
}

wrap_life_span_handler! {
    struct HelperLifeSpan {
        shared: Arc<Shared>,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else { return };
            if let Some(host) = browser.host() {
                let mut observer = HelperDevTools::new(self.shared.clone());
                let registration = host.add_dev_tools_message_observer(Some(&mut observer));
                DEVTOOLS.with_borrow_mut(|d| *d = registration);
            }
            BROWSER.with_borrow_mut(|b| *b = Some(browser.clone()));
            let version = String::from_utf8_lossy(sys::CEF_VERSION);
            self.shared.send(&FromHelper::Ready {
                engine: format!("CEF {}", version.trim_end_matches('\0')),
            });
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            DEVTOOLS.with_borrow_mut(|d| *d = None);
            BROWSER.with_borrow_mut(|b| *b = None);
            quit_message_loop();
        }
    }
}

wrap_dev_tools_message_observer! {
    struct HelperDevTools {
        shared: Arc<Shared>,
    }

    impl DevToolsMessageObserver {
        fn on_dev_tools_message(&self, _browser: Option<&mut Browser>, message: Option<&[u8]>) -> ::std::os::raw::c_int {
            let Some(message) = message else { return 1 };
            match serde_json::from_slice::<serde_json::Value>(message) {
                Ok(message) => self.shared.send(&FromHelper::DevTools { message }),
                Err(error) => eprintln!("cef-helper: unreadable DevTools message: {error}"),
            }
            1
        }
    }
}

wrap_display_handler! {
    struct HelperDisplay {
        shared: Arc<Shared>,
    }

    impl DisplayHandler {
        fn on_cursor_change(
            &self,
            _browser: Option<&mut Browser>,
            _cursor: *mut u8,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> ::std::os::raw::c_int {
            let cursor = match type_ {
                CursorType::HAND => CursorKind::PointingHand,
                CursorType::IBEAM => CursorKind::IBeam,
                CursorType::CROSS => CursorKind::Crosshair,
                CursorType::GRAB => CursorKind::Grab,
                CursorType::GRABBING => CursorKind::Grabbing,
                CursorType::EASTWESTRESIZE => CursorKind::ResizeLeftRight,
                CursorType::NORTHSOUTHRESIZE => CursorKind::ResizeUpDown,
                CursorType::NOTALLOWED => CursorKind::NotAllowed,
                CursorType::NONE => CursorKind::Hidden,
                _ => CursorKind::Arrow,
            };
            self.shared.send(&FromHelper::Cursor { cursor });
            1
        }
    }
}

wrap_render_handler! {
    struct HelperRender {
        shared: Arc<Shared>,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            let Some(rect) = rect else { return };
            let (w, h, _) = *self.shared.view.lock().unwrap();
            *rect = Rect { x: 0, y: 0, width: w.max(1), height: h.max(1) };
        }

        fn screen_info(&self, _browser: Option<&mut Browser>, screen_info: Option<&mut ScreenInfo>) -> ::std::os::raw::c_int {
            let Some(info) = screen_info else { return 0 };
            let (w, h, scale) = *self.shared.view.lock().unwrap();
            let rect = Rect { x: 0, y: 0, width: w.max(1), height: h.max(1) };
            info.device_scale_factor = scale;
            info.rect = rect.clone();
            info.available_rect = rect;
            1
        }

        fn on_popup_show(&self, _browser: Option<&mut Browser>, show: ::std::os::raw::c_int) {
            let rect = if show != 0 { *self.shared.popup.lock().unwrap() } else { None };
            self.shared.send(&FromHelper::Popup { show: show != 0, rect });
        }

        fn on_popup_size(&self, _browser: Option<&mut Browser>, rect: Option<&Rect>) {
            let Some(rect) = rect else { return };
            let rect = proto::Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
            *self.shared.popup.lock().unwrap() = Some(rect);
            self.shared.send(&FromHelper::Popup { show: true, rect: Some(rect) });
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let shared = &self.shared;
            let Some(slot) = shared
                .free
                .iter()
                .position(|f| f.compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed).is_ok())
            else {
                // The app still holds every slot. CEF's buffer is only valid during this call,
                // so drop the frame and ask for a repaint when a slot comes back.
                shared.repaint_wanted.store(true, Ordering::Release);
                return;
            };
            let (w, h) = (width as u32, height as u32);
            // SAFETY: the slot was free and is now ours until the app releases it.
            let target = match unsafe { shared.shm.slot_mut(slot as u32, w, h) } {
                Ok(target) => target,
                Err(error) => {
                    eprintln!("cef-helper: frame does not fit: {error:#}");
                    shared.free[slot].store(true, Ordering::Release);
                    return;
                }
            };
            // SAFETY: CEF hands over width * height * 4 bytes, valid for this call.
            let source = unsafe { std::slice::from_raw_parts(buffer, target.len()) };
            target.copy_from_slice(source);
            let kind = if type_ == PaintElementType::POPUP { FrameKind::Popup } else { FrameKind::View };
            let dirty = dirty_rects
                .unwrap_or_default()
                .iter()
                .map(|r| proto::Rect { x: r.x, y: r.y, width: r.width, height: r.height })
                .collect();
            let seq = shared.seq.fetch_add(1, Ordering::Relaxed) + 1;
            shared.send(&FromHelper::Frame {
                slot: slot as u32,
                kind,
                seq,
                width: w,
                height: h,
                dirty,
                produced_unix_ns: wire::unix_ns(),
            });
        }

        fn on_ime_composition_range_changed(
            &self,
            _browser: Option<&mut Browser>,
            selected_range: Option<&Range>,
            character_bounds: Option<&[Rect]>,
        ) {
            let (range_start, range_end) = selected_range.map_or((0, 0), |r| (r.from, r.to));
            let characters = character_bounds
                .unwrap_or_default()
                .iter()
                .map(|r| proto::Rect { x: r.x, y: r.y, width: r.width, height: r.height })
                .collect();
            self.shared.send(&FromHelper::ImeBounds { range_start, range_end, characters });
        }
    }
}

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}
