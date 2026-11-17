//! The CEF helper for the browser trial (Task 12, decision 0012). Throwaway.
//!
//! One executable, two roles:
//!
//! - Started by `spike-app` or `spike-smoke` (no `--type=` switch), it is CEF's browser
//!   process. It owns CEF entirely: on macOS its own `NSApplication` subclass (see `mac.rs`),
//!   and CEF's own message loop (`CefRunMessageLoop`), so nothing has to be pumped from outside.
//!   It renders one visible windowless browser (plus hidden ones with `--tabs`), copies each
//!   `OnPaint` frame into a shared-memory slot, maps the forwarded input onto CEF's off-screen
//!   input calls, and relays DevTools calls through `SendDevToolsMessage` (no remote-debugging
//!   port, threat model P32).
//! - Started by Chromium with `--type=renderer`, `--type=gpu-process` and so on, it runs that
//!   sub-process and exits. On macOS those copies live in the helper app bundles Chromium
//!   requires (`<name> Helper (Renderer).app` and so on) and enter Chromium's sandbox first;
//!   elsewhere Chromium starts this same file again.
//!
//! Before either role does anything, `guard::check` stops a re-launch loop (see `guard.rs`).
//!
//! The entry point is [`run`]: `main.rs` calls it on macOS and Linux; on Windows CEF's
//! `bootstrapc.exe` (renamed `spike-cef-helper.exe`) loads `spike-cef-helper.dll` (the
//! `cef-helper-win` crate), which calls it with the sandbox information the bootstrap set up.
//!
//! The protocol is the stand-in's (`spike-protocol`). Arguments from the app:
//! `--shm-fd`/`--shm-handle`, `--shm-size`, `--profile-dir DIR` (the dedicated profile and
//! cache), `--start-url URL` (default `about:blank`), `--tabs N` (N - 1 hidden extra tabs,
//! for memory measurements), `--hidden-url URL` (what the hidden tabs load), and
//! `--probe-shared-texture` (asks CEF for shared textures instead of `OnPaint` buffers and
//! only logs what arrives; the app then gets no frames), `--frame-rate N` (CEF's windowless
//! frame rate, default 60).

use crate::{guard, keys};
use cef::args::Args;
use cef::*;
use spike_protocol::shm::SharedFrames;
use spike_protocol::view::{self as proto, CursorKind, FrameKind, InputEvent};
use spike_protocol::wire::{self, FromHelper, ToHelper};
use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const EVENTFLAG_SHIFT_DOWN: u32 = 1 << 1;
const EVENTFLAG_CONTROL_DOWN: u32 = 1 << 2;
const EVENTFLAG_ALT_DOWN: u32 = 1 << 3;
const EVENTFLAG_LEFT_MOUSE_BUTTON: u32 = 1 << 4;
const EVENTFLAG_MIDDLE_MOUSE_BUTTON: u32 = 1 << 5;
const EVENTFLAG_RIGHT_MOUSE_BUTTON: u32 = 1 << 6;
const EVENTFLAG_COMMAND_DOWN: u32 = 1 << 7;
const EVENTFLAG_IS_REPEAT: u32 = 1 << 13;
/// CEF's "no range" (`CefRange::InvalidRange()`). The range arguments of the input-method calls
/// must not be null: libcef ignores a call whose range pointer is null, silently.
const NO_RANGE: Range = Range {
    from: u32::MAX,
    to: u32::MAX,
};

/// The native cursor handle `OnCursorChange` passes, typed per platform by the bindings
/// (`cef_cursor_handle_t`). The helper reads only the cursor type.
#[cfg(target_os = "macos")]
type CursorHandle = *mut u8;
#[cfg(target_os = "linux")]
type CursorHandle = std::os::raw::c_ulong;
#[cfg(windows)]
type CursorHandle = sys::HCURSOR;

/// Whether a shared-texture frame carried a handle: an IOSurface on macOS, a D3D11 shared handle
/// on Windows, dmabuf planes on Linux (CEF's `cef_accelerated_paint_info_t` per platform).
#[cfg(target_os = "macos")]
fn shared_texture_present(info: &AcceleratedPaintInfo) -> bool {
    !info.shared_texture_io_surface.is_null()
}
#[cfg(windows)]
fn shared_texture_present(info: &AcceleratedPaintInfo) -> bool {
    !info.shared_texture_handle.is_null()
}
#[cfg(target_os = "linux")]
fn shared_texture_present(info: &AcceleratedPaintInfo) -> bool {
    info.plane_count > 0
}

/// State shared between CEF's UI thread and the thread reading the app's messages.
struct Shared {
    shm: SharedFrames,
    free: Vec<AtomicBool>,
    repaint_wanted: AtomicBool,
    out: Mutex<BufWriter<File>>,
    /// The view in logical pixels and the scale factor, written by the reader thread as soon as
    /// the app sends it, read by CEF whenever it asks for the view rectangle or screen info.
    view: Mutex<(i32, i32, f32)>,
    popup: Mutex<Option<proto::Rect>>,
    seq: AtomicU64,
    /// The visible browser's identifier; only its frames go to the app.
    main_id: AtomicU32,
    accelerated_logged: AtomicU32,
    /// The copy from CEF's buffer into shared memory: frames and nanoseconds since the last
    /// report.
    copies: AtomicU64,
    copy_ns: AtomicU64,
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

    fn status(&self, text: impl Into<String>) {
        let text = text.into();
        eprintln!("cef-helper: {text}");
        self.send(&FromHelper::Status { text });
    }
}

struct Options {
    start_url: String,
    tabs: usize,
    hidden_url: String,
    probe_shared_texture: bool,
    frame_rate: i32,
}

thread_local! {
    // CEF objects live on CEF's UI thread only. Index 0 is the visible browser.
    static BROWSERS: RefCell<Vec<Browser>> = const { RefCell::new(Vec::new()) };
    static DEVTOOLS: RefCell<Option<Registration>> = const { RefCell::new(None) };
    static COMPOSING: Cell<bool> = const { Cell::new(false) };
    /// Messages that arrived before the visible browser existed, replayed once it does.
    static PENDING: RefCell<Vec<ToHelper>> = const { RefCell::new(Vec::new()) };
    static CLOSING: Cell<bool> = const { Cell::new(false) };
}

/// Linux: the process's own `argc` and `argv` (see `main.rs`), given to CEF instead of the copy
/// `Args` makes, so Chromium's process titles reach `/proc/<pid>/cmdline`.
#[cfg(target_os = "linux")]
static PROCESS_ARGV: Mutex<Option<(i32, usize)>> = Mutex::new(None);

#[cfg(target_os = "linux")]
pub fn set_process_argv(argc: std::ffi::c_int, argv: *mut *mut std::ffi::c_char) {
    if argc > 0 && !argv.is_null() {
        *PROCESS_ARGV.lock().unwrap() = Some((argc, argv as usize));
    }
}

/// The arguments handed to CEF: the process's own on Linux when `main.rs` captured them and they
/// match what the standard library read, otherwise the copy `Args` made.
fn main_args(args: &Args, raw: &[String]) -> MainArgs {
    process_main_args(raw).unwrap_or_else(|| args.as_main_args().clone())
}

#[cfg(target_os = "linux")]
fn process_main_args(raw: &[String]) -> Option<MainArgs> {
    let (argc, argv) = (*PROCESS_ARGV.lock().unwrap())?;
    (argc as usize == raw.len()).then(|| MainArgs {
        argc,
        argv: argv as *mut *mut std::ffi::c_char,
    })
}

#[cfg(not(target_os = "linux"))]
fn process_main_args(_raw: &[String]) -> Option<MainArgs> {
    None
}

/// Runs this process in its role (CEF's browser process or one of its sub-processes) and
/// returns the exit code. `sandbox_info` is the Windows bootstrap's sandbox information, null
/// elsewhere.
pub fn run(sandbox_info: *mut u8) -> i32 {
    match run_inner(sandbox_info) {
        ExitCode::SUCCESS => 0,
        _ => 1,
    }
}

fn run_inner(sandbox_info: *mut u8) -> ExitCode {
    let raw: Vec<String> = std::env::args().collect();
    let is_subprocess = raw.iter().any(|a| a.starts_with("--type="));
    if let Err(reason) = guard::check(is_subprocess) {
        eprintln!("cef-helper: refusing to start: {reason}");
        return ExitCode::from(3);
    }
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
        let main_args = main_args(&args, &raw);
        let code = execute_process(Some(&main_args), None::<&mut App>, sandbox_info);
        return ExitCode::from(code.clamp(0, 255) as u8);
    }

    match run_browser_process(&raw, &args, sandbox_info) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cef-helper: error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

/// Switches the browser process refuses to start with: they turn Chromium's sandbox off or weaken
/// it (design 5.8), or open a debugging port any local process could drive (P32). A narrow deny
/// list; the evidence for the sandbox is the per-process check, and a real implementation builds
/// Chromium's command line from an allow list instead.
const REFUSED_SWITCHES: &[&str] = &[
    "no-sandbox",
    "disable-setuid-sandbox",
    "disable-namespace-sandbox",
    "disable-seccomp-filter-sandbox",
    "disable-gpu-sandbox",
    "single-process",
    "in-process-gpu",
    "remote-debugging-port",
    "remote-debugging-pipe",
    "remote-debugging-address",
];

/// The switch name in any spelling Chromium accepts (`--name`, `-name`, and `/name` on
/// Windows), without its `=value`.
fn switch_name(arg: &str) -> Option<&str> {
    let rest = arg
        .strip_prefix("--")
        .or_else(|| arg.strip_prefix('-'))
        .or_else(|| arg.strip_prefix('/').filter(|_| cfg!(windows)))?;
    rest.split('=').next()
}

fn run_browser_process(raw: &[String], args: &Args, sandbox_info: *mut u8) -> anyhow::Result<()> {
    if let Some(refused) = raw.iter().skip(1).find(|a| {
        switch_name(a).is_some_and(|name| {
            REFUSED_SWITCHES
                .iter()
                .any(|r| r.eq_ignore_ascii_case(name))
        })
    }) {
        anyhow::bail!("refusing to start with {refused}: it weakens the sandbox or opens a port");
    }
    let profile =
        arg(raw, "--profile-dir").ok_or_else(|| anyhow::anyhow!("missing --profile-dir"))?;
    let options = Options {
        start_url: arg(raw, "--start-url").unwrap_or("about:blank").to_string(),
        tabs: arg(raw, "--tabs")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1usize)
            .clamp(1, 8),
        hidden_url: arg(raw, "--hidden-url")
            .or(arg(raw, "--start-url"))
            .unwrap_or("about:blank")
            .to_string(),
        probe_shared_texture: raw.iter().any(|a| a == "--probe-shared-texture"),
        frame_rate: arg(raw, "--frame-rate")
            .and_then(|v| v.parse().ok())
            .unwrap_or(60i32)
            .clamp(1, 240),
    };
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
        main_id: AtomicU32::new(u32::MAX),
        accelerated_logged: AtomicU32::new(0),
        copies: AtomicU64::new(0),
        copy_ns: AtomicU64::new(0),
    });

    #[cfg(target_os = "macos")]
    crate::mac::install_application();

    let log_file = std::path::Path::new(profile).join("cef.log");
    // `--allow-background-network` keeps Chromium's default background fetches (to measure them).
    let allow_background_network = raw.iter().any(|a| a == "--allow-background-network");
    let mut app = HelperApp::new(shared.clone(), Arc::new(options), allow_background_network);
    let settings = Settings {
        windowless_rendering_enabled: 1,
        no_sandbox: 0,
        root_cache_path: CefString::from(profile),
        cache_path: CefString::from(profile),
        log_file: CefString::from(log_file.to_string_lossy().as_ref()),
        log_severity: LogSeverity::WARNING,
        // Never a debugging port (P32); 0 is CEF's "off".
        remote_debugging_port: 0,
        ..Default::default()
    };
    if cfg!(windows) {
        eprintln!(
            "cef-helper: Windows bootstrap sandbox information {}",
            if sandbox_info.is_null() {
                "absent"
            } else {
                "present"
            }
        );
    }
    let main_args = main_args(args, raw);
    if initialize(
        Some(&main_args),
        Some(&settings),
        Some(&mut app),
        sandbox_info,
    ) != 1
    {
        anyhow::bail!("CefInitialize failed");
    }

    let reader_shared = shared.clone();
    std::thread::spawn(move || read_app_messages(input, reader_shared));

    run_message_loop();
    BROWSERS.with_borrow_mut(|b| b.clear());
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
        // explicit handle list.
        use std::os::windows::io::AsHandle as _;
        let input = File::from(std::io::stdin().as_handle().try_clone_to_owned()?);
        let output = File::from(std::io::stdout().as_handle().try_clone_to_owned()?);
        Ok((input, output))
    }
}

/// The thread that reads the app's messages. Slot releases and the view size are handled here;
/// everything else is posted to CEF's UI thread, where CEF's objects live.
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
            ToHelper::Resize {
                width,
                height,
                scale_factor,
            } => {
                // Validated: the app is trusted, but the numbers size a shared-memory frame.
                let (max_w, max_h) = shared.shm.max_size();
                let scale = if scale_factor.is_finite() {
                    scale_factor.clamp(0.5, 4.0)
                } else {
                    1.0
                };
                let w = (width as f32).clamp(1.0, max_w as f32 / scale) as i32;
                let h = (height as f32).clamp(1.0, max_h as f32 / scale) as i32;
                *shared.view.lock().unwrap() = (w, h, scale);
                post(ToHelper::Resize {
                    width: w as u32,
                    height: h as u32,
                    scale_factor: scale,
                });
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

fn main_browser() -> Option<Browser> {
    BROWSERS.with_borrow(|b| b.first().cloned())
}

fn handle_on_ui_thread(message: ToHelper) {
    let browser = main_browser();
    if browser.is_none() && !matches!(message, ToHelper::Shutdown | ToHelper::Release { .. }) {
        // The browser is created once CEF's context is up; keep what arrives before.
        PENDING.with_borrow_mut(|p| p.push(message));
        return;
    }
    let host = browser.as_ref().and_then(|b| b.host());
    match message {
        ToHelper::Shutdown => close_all(),
        // A slot came free after a frame was dropped for lack of one: ask for a repaint.
        ToHelper::Release { .. } => {
            if let Some(host) = host {
                host.invalidate(PaintElementType::VIEW);
            }
        }
        ToHelper::Resize { .. } => {
            let Some(host) = host else { return };
            // The reader thread already stored the size; CEF reads it back from the handlers.
            host.notify_screen_info_changed();
            host.was_resized();
        }
        ToHelper::Input { event } => {
            if let (Some(host), Some(browser)) = (host, browser) {
                forward_input(&browser, &host, event);
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

fn close_all() {
    CLOSING.set(true);
    let browsers = BROWSERS.with_borrow(|b| b.clone());
    if browsers.is_empty() {
        quit_message_loop();
        return;
    }
    for browser in browsers {
        if let Some(host) = browser.host() {
            host.close_browser(1);
        }
    }
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

/// On macOS, Chromium leaves the edit shortcuts (Command-C and so on) to the application's
/// menu, so Blink's own key bindings do not handle them. An off-screen embedder has no menu for
/// the page, so the helper performs them on the focused frame. This is also the one place the
/// app could intercept clipboard access by a shortcut (P52).
fn edit_command(browser: &Browser, key: &str, modifiers: &proto::Modifiers) -> bool {
    if !cfg!(target_os = "macos") || !modifiers.platform || modifiers.control || modifiers.alt {
        return false;
    }
    let Some(frame) = browser.focused_frame().or_else(|| browser.main_frame()) else {
        return false;
    };
    match (key, modifiers.shift) {
        ("c", false) => frame.copy(),
        ("v", false) => frame.paste(),
        ("x", false) => frame.cut(),
        ("a", false) => frame.select_all(),
        ("z", false) => frame.undo(),
        ("z", true) => frame.redo(),
        _ => return false,
    }
    true
}

/// Maps one forwarded event onto CEF's off-screen input calls. Positions arrive in logical
/// pixels relative to the view, which is what CEF's `MouseEvent` takes.
fn forward_input(browser: &Browser, host: &BrowserHost, event: InputEvent) {
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
            host.send_mouse_click_event(Some(&e), kind, up as i32, click_count.max(1) as i32);
        }
        InputEvent::Wheel {
            x,
            y,
            delta_x,
            delta_y,
            unit,
            modifiers,
        } => {
            // CEF takes pixels; a line is about 40 pixels in Chromium. GPUI and CEF agree on the
            // sign: positive y moves the content down (towards the top of the page).
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
                native_key_code: codes.native_down,
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
            edit_command(browser, &key, &modifiers);
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
                native_key_code: codes.native_up,
                is_system_key: 0,
                character: 0,
                unmodified_character: 0,
                focus_on_editable_field: 0,
            };
            host.send_key_event(Some(&up));
        }
        InputEvent::Text { text } => {
            if COMPOSING.replace(false) {
                host.ime_commit_text(Some(&CefString::from(text.as_str())), Some(&NO_RANGE), 0);
            } else {
                // Control characters (Return, Tab) already went as CHAR events with their key.
                for unit in text.encode_utf16().filter(|u| *u >= 0x20) {
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
                from: selected_start.min(len),
                to: selected_end.min(len),
            };
            host.ime_set_composition(
                Some(&CefString::from(text.as_str())),
                Some(&[underline]),
                Some(&NO_RANGE),
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
        options: Arc<Options>,
        allow_background_network: bool,
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
            // Without these, the network service talks to Google's servers on its own (component
            // updates, field trials and similar background fetches) even while showing a local
            // file; the trial saw the connections with lsof. P52 wants the browser to resolve
            // only the names it is sent to.
            if !self.allow_background_network {
                for switch in ["disable-background-networking", "disable-component-update", "no-pings"] {
                    command_line.append_switch(Some(&switch.into()));
                }
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(HelperBrowserProcess::new(self.shared.clone(), self.options.clone()))
        }
    }
}

fn new_client(shared: &Arc<Shared>) -> Client {
    HelperClient::new(
        HelperRender::new(shared.clone()),
        HelperDisplay::new(shared.clone()),
        HelperLifeSpan::new(shared.clone()),
        HelperLoad::new(shared.clone()),
        HelperRequest::new(shared.clone()),
        refusal_set(shared.clone()),
    )
}

wrap_browser_process_handler! {
    struct HelperBrowserProcess {
        shared: Arc<Shared>,
        options: Arc<Options>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            let browser_settings = BrowserSettings {
                windowless_frame_rate: self.options.frame_rate,
                ..Default::default()
            };
            for tab in 0..self.options.tabs {
                let window_info = WindowInfo {
                    windowless_rendering_enabled: 1,
                    shared_texture_enabled: (tab == 0 && self.options.probe_shared_texture) as i32,
                    ..Default::default()
                };
                let url = if tab == 0 { &self.options.start_url } else { &self.options.hidden_url };
                let mut client = new_client(&self.shared);
                let created = browser_host_create_browser(
                    Some(&window_info),
                    Some(&mut client),
                    Some(&CefString::from(url.as_str())),
                    Some(&browser_settings),
                    None,
                    None,
                );
                if created != 1 {
                    eprintln!("cef-helper: could not create browser {tab}");
                    quit_message_loop();
                    return;
                }
            }
        }
    }
}

wrap_client! {
    struct HelperClient {
        render: RenderHandler,
        display: DisplayHandler,
        life_span: LifeSpanHandler,
        load: LoadHandler,
        request: RequestHandler,
        refusals: HelperRefusalSet,
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

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(self.load.clone())
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(self.request.clone())
        }

        fn dialog_handler(&self) -> Option<DialogHandler> {
            Some(self.refusals.dialog.clone())
        }

        fn download_handler(&self) -> Option<DownloadHandler> {
            Some(self.refusals.download.clone())
        }

        fn permission_handler(&self) -> Option<PermissionHandler> {
            Some(self.refusals.permission.clone())
        }

        fn jsdialog_handler(&self) -> Option<JsdialogHandler> {
            Some(self.refusals.jsdialog.clone())
        }
    }
}

/// The handlers that refuse what the trial does not support and report it to the app, which in
/// the real product would decide (P35): file choosers, downloads, permission prompts, page
/// dialogs.
#[derive(Clone)]
struct HelperRefusalSet {
    dialog: DialogHandler,
    download: DownloadHandler,
    permission: PermissionHandler,
    jsdialog: JsdialogHandler,
}

fn refusal_set(shared: Arc<Shared>) -> HelperRefusalSet {
    HelperRefusalSet {
        dialog: HelperDialog::new(shared.clone()),
        download: HelperDownload::new(shared.clone()),
        permission: HelperPermission::new(shared.clone()),
        jsdialog: HelperJsDialog::new(shared),
    }
}

wrap_dialog_handler! {
    struct HelperDialog {
        shared: Arc<Shared>,
    }

    impl DialogHandler {
        fn on_file_dialog(
            &self,
            _browser: Option<&mut Browser>,
            mode: FileDialogMode,
            _title: Option<&CefString>,
            _default_file_path: Option<&CefString>,
            _accept_filters: Option<&mut CefStringList>,
            _accept_extensions: Option<&mut CefStringList>,
            _accept_descriptions: Option<&mut CefStringList>,
            callback: Option<&mut FileDialogCallback>,
        ) -> ::std::os::raw::c_int {
            self.shared.status(format!("refused: file dialog (mode {mode:?})"));
            if let Some(callback) = callback {
                callback.cancel();
            }
            1
        }
    }
}

wrap_download_handler! {
    struct HelperDownload {
        shared: Arc<Shared>,
    }

    impl DownloadHandler {
        fn can_download(
            &self,
            _browser: Option<&mut Browser>,
            _url: Option<&CefString>,
            _request_method: Option<&CefString>,
        ) -> ::std::os::raw::c_int {
            self.shared.status("refused: download");
            0
        }
    }
}

wrap_permission_handler! {
    struct HelperPermission {
        shared: Arc<Shared>,
    }

    impl PermissionHandler {
        fn on_request_media_access_permission(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut MediaAccessCallback>,
        ) -> ::std::os::raw::c_int {
            self.shared.status(format!("refused: media access 0x{requested_permissions:x}"));
            if let Some(callback) = callback {
                callback.cancel();
            }
            1
        }

        fn on_show_permission_prompt(
            &self,
            _browser: Option<&mut Browser>,
            prompt_id: u64,
            _requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut PermissionPromptCallback>,
        ) -> ::std::os::raw::c_int {
            self.shared.status(format!(
                "refused: permission prompt {prompt_id} for 0x{requested_permissions:x}"
            ));
            if let Some(callback) = callback {
                callback.cont(PermissionRequestResult::DENY);
            }
            1
        }
    }
}

wrap_jsdialog_handler! {
    struct HelperJsDialog {
        shared: Arc<Shared>,
    }

    impl JsdialogHandler {
        fn on_jsdialog(
            &self,
            _browser: Option<&mut Browser>,
            _origin_url: Option<&CefString>,
            dialog_type: JsdialogType,
            _message_text: Option<&CefString>,
            _default_prompt_text: Option<&CefString>,
            _callback: Option<&mut JsdialogCallback>,
            suppress_message: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            self.shared.status(format!("suppressed: page dialog {dialog_type:?}"));
            if let Some(suppress) = suppress_message {
                *suppress = 1;
            }
            0
        }
    }
}

wrap_life_span_handler! {
    struct HelperLifeSpan {
        shared: Arc<Shared>,
    }

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: ::std::os::raw::c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            // A windowless browser's popup would otherwise become a native window.
            let url = target_url.map(|u| u.to_string()).unwrap_or_default();
            self.shared.status(format!("refused: popup window to {} characters of URL", url.len()));
            1
        }

        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else { return };
            let first = BROWSERS.with_borrow_mut(|b| {
                b.push(browser.clone());
                b.len() == 1
            });
            if !first {
                // An extra tab: a background tab, alive but not painting.
                if let Some(host) = browser.host() {
                    host.was_hidden(1);
                }
                return;
            }
            self.shared.main_id.store(browser.identifier() as u32, Ordering::Release);
            if let Some(host) = browser.host() {
                let mut observer = HelperDevTools::new(self.shared.clone());
                let registration = host.add_dev_tools_message_observer(Some(&mut observer));
                DEVTOOLS.with_borrow_mut(|d| *d = registration);
                host.was_resized();
            }
            let version = String::from_utf8_lossy(sys::CEF_VERSION);
            self.shared.send(&FromHelper::Ready {
                engine: format!("CEF {}", version.trim_end_matches('\0')),
            });
            for message in PENDING.with_borrow_mut(std::mem::take) {
                handle_on_ui_thread(message);
            }
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else { return };
            let id = browser.identifier();
            let empty = BROWSERS.with_borrow_mut(|b| {
                b.retain(|other| other.identifier() != id);
                b.is_empty()
            });
            if id as u32 == self.shared.main_id.load(Ordering::Acquire) {
                DEVTOOLS.with_borrow_mut(|d| *d = None);
            }
            if empty && CLOSING.get() {
                quit_message_loop();
            }
        }
    }
}

wrap_load_handler! {
    struct HelperLoad {
        shared: Arc<Shared>,
    }

    impl LoadHandler {
        fn on_load_end(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            http_status_code: ::std::os::raw::c_int,
        ) {
            let main = frame.is_some_and(|f| f.is_main() == 1);
            let id = browser.map(|b| b.identifier()).unwrap_or(-1);
            if main {
                self.shared.status(format!("load-end browser {id} status {http_status_code}"));
            }
        }

        fn on_load_error(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            error_code: Errorcode,
            _error_text: Option<&CefString>,
            _failed_url: Option<&CefString>,
        ) {
            let id = browser.map(|b| b.identifier()).unwrap_or(-1);
            self.shared.status(format!("load-error browser {id} {error_code:?}"));
        }
    }
}

wrap_request_handler! {
    struct HelperRequest {
        shared: Arc<Shared>,
    }

    impl RequestHandler {
        fn on_render_process_terminated(
            &self,
            browser: Option<&mut Browser>,
            status: TerminationStatus,
            error_code: ::std::os::raw::c_int,
            _error_string: Option<&CefString>,
        ) {
            self.shared.status(format!("renderer-terminated {status:?} code {error_code}; reloading"));
            // Recovery: a reload starts a fresh renderer for this browser.
            if let Some(browser) = browser
                && !CLOSING.get()
            {
                browser.reload();
            }
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
            browser: Option<&mut Browser>,
            _cursor: CursorHandle,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> ::std::os::raw::c_int {
            if !is_main(&self.shared, browser) {
                return 1;
            }
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

fn is_main(shared: &Shared, browser: Option<&mut Browser>) -> bool {
    browser.is_some_and(|b| b.identifier() as u32 == shared.main_id.load(Ordering::Acquire))
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

        fn on_popup_show(&self, browser: Option<&mut Browser>, show: ::std::os::raw::c_int) {
            if !is_main(&self.shared, browser) {
                return;
            }
            let rect = if show != 0 { *self.shared.popup.lock().unwrap() } else { None };
            self.shared.send(&FromHelper::Popup { show: show != 0, rect });
        }

        fn on_popup_size(&self, browser: Option<&mut Browser>, rect: Option<&Rect>) {
            if !is_main(&self.shared, browser) {
                return;
            }
            let Some(rect) = rect else { return };
            let rect = proto::Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
            *self.shared.popup.lock().unwrap() = Some(rect);
            self.shared.send(&FromHelper::Popup { show: true, rect: Some(rect) });
        }

        fn on_paint(
            &self,
            browser: Option<&mut Browser>,
            type_: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            if buffer.is_null() || width <= 0 || height <= 0 || !is_main(&self.shared, browser) {
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
            let started = std::time::Instant::now();
            target.copy_from_slice(source);
            let this_ns = started.elapsed().as_nanos() as u64;
            let ns = shared.copy_ns.fetch_add(this_ns, Ordering::Relaxed) + this_ns;
            let n = shared.copies.fetch_add(1, Ordering::Relaxed) + 1;
            if n == 300 {
                shared.copies.store(0, Ordering::Relaxed);
                shared.copy_ns.store(0, Ordering::Relaxed);
                shared.status(format!(
                    "helper copy into shared memory: {:.2} ms average over 300 frames of {w}x{h}",
                    ns as f64 / 300.0 / 1e6
                ));
            }
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

        fn on_accelerated_paint(
            &self,
            browser: Option<&mut Browser>,
            type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            info: Option<&AcceleratedPaintInfo>,
        ) {
            if !is_main(&self.shared, browser) {
                return;
            }
            // Probe only: report what a shared texture looks like, a few times.
            let n = self.shared.accelerated_logged.fetch_add(1, Ordering::Relaxed);
            if n < 5 || n.is_multiple_of(60) {
                let line = match info {
                    Some(info) => format!(
                        "shared-texture frame {n} type {type_:?} format {:?} handle_present={}",
                        info.format,
                        shared_texture_present(info),
                    ),
                    None => format!("shared-texture frame {n} without info"),
                };
                self.shared.status(line);
            }
        }

        fn on_ime_composition_range_changed(
            &self,
            browser: Option<&mut Browser>,
            selected_range: Option<&Range>,
            character_bounds: Option<&[Rect]>,
        ) {
            if !is_main(&self.shared, browser) {
                return;
            }
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
