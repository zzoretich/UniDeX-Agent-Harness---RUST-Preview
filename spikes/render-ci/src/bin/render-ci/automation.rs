//! The automation socket (design 10.3), a sketch shaped by the threat model.
//!
//! - Compiled only with the `automation` feature; a release build is made without it. `MARKER` is printed at start so
//!   it is in every binary that contains this module, and CI checks that the release binary lacks it.
//! - Off unless the app is started with `--automation`.
//! - Local only, never a TCP port. Unix: a socket file inside a fresh directory created with mode 0700 (creation fails
//!   if the name exists, so a planted directory is never reused), the socket itself chmod 0600, and every connection's
//!   peer user id compared with the app's own (SO_PEERCRED on Linux, getpeereid on macOS). Windows: a named pipe whose
//!   DACL grants access to the current user's SID only (protected, so nothing is inherited), created with
//!   FILE_FLAG_FIRST_PIPE_INSTANCE (fails if another process already owns the name) and PIPE_REJECT_REMOTE_CLIENTS.
//! - One client at a time. One request per line, at most `MAX_REQUEST` bytes, a fixed set of verbs; anything else is
//!   refused. No verb takes a path or runs anything: a frame comes back inline as base64 PNG.
//!
//! Verbs: hello, state, tree, a11y_activate, click {id}, frame, quit.

use crate::{TrialView, capture, platform_json, window_json};
use base64::Engine as _;
use gpui::{
    AnyWindowHandle, App, AppContext as _, Entity, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PlatformInput, point,
};
use render_ci::{MAX_REQUEST, read_line_limited};
use serde_json::{Value, json};
use std::io::{BufReader, Write};
use std::time::{Duration, Instant};

/// Present in a binary exactly when it contains the automation socket.
pub const MARKER: &str = "udx-automation-socket-v1";
const PROTOCOL: u32 = 1;

type Job = (Value, std::sync::mpsc::Sender<Value>);

pub fn start(cx: &mut App, window: AnyWindowHandle, view: Entity<TrialView>, start: Instant) {
    let endpoint = match endpoint::Endpoint::create() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("render-ci: could not open the automation endpoint: {e:#}");
            std::process::exit(6);
        }
    };
    log::info!("{MARKER}: listening at {}", endpoint.display());
    // The client reads this line; nothing else is printed on stdout in automation mode.
    println!("RENDER_CI_ENDPOINT={}", endpoint.display());
    let _ = std::io::stdout().flush();

    // The native window handle, for Windows' accessibility activation (done from the server thread).
    let native = cx
        .update_window(window, |_, window, _| native_handle(window))
        .unwrap_or(0);

    let (tx, rx) = async_channel::unbounded::<Job>();
    std::thread::Builder::new()
        .name("automation".into())
        .spawn(move || server(endpoint, tx, native))
        .expect("spawn the automation thread");

    cx.spawn(async move |cx| {
        while let Ok((request, reply)) = rx.recv().await {
            let verb = request["cmd"].as_str().unwrap_or("").to_string();
            let response = cx.update(|cx| handle(&request, window, &view, cx, start));
            let _ = reply.send(response);
            if verb == "quit" {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                log::info!("quit requested over the automation socket");
                std::process::exit(0);
            }
        }
    })
    .detach();
}

fn native_handle(window: &gpui::Window) -> isize {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
        Ok(RawWindowHandle::AppKit(h)) => h.ns_view.as_ptr() as isize,
        Ok(RawWindowHandle::Xlib(h)) => h.window as isize,
        Ok(RawWindowHandle::Xcb(h)) => h.window.get() as isize,
        _ => 0,
    }
}

fn error(message: impl Into<String>) -> Value {
    json!({ "ok": false, "error": message.into() })
}

/// The server thread: accepts one verified client at a time and forwards each request to the UI thread.
fn server(endpoint: endpoint::Endpoint, tx: async_channel::Sender<Job>, native: isize) {
    loop {
        let (reader, mut writer) = match endpoint.accept() {
            Ok(Some(pair)) => pair,
            Ok(None) => continue, // refused peer; already logged
            Err(e) => {
                log::error!("automation accept failed: {e:#}");
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
        };
        let mut reader = BufReader::new(reader);
        loop {
            let line = match read_line_limited(&mut reader, MAX_REQUEST) {
                Ok(Some(l)) => l,
                Ok(None) => break,
                Err(e) => {
                    let _ = writeln!(writer, "{}", error(format!("bad request line: {e}")));
                    break;
                }
            };
            let request: Value = match serde_json::from_str(&line) {
                Ok(v @ Value::Object(_)) => v,
                _ => {
                    let _ = writeln!(
                        writer,
                        "{}",
                        error("request must be one JSON object per line")
                    );
                    continue;
                }
            };
            let verb = request["cmd"].as_str().unwrap_or("");
            let allowed = [
                "hello",
                "state",
                "tree",
                "a11y_activate",
                "click",
                "frame",
                "quit",
            ];
            if !allowed.contains(&verb) {
                let _ = writeln!(writer, "{}", error(format!("unknown verb {verb:?}")));
                continue;
            }
            let mut pre = Value::Null;
            if verb == "a11y_activate" {
                pre = activate_from_server_thread(native);
            }
            let (reply_tx, reply_rx) = std::sync::mpsc::channel();
            if tx.send_blocking((request, reply_tx)).is_err() {
                return;
            }
            let mut response = reply_rx
                .recv_timeout(Duration::from_secs(30))
                .unwrap_or_else(|_| error("the UI thread did not answer within 30 s"));
            if !pre.is_null() {
                response["server_thread"] = pre;
            }
            if writeln!(writer, "{response}")
                .and_then(|_| writer.flush())
                .is_err()
            {
                break;
            }
        }
    }
}

/// Windows: a WM_GETOBJECT for the UI Automation root, sent to the app's own window from this thread (the UI thread
/// handles it in its message loop). AccessKit treats it like a UIA client asking and activates.
#[cfg(windows)]
fn activate_from_server_thread(native: isize) -> Value {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_GETOBJECT,
    };
    const UIA_ROOT_OBJECT_ID: isize = -25;
    if native == 0 {
        return json!({ "sent": false, "reason": "no HWND" });
    }
    let mut result: usize = 0;
    let rc = unsafe {
        SendMessageTimeoutW(
            native as windows_sys::Win32::Foundation::HWND,
            WM_GETOBJECT,
            0,
            UIA_ROOT_OBJECT_ID,
            SMTO_ABORTIFHUNG,
            2000,
            &mut result,
        )
    };
    json!({ "sent": true, "method": "SendMessageTimeoutW(WM_GETOBJECT, UiaRootObjectId)", "rc": rc, "result": result })
}

#[cfg(not(windows))]
fn activate_from_server_thread(_native: isize) -> Value {
    Value::Null
}

/// Runs on the UI thread.
fn handle(
    request: &Value,
    window: AnyWindowHandle,
    view: &Entity<TrialView>,
    cx: &mut App,
    start: Instant,
) -> Value {
    let verb = request["cmd"].as_str().unwrap_or("");
    match verb {
        "hello" => cx
            .update_window(window, |_, window, _| {
                json!({
                    "ok": true,
                    "marker": MARKER,
                    "protocol": PROTOCOL,
                    "platform": platform_json(),
                    "window": window_json(window),
                    "uptime_ms": start.elapsed().as_millis(),
                })
            })
            .unwrap_or_else(|e| error(format!("window gone: {e:#}"))),
        "state" => {
            let v = view.read(cx);
            json!({
                "ok": true,
                "clicks": v.clicks,
                "rendered_clicks": v.rendered_clicks,
                "render_count": v.render_count,
            })
        }
        "tree" => cx
            .update_window(window, |_, window, cx| {
                let framework = window
                    .debug_a11y_tree_json()
                    .and_then(|s| serde_json::from_str::<Value>(&s).ok());
                let registry: Vec<Value> = view
                    .read(cx)
                    .registry
                    .borrow()
                    .iter()
                    .map(|t| {
                        json!({
                            "id": t.id,
                            "role": t.role,
                            "label": t.label,
                            "bounds_logical": [
                                f32::from(t.bounds.origin.x),
                                f32::from(t.bounds.origin.y),
                                f32::from(t.bounds.size.width),
                                f32::from(t.bounds.size.height),
                            ],
                        })
                    })
                    .collect();
                json!({
                    "ok": true,
                    "accesskit": {
                        "source": "gpui Window::debug_a11y_tree_json (the AccessKit tree GPUI built for the last frame)",
                        "a11y_enabled": window.is_a11y_enabled(),
                        "a11y_active": window.is_a11y_active(),
                        "tree": framework,
                    },
                    "registry": {
                        "source": "the app's own probes: a canvas inside each tagged element records its prepaint bounds",
                        "elements": registry,
                    },
                })
            })
            .unwrap_or_else(|e| error(format!("window gone: {e:#}"))),
        "a11y_activate" => cx
            .update_window(window, |_, window, _| {
                let note = activate_on_ui_thread(window);
                window.refresh();
                json!({ "ok": true, "ui_thread": note, "a11y_active_now": window.is_a11y_active() })
            })
            .unwrap_or_else(|e| error(format!("window gone: {e:#}"))),
        "click" => {
            let Some(id) = request["id"].as_str() else {
                return error("click needs a string \"id\"");
            };
            let found = view
                .read(cx)
                .registry
                .borrow()
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.bounds);
            let Some(bounds) = found else {
                return error(format!("no tagged element {id:?} in the last frame"));
            };
            let centre = point(
                bounds.origin.x + bounds.size.width / 2.0,
                bounds.origin.y + bounds.size.height / 2.0,
            );
            cx.update_window(window, |_, window, cx| {
                let modifiers = Modifiers::default();
                let moved = window.dispatch_event(
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: centre,
                        pressed_button: None,
                        modifiers,
                    }),
                    cx,
                );
                let down = window.dispatch_event(
                    PlatformInput::MouseDown(MouseDownEvent {
                        button: MouseButton::Left,
                        position: centre,
                        modifiers,
                        click_count: 1,
                        first_mouse: false,
                    }),
                    cx,
                );
                let up = window.dispatch_event(
                    PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position: centre,
                        modifiers,
                        click_count: 1,
                    }),
                    cx,
                );
                json!({
                    "ok": true,
                    "method": "gpui Window::dispatch_event (mouse move, down, up at the element's centre)",
                    "at_logical": [f32::from(centre.x), f32::from(centre.y)],
                    "results": [format!("{moved:?}"), format!("{down:?}"), format!("{up:?}")],
                })
            })
            .unwrap_or_else(|e| error(format!("window gone: {e:#}")))
        }
        "frame" => cx
            .update_window(window, |_, window, _| match capture::capture_frame(window) {
                Ok((img, method)) => {
                    let mut png = Vec::new();
                    match img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png) {
                        Ok(()) => json!({
                            "ok": true,
                            "method": method,
                            "width": img.width(),
                            "height": img.height(),
                            "png_base64": base64::engine::general_purpose::STANDARD.encode(&png),
                        }),
                        Err(e) => error(format!("PNG encoding failed: {e}")),
                    }
                }
                Err(e) => error(format!("capture failed: {e:#}")),
            })
            .unwrap_or_else(|e| error(format!("window gone: {e:#}"))),
        "quit" => json!({ "ok": true }),
        other => error(format!("unknown verb {other:?}")),
    }
}

/// macOS: ask the app's own content view for its accessibility children. AccessKit's subclass of that view answers
/// by activating (exactly what happens when VoiceOver first looks at the window). No other application is involved.
#[cfg(target_os = "macos")]
fn activate_on_ui_thread(window: &gpui::Window) -> Value {
    use objc2::runtime::AnyObject;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(RawWindowHandle::AppKit(h)) = HasWindowHandle::window_handle(window).map(|h| h.as_raw())
    else {
        return json!({ "asked": false, "reason": "no AppKit view" });
    };
    // GPUI's view is a subview of the window's content view; AccessKit subclasses the content view.
    let view = h.ns_view.as_ptr() as *mut AnyObject;
    let ns_window: *mut AnyObject = unsafe { objc2::msg_send![view, window] };
    if ns_window.is_null() {
        return json!({ "asked": false, "reason": "the view has no window" });
    }
    let content: *mut AnyObject = unsafe { objc2::msg_send![ns_window, contentView] };
    if content.is_null() {
        return json!({ "asked": false, "reason": "the window has no content view" });
    }
    let children: *mut AnyObject = unsafe { objc2::msg_send![content, accessibilityChildren] };
    json!({ "asked": true, "method": "[[view window] contentView] accessibilityChildren]", "returned_nil": children.is_null() })
}

#[cfg(target_os = "linux")]
fn activate_on_ui_thread(_window: &gpui::Window) -> Value {
    json!({
        "asked": false,
        "reason": "AccessKit on Linux activates when the session's AT-SPI bus reports org.a11y.Status.IsEnabled = true; \
                   the test harness sets that, the app cannot",
        "at_spi_bus_address_set": std::env::var_os("AT_SPI_BUS_ADDRESS").is_some(),
        "dbus_session_set": std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some(),
    })
}

#[cfg(windows)]
fn activate_on_ui_thread(_window: &gpui::Window) -> Value {
    json!({ "asked": false, "reason": "done from the server thread (see server_thread)" })
}

#[cfg(unix)]
mod endpoint {
    use anyhow::{Context as _, Result, anyhow, bail};
    use std::io::{Read, Write};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::os::unix::io::AsRawFd;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;

    /// The reading and writing halves of one connection.
    pub type Halves = (Box<dyn Read + Send>, Box<dyn Write + Send>);

    pub struct Endpoint {
        listener: UnixListener,
        dir: PathBuf,
        path: PathBuf,
    }

    fn unique_name() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        format!("rci-{}-{:08x}", std::process::id(), nanos)
    }

    impl Endpoint {
        pub fn create() -> Result<Self> {
            let base = std::env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .filter(|p| p.is_dir())
                .unwrap_or_else(std::env::temp_dir);
            let dir = base.join(unique_name());
            // Fails if anything already exists at that path.
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&dir)
                .with_context(|| format!("creating {}", dir.display()))?;
            let meta = std::fs::symlink_metadata(&dir)?;
            let uid = unsafe { libc::geteuid() };
            if !meta.is_dir() || meta.uid() != uid || meta.mode() & 0o777 != 0o700 {
                bail!(
                    "{} is not a private directory owned by this user",
                    dir.display()
                );
            }
            let path = dir.join("socket");
            // Create the socket file owner-only from the start, then make sure.
            let old = unsafe { libc::umask(0o177) };
            let bound = UnixListener::bind(&path);
            unsafe { libc::umask(old) };
            let listener = bound.with_context(|| format!("binding {}", path.display()))?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            Ok(Self {
                listener,
                dir,
                path,
            })
        }

        pub fn display(&self) -> String {
            self.path.display().to_string()
        }

        /// Accepts one connection. Returns None (after logging) when the peer is another user.
        pub fn accept(&self) -> Result<Option<Halves>> {
            let (stream, _) = self.listener.accept()?;
            let peer = peer_uid(&stream)?;
            let own = unsafe { libc::geteuid() };
            if peer != own {
                log::warn!(
                    "automation: refused a connection from user id {peer} (this app runs as {own})"
                );
                return Ok(None);
            }
            log::info!("automation: client connected (peer user id {peer} matches)");
            let reader = stream.try_clone()?;
            Ok(Some((Box::new(reader), Box::new(stream))))
        }
    }

    impl Drop for Endpoint {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_dir(&self.dir);
        }
    }

    #[cfg(target_os = "linux")]
    fn peer_uid(stream: &UnixStream) -> Result<u32> {
        let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let rc = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                &mut cred as *mut libc::ucred as *mut libc::c_void,
                &mut len,
            )
        };
        if rc != 0 {
            return Err(anyhow!(
                "SO_PEERCRED failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(cred.uid)
    }

    #[cfg(not(target_os = "linux"))]
    fn peer_uid(stream: &UnixStream) -> Result<u32> {
        let mut uid: libc::uid_t = 0;
        let mut gid: libc::gid_t = 0;
        let rc = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
        if rc != 0 {
            return Err(anyhow!(
                "getpeereid failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(uid)
    }
}

#[cfg(windows)]
mod endpoint {
    use anyhow::{Result, anyhow};
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_PIPE_CONNECTED, GetLastError, HANDLE, INVALID_HANDLE_VALUE, LocalFree,
    };
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_READMODE_BYTE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    /// The reading and writing halves of one connection.
    pub type Halves = (Box<dyn Read + Send>, Box<dyn Write + Send>);

    pub struct Endpoint {
        name: String,
        wide_name: Vec<u16>,
        descriptor: PSECURITY_DESCRIPTOR,
        /// The instance created up front with FILE_FLAG_FIRST_PIPE_INSTANCE, used for the first client.
        pending: std::sync::Mutex<Option<isize>>,
    }

    // The descriptor is only read after creation.
    unsafe impl Send for Endpoint {}

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The current user's SID as a string (S-1-5-...).
    fn user_sid_string() -> Result<String> {
        unsafe {
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(anyhow!("OpenProcessToken failed: {}", GetLastError()));
            }
            let mut needed: u32 = 0;
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
            let mut buf = vec![0u8; needed as usize];
            let ok = GetTokenInformation(
                token,
                TokenUser,
                buf.as_mut_ptr().cast(),
                needed,
                &mut needed,
            );
            CloseHandle(token);
            if ok == 0 {
                return Err(anyhow!("GetTokenInformation failed: {}", GetLastError()));
            }
            let user = &*(buf.as_ptr() as *const TOKEN_USER);
            let mut sid_str: *mut u16 = std::ptr::null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut sid_str) == 0 {
                return Err(anyhow!("ConvertSidToStringSidW failed: {}", GetLastError()));
            }
            let mut len = 0;
            while *sid_str.add(len) != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(sid_str, len));
            LocalFree(sid_str.cast());
            Ok(s)
        }
    }

    impl Endpoint {
        pub fn create() -> Result<Self> {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            let name = format!(r"\\.\pipe\render-ci-{}-{:08x}", std::process::id(), nanos);
            let sid = user_sid_string()?;
            // Protected DACL with one entry: generic all for this user. Nobody else, not even administrators.
            let sddl = format!("D:P(A;;GA;;;{sid})");
            // The SID itself is not logged: CI logs are public.
            log::info!("automation: pipe DACL D:P(A;;GA;;;<this user's SID>)");
            let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            let ok = unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    wide(&sddl).as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                return Err(anyhow!(
                    "building the security descriptor failed: {}",
                    unsafe { GetLastError() }
                ));
            }
            let mut endpoint = Self {
                wide_name: wide(&name),
                name,
                descriptor,
                pending: std::sync::Mutex::new(None),
            };
            let first = endpoint.new_instance(true)?;
            *endpoint.pending.get_mut().unwrap() = Some(first as isize);
            Ok(endpoint)
        }

        fn new_instance(&self, first: bool) -> Result<HANDLE> {
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: self.descriptor,
                bInheritHandle: 0,
            };
            let open_mode = PIPE_ACCESS_DUPLEX
                | if first {
                    FILE_FLAG_FIRST_PIPE_INSTANCE
                } else {
                    0
                };
            let handle = unsafe {
                CreateNamedPipeW(
                    self.wide_name.as_ptr(),
                    open_mode,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    PIPE_UNLIMITED_INSTANCES,
                    1 << 20,
                    1 << 16,
                    0,
                    &attributes,
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                return Err(anyhow!("CreateNamedPipeW failed: {}", unsafe {
                    GetLastError()
                }));
            }
            Ok(handle)
        }

        pub fn display(&self) -> String {
            self.name.clone()
        }

        pub fn accept(&self) -> Result<Option<Halves>> {
            let pending = self.pending.lock().unwrap().take();
            let handle = match pending {
                Some(h) => h as HANDLE,
                None => self.new_instance(false)?,
            };
            let ok = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) };
            if ok == 0 {
                let err = unsafe { GetLastError() };
                if err != ERROR_PIPE_CONNECTED {
                    unsafe { CloseHandle(handle) };
                    return Err(anyhow!("ConnectNamedPipe failed: {err}"));
                }
            }
            let mut pid: u32 = 0;
            unsafe { GetNamedPipeClientProcessId(handle, &mut pid) };
            log::info!(
                "automation: client connected (process {pid}; the pipe's DACL admits this user only)"
            );
            let file = unsafe { File::from_raw_handle(handle as _) };
            let reader = file.try_clone()?;
            Ok(Some((Box::new(reader), Box::new(file))))
        }
    }
}
