# 0012: Embedded browser trial

Status: proposed (trial in progress)
Date: 2026-11-17

This is the record of M0 Task 12, the trial of the riskiest unknown in the project: whether Chromium can be embedded
inside a GPUI window on macOS, Linux and Windows by rendering it off-screen in a helper process and drawing its frames as
an element. Phase A (research, the trial code that needs no Chromium, and measurements with a stand-in frame source) is
recorded here. Phase B (the CEF helper running, the hands-on measurements, CI on Linux and Windows) finishes it, and only
then does the "Decision" section become a decision.

## Context

Decision 0004 embeds Chromium off-screen in a helper process, drawn as an element, so app UI (ask cards, popovers, the
command palette) can sit above a page. Design section 5.8 adds a `BrowserView` trait, CEF as the first engine, pointer,
keyboard, text entry, cursor and popup forwarding, pages driven over the Chrome DevTools Protocol, Chromium's own sandbox
on everywhere, the component fetched on first use as a version-pinned, hash-checked download, and an external-window
fallback per platform. Section 8 allows the OS web view on Windows. The threat model (4.9) requires a portless DevTools
channel (P32) and a helper channel that is an inherited anonymous pipe plus shared memory, never a named socket (P53).
GPUI is pinned to the `gpui-pre 0.3.12` snapshot (decision 0007), whose `surface()` element, the only way it draws
external pixel buffers, is macOS-only.

The closest precedent, a crate by the widget kit's own maintainers that wraps CEF off-screen rendering for GPUI, is
described by its authors as experimental and not good enough; they ship the system web view instead.

Every fact below was checked on 2026-11-17 against the sources named, unless marked unverified.

## Options considered

| Option | Trade-off |
|---|---|
| A. CEF off-screen in a helper process, CPU frames through shared memory, drawn as GPUI images (phase A's choice to try first) | Works with the pinned GPUI on all three platforms; every frame is copied and uploaded to the GPU on the app's UI thread. |
| B. As A, but CEF's shared textures (IOSurface, DXGI handle, dmabuf) drawn directly | Fastest; needs GPUI changes on every platform (no external texture path on Linux or Windows; macOS surfaces take only NV12). |
| C. CEF as a native child view | Rejected in 0004: it sits above everything GPUI draws. |
| D. Windows only: the OS web view (WebView2) behind the same trait | OS-patched Chromium that speaks DevTools without a port; can sit under app UI only through DirectComposition visual hosting, which needs a GPUI change. |
| E. An external browser window | The fallback per platform where embedding fails. |

## Findings (phase A)

### CEF bindings for Rust

- **`cef` (the Tauri team's bindings)**, github.com/tauri-apps/cef-rs, crates `cef`, `cef-dll-sys`, `download-cef`,
  `export-cef-dir`. Version 156.6.0+156.0.30, published 2026-11-16, which targets CEF 156.0.30 (Chromium 156.0.8078.96).
  Licence Apache-2.0 OR MIT. Actively maintained: releases track CEF stable within days (156.0.0 on 2026-10-23 to
  156.6.0), about 0.2 million downloads in 90 days, 18 open issues. Targets: x86_64 and ARM64 on Linux, macOS and
  Windows. Bindings are pre-generated per target, so no bindgen at build time.
- **How CEF is obtained (verified in the published `cef-dll-sys` build script).** Building `cef-dll-sys` without the
  `dox` feature downloads `https://cef-builds.spotifycdn.com/index.json` (10.7 MB), then the "minimal" archive for the
  target and version, checks its SHA-1 against that index, extracts it, and builds CEF's C++ wrapper
  (`libcef_dll_wrapper`) with CMake and Ninja. Where: `OUT_DIR` (inside the build folder) by default; `CEF_PATH/<cef
  version>/` if `CEF_PATH` is set to a folder that does not hold an extracted copy; nothing if it does. An archive already
  present with the right SHA-1 is not downloaded again. `NIX_CEF_BINARY` and `FLATPAK` are other sources.
  `CEF_DOWNLOAD_URL` changes the host. With the `dox` feature the build script does nothing, so code can be
  type-checked without CEF: the trial's CEF helper type-checks this way (phase A ran it; it downloaded nothing).
- **Off-screen rendering.** `OnPaint` (a BGRA buffer with dirty rectangles, any platform) and `OnAcceleratedPaint`
  (a shared texture: a D3D11 shared `HANDLE` on Windows, an `IOSurface` on macOS, dmabuf planes on Linux) are both bound.
  CEF's header says the shared texture is valid only during the callback and must be copied into a texture the client
  owns. The header comment on `shared_texture_enabled` still says "only supported on Windows (D3D11)" while the
  `OnAcceleratedPaint` text covers all three; the bindings' `accelerated_osr` feature imports all three into `wgpu`
  textures. Shared textures on macOS and Linux are therefore claimed, not verified here. Windowless browsers always use
  CEF's Alloy runtime style (header text).
- **What an application ships** (bindings' bundling code and CEF documentation): macOS, an app bundle with
  `Contents/Frameworks/Chromium Embedded Framework.framework` and five helper bundles named `<name> Helper.app`,
  `<name> Helper (GPU).app`, `(Renderer)`, `(Plugin)`, `(Alerts)`, each `LSUIElement`; the sub-process sandbox comes from
  `libcef_sandbox.dylib` inside the framework, loaded by each helper at the top of `main` (M138 and later). Linux,
  `libcef.so` and its resources (`*.pak`, `icudtl.dat`, V8 snapshots, `locales/`, SwiftShader and ANGLE libraries) next
  to the executable, plus `chrome-sandbox`, which must be owned by root with the setuid bit to be used. Windows,
  `libcef.dll` and resources next to the executable; since M138 the sandbox needs the client built as a DLL and started
  by CEF's `bootstrap.exe` (renamed to the client's name), which the bindings' bundler supports.
- **Other bindings.** `tauri-runtime-cef` (Tauri's CEF runtime, alpha, built on `cef`); `bevy_cef` (off-screen CEF into
  Bevy textures, CEF in the game's own process); `wef` (the precedent below). None runs CEF in a separate process the
  way this design does.

### The precedent: `wef`

github.com/longbridge/wef (Apache-2.0), extracted from the widget kit in 2025-09, last pushed 2026-01-01. What its own
repository says went wrong:

- Its README marks it experimental and not good enough, citing app size (the CEF framework added about 1 GB) and the
  development experience; its authors ship the system web view (through Wry) in their desktop app.
- CEF runs inside the app's process, pumped from GPUI's loop through a CEF external message pump written per platform
  in C++, and the same executable serves as CEF's sub-process.
- Open issues: special keys (Enter, Backspace, Shift) do nothing on macOS (#14); calling its init with default settings
  restarts the program's `main` over and over until the machine falls over (#9, the classic single-executable sub-process
  re-entry); video sites fail with "no supported streams" (#2: CEF's published builds carry no proprietary codecs).
- Its frame path is the one option A uses: a new GPUI `RenderImage` per `OnPaint`, the previous one dropped from the
  atlas.

What this design does differently: CEF in its own process (no external pump, no app-object conflict, no sub-process
re-entry in the app), explicit key-code mapping (`spikes/browser/cef-helper/src/keys.rs`), and the component fetched on
first use, so the installer does not grow. The codec gap stays: a CEF build with H.264 and AAC means building Chromium
with proprietary codecs, which brings patent licensing.

### macOS: CEF's application object and GPUI's

- CEF requires `NSApp` to be an `NSApplication` subclass implementing `CefAppProtocol` (`isHandlingSendEvent` and
  `setHandlingSendEvent:`, and `sendEvent:` setting the flag) before it initializes (bindings' `application_mac.rs`,
  their `cefsimple` example).
- GPUI registers its own `GPUIApplication` subclass at load time, with no such methods (`gpui-pre-macos 0.3.12`,
  `platform.rs`). Running CEF in the app's process would mean adding methods to GPUI's class at run time and pumping
  CEF from GPUI's loop.
- A helper process that owns CEF entirely avoids both: it installs its own subclass (`cef-helper/src/mac.rs`, activation
  policy "prohibited", so no Dock icon or menu bar) and runs CEF's own loop (`CefRunMessageLoop`). The thread reading the
  app's messages posts each one to CEF's UI thread (`CefPostTask`). This compiles against the bindings; that it runs is
  phase B.

### Frame transport, and what this GPUI snapshot can draw

Read from the pinned sources in the cargo registry:

- **`surface()` is macOS-only, and NV12-only.** The element exists only on macOS and iOS; the Metal renderer accepts only
  `CVPixelBuffer`s in `kCVPixelFormatType_420YpCbCr8BiPlanarFullRange` (it was built for screen-share video). CEF's
  textures are BGRA, so drawing them this way needs a GPU conversion to 4:2:0, which blurs coloured text, or a GPUI
  change. On Windows `draw_surfaces` is an empty function; on Linux the `wgpu` renderer skips surface batches.
- **The path that works everywhere: a `RenderImage` per frame.** `window.paint_image` takes an `Arc<RenderImage>` (BGRA
  bytes). The atlas caches by image id, so each new frame is a new image: one upload into the sprite atlas (Metal
  `replaceRegion`, D3D11 `UpdateSubresource`, `wgpu` `write_texture` after one more copy). A frame wider or taller than
  1024 pixels gets an atlas texture of its own, so each frame also allocates a texture, and the previous frame's image
  must be dropped with `window.drop_image` or the atlas grows.
- **Conclusion: without changing GPUI, Linux and Windows have exactly one frame path, the CPU one.** Option B needs an
  external-texture element in GPUI's three renderers, a fork of the pinned snapshot (decision 0007).
- Transport options for B, for later: macOS `IOSurface` (cross-process by mach port, which needs a mach channel, not a
  pipe; global surface ids are deprecated and readable by any process); Windows DXGI NT handle (duplicated into the app,
  opened with `OpenSharedResource1`); Linux dmabuf file descriptors (passed over a socket pair, imported with Vulkan
  external memory).

### Input forwarding

| What | CEF (off-screen) | GPUI side |
|---|---|---|
| Pointer | `SendMouseMoveEvent` (with a leave flag), `SendMouseClickEvent` (button, up/down, click count) | `on_mouse_move`, `on_any_mouse_down`, `on_mouse_up`; positions made pane-relative |
| Wheel | `SendMouseWheelEvent` in pixels | `on_scroll`: `ScrollDelta::Pixels` or `Lines` (a line is about 40 pixels); sign checked by hand in phase B |
| Keys | `SendKeyEvent` `RAWKEYDOWN`, `KEYUP`, `CHAR`, with a Windows virtual-key code, the macOS native code and the modifiers | `on_key_down` and `on_key_up` give a key name and the character; names mapped explicitly |
| Text | `CHAR` events, or `ImeCommitText` inside a composition | the input handler's `replace_text_in_range` (registered with `window.handle_input` in paint) |
| Input methods | `ImeSetComposition`, `ImeFinishComposingText`, `ImeCancelComposition`; `OnImeCompositionRangeChanged` gives character bounds | `replace_and_mark_text_in_range`, `unmark_text`; `bounds_for_range` returns the engine's character bounds so the candidate window follows |
| Cursor | `OnCursorChange` (display handler) | `window.set_cursor_style` on the pane's hitbox; GPUI has no custom-image cursors, so those fall back to an arrow |
| Popups (`<select>`) | `OnPopupShow`, `OnPopupSize`, and `OnPaint` with `PET_POPUP` | painted as a second image at the popup's rectangle |
| Focus | `SetFocus` | `on_focus` and `on_blur` subscriptions |
| Scale | `GetScreenInfo` (`device_scale_factor`), `NotifyScreenInfoChanged`, `WasResized`, `GetViewRect` in logical pixels | `window.scale_factor()`, the pane's bounds in prepaint |

Found with the stand-in (each fixed in the trial code):

- The pane's hitbox must be inserted in prepaint, like every element's. Inserted in paint, it lands above the overlay's
  hitbox, so the page kept getting pointer moves under the overlay.
- GPUI's hover state turns off while the user types, so it cannot stand for "the pointer left the page". The trial sends
  `PointerLeave` when its hitbox stops being hovered after a pointer move.
- `window.dispatch_keystroke` delivers a key-down to the element and then the character through the input handler: the
  key-down plus character pair CEF's `RAWKEYDOWN` plus `CHAR` model expects.
- The input handler cannot return the page's text (`text_for_range` is `None`); CEF reports only the selected text.
  Input methods that need surrounding text (reconversion) may degrade. Unverified until phase B.

### DevTools Protocol

CEF's `SendDevToolsMessage`, `ExecuteDevToolsMethod` and `AddDevToolsMessageObserver` work without a DevTools front-end
or a remote-debugging session (CEF header text). The helper relays the app's calls through them over its own pipe, so no
port is opened (P32); it also refuses to start with a remote-debugging switch. A remote-debugging port is a TCP listener
any local process can reach; Chromium's `--remote-debugging-pipe` would also work without a port, but CEF's own entry
point needs no extra channel at all. The bindings' generated documentation says "function" where the protocol says
"method"; the helper sends `method` (to confirm in phase B).

### Windows: the OS web view

WebView2 (Microsoft Learn, read 2026-11-17): it speaks the DevTools Protocol in-process through
`ICoreWebView2::CallDevToolsProtocolMethod` and `ICoreWebView2DevToolsProtocolEventReceiver`, no port. It has three
hosting modes: windowed (a child window, which sits above everything GPUI draws, like option C), window-to-visual (still
hosted in a window), and visual hosting (`CoreWebView2CompositionController`: the web view renders into a DirectComposition
or `Windows.UI.Composition` visual and the app forwards pointer input). GPUI's Windows renderer already draws through
DirectComposition (a premultiplied-alpha swap chain in a composition visual, `gpui-pre-windows`, `directx_renderer.rs`),
but keeps its device and visual private, so placing a web-view visual below GPUI's and leaving a transparent hole needs a
GPUI change. WebView2 has no off-screen frame stream (its preview capture produces single PNG or JPEG images).
Provisional: WebView2 cannot go under app-drawn UI in this snapshot without a GPUI change; as a windowed view it is a
better fallback than an external window, keeping the browser in the app but without overlays.

### Linux: Chromium's sandbox by package format

Chromium's first layer is a namespace sandbox (unprivileged user namespaces) or the setuid helper (`chrome-sandbox`,
owned by root, mode 4755); the second is seccomp-bpf (Chromium `sandbox/linux/README.md`). Ubuntu restricts
unprivileged user namespaces through AppArmor since 23.10, on by default from 24.04; an application gets them back
through an AppArmor profile with a `userns` rule (Chrome's own package ships one for its install path) or uses the
setuid helper (Chromium `docs/security/apparmor-userns-restrictions.md`).

- `.deb`: can install `chrome-sandbox` owned by root with the setuid bit, and an AppArmor profile, so the sandbox works.
  CI checks this in phase B (an installed `.deb`, then the renderer's seccomp mode and namespaces).
- Tarball: unpacked as the user, so no setuid helper, and on Ubuntu 24.04 and later no user namespaces either: no usable
  sandbox. The design forbids turning it off, so the tarball cannot carry the browser there unless the user runs a
  privileged setup step. CI records the failure mode in phase B.
- AppImage: mounted `nosuid` through FUSE and blocked from user namespaces on Ubuntu, the known trouble the design names.
  Flatpak needs Flatpak's own sandbox through a shim (zypak); Snap gets a generated AppArmor profile that allows user
  namespaces. Both community reports, unverified here.

## Stand-in measurements (phase A, macOS)

The trial code in `spikes/browser` (a standalone workspace) is complete except for running CEF:

- `protocol`: the `BrowserView` trait; JSON-line messages on the helper's inherited stdin and stdout; three
  shared-memory frame slots in one region the app creates and the helper inherits (Unix: a POSIX object unlinked as soon
  as it is open, passed as descriptor 3; Windows: an inheritable unnamed file mapping); `HelperBrowser`, which spawns a
  helper, copies frames out and releases slots on a reader thread, and never touches the UI thread; `spike-smoke`, a
  headless check of any helper.
- `standin`: a helper with no CEF that paints a moving test pattern (scrolling stripes, a moving square, the pointer as
  it saw it, the typed text, a green corner marker) into the slots at 60 frames a second, logs the input it receives,
  reports cursor changes, composition bounds and a popup, and echoes the two DevTools calls, labelled as a stand-in.
- `app`: the GPUI window. A canvas element draws each frame with `paint_image` and drops the previous one; input is
  forwarded as in the table above; an occluding overlay card is drawn above the page. `--script` drives pointer, click,
  typing, a composition, a wheel step and a click on the overlay through GPUI's own event dispatch; `--selfcheck` reads
  the window back with GPUI's `render_to_image` and checks two pixels.
- `cef-helper`: the CEF helper described above, behind an optional feature; type-checked with `--features dox`.

Machine: Apple M4, 16 GB, macOS 26.6.2, displays at 60 Hz. Release build. Commands, from `spikes/browser`:
`cargo build --release`, then `target/release/spike-app --seconds 15 --selfcheck --script --demo-popup` with
`--display 1` (a scale-2 display) or `--display 0` (scale 1). Figures are the medians of the per-second lines after the
first two seconds (13 seconds per run); CPU is each process's own user plus system time over wall time, in percent of
one core, read with `getrusage` and cross-checked once with `ps` (34 % and 4.2 % against 33 % and 4 % logged).

| | 1280 x 800 logical at scale 2 (2560 x 1602 device pixels) | 1280 x 800 at scale 1 |
|---|---|---|
| Frames painted per second (helper sends 60) | 61 (59 to 63), none superseded, none skipped | 63 (61 to 63), none superseded |
| `paint_image` on the UI thread, average (worst second's maximum) | 3.9 ms (7.1 ms) | 1.5 ms (2.5 ms) |
| Copy out of shared memory, reader thread | 1.1 ms | 0.35 ms |
| Helper painted to app painted, average (maximum) | 17 ms (24 ms) | 7.4 ms (12.6 ms) |
| App CPU | 37 % | 23 % |
| Stand-in CPU (painting the pattern) | 6 % | 2.5 % |
| App memory, footprint (resident) | 310 MB (233 MB) | 126 MB (104 MB) |

At one frame a second (`--fps 1`, scale 2) the app used 1.4 % CPU, and a single cold frame's `paint_image` took 5 to
7 ms. The "painted" counts slightly exceed 60 because each counting window is a little longer than one second.

Overlay and input, from the program's own logs: the self-check read the overlay's swatch as magenta (255, 0, 255) and the
page's corner marker as green (0, 255, 0) at both scales, so GPUI draws the overlay above the frames and the frames reach
the window. The script's pointer moves came back as cursor changes from the helper; the click focused the pane and
reached the page; typed keys arrived as key-down plus text; the composition's character bounds came back and placed the
candidate rectangle; the click on the overlay did not reach the page, and moving onto it sent `PointerLeave`.
`spike-smoke` against the stand-in: 30 frames of the right size with the marker intact, both DevTools answers.

What this shows: with the pinned GPUI, externally produced frames reach the screen at the display's 60 Hz with headroom
at a typical pane size, and app UI draws above them. The cost is real: about 4 ms of the UI thread per full 2560 x 1600
frame. At 120 Hz (an 8.3 ms budget) or a full-screen pane on a large high-resolution display, a stream of full-frame
updates would take half the budget or more. CEF repaints only on change and reports dirty rectangles, so splitting the
frame into tiles that are uploaded only when dirty is the first optimisation to try; scrolling still dirties everything.

## The CEF download (for the maintainer)

For phase B on this Mac (macOS arm64):

- File: `cef_binary_156.0.30+g6028375+chromium-156.0.8078.96_macosarm64_minimal.tar.bz2`, from
  `https://cef-builds.spotifycdn.com` (CEF's build host, served by a CDN). 134,874,408 bytes (128.6 MiB) compressed.
  On disk after extraction: not published; estimated 300 to 400 MB, so about 0.5 GB with the archive kept. Measured in
  phase B.
- Checksums: the host publishes `.sha1` and `.sha256` files beside each archive and SHA-1 in its index. SHA-256
  `731b599e78c71bf33530d65bfa18963f7d1d59902c1f381f5452188e5116b855` (SHA-1
  `6393d8a838d3b29f6b6939b970b5931dde2082aa`). Both come from the same host, so they catch corruption, not a compromised
  host; CEF archives are not signed.
- Licence: CEF is under a BSD-3-Clause-style licence; Chromium is BSD-3-Clause with many third-party components whose
  notices are in the `CREDITS.html` the distribution carries and an app must ship.
- Where it lands: inside the spike's build folder only. The proposed command pins the hash and keeps the build script
  from downloading the archive itself:

  ```sh
  cd spikes/browser
  mkdir -p target/cef/156.0.30
  curl --proto '=https' --tlsv1.2 -fL -o \
    "target/cef/156.0.30/cef_binary_156.0.30+g6028375+chromium-156.0.8078.96_macosarm64_minimal.tar.bz2" \
    "https://cef-builds.spotifycdn.com/cef_binary_156.0.30%2Bg6028375%2Bchromium-156.0.8078.96_macosarm64_minimal.tar.bz2"
  echo "731b599e78c71bf33530d65bfa18963f7d1d59902c1f381f5452188e5116b855  target/cef/156.0.30/cef_binary_156.0.30+g6028375+chromium-156.0.8078.96_macosarm64_minimal.tar.bz2" | shasum -a 256 -c -
  CEF_PATH="$PWD/target/cef" cargo build --release -p spike-cef-helper --features cef
  ```

  The build step then fetches only `index.json` (10.7 MB) to compare the SHA-1, and extracts to
  `target/cef/156.0.30/cef_macos_aarch64/`. Without `CEF_PATH`, the same build downloads the archive itself into
  `target/release/build/cef-dll-sys-*/out/` (also inside the spike). No global cache is used either way.
- **Also needed on this Mac: CMake and Ninja**, which the bindings' build script runs to build CEF's C++ wrapper. Neither
  is installed, and installing them needs the maintainer's agreement.
- CI downloads per run: macOS 134.9 MB, Linux 333.3 MB (`..._linux64_minimal.tar.bz2`, SHA-256
  `356db99d653698aa9ba78b22965032b4fa5b23ba079ffdbaa5d28a8530f04011`), Windows 175.2 MB
  (`..._windows64_minimal.tar.bz2`, SHA-256 `74fd321975e99703acd8fc7e3c1b0859292f61b388f4052a0cd7ef1fb3b29f6e`), plus
  `index.json` each. The temporary workflow pins these hashes.

## Phase B plan

- **Process layout.** The app starts `spike-cef-helper` (inside `spike-cef-helper.app` on macOS, laid out by
  `cef-helper/bundle-macos.sh`) with piped stdin and stdout and the inherited frame region. The helper is CEF's browser
  process with its own application object and loop, a dedicated profile under the build folder, and descriptors 0 and 1
  pointed away from the pipes before CEF starts, so Chromium's sub-processes cannot inherit the protocol channel. CEF
  starts the renderer, GPU and utility sub-processes from the helper bundles, inside Chromium's sandbox.
- **Transport first: option A**, exactly as measured with the stand-in. **Second, if A misses the targets:** dirty-tile
  uploads (frames split into tiles, each its own image, re-uploaded only when a dirty rectangle touches it). Shared
  textures (option B) only as a separate, labelled experiment, because they need a GPUI fork.
- **Measured by hand on this Mac**, with the app's per-second log lines, `getrusage`, `ps` over the helper's process tree
  and the self-check:
  1. Frame rate while scrolling a long page (a generated long local page; trackpad scrolling for ten seconds): painted
     frames per second, frames superseded, `paint_image` time, and the page's own `requestAnimationFrame` rate read with
     a DevTools `Runtime.evaluate`.
  2. Typing in a form: letters, Enter, Backspace, arrows, Tab; then Japanese and Chinese input methods, checking the
     candidate window sits at the caret and the committed value read back with `Runtime.evaluate`.
  3. A GPUI popover over the page: the overlay card, also over an open `<select>` popup; self-check readback.
  4. Memory per tab: the sum over the helper and all its sub-processes (footprint and resident), for a blank page, a
     simple page and a heavy page; plus idle and scrolling CPU of every process, and time from helper start to first frame.
  5. Size of the component: the bundle on disk, the CEF archive, and the bundle compressed as it would be downloaded.
- **CI** (the temporary workflow): Linux and Windows build the helper with CEF downloaded on the runner; macOS bundles
  and smoke-checks it; Linux smoke-checks it headless (`--ozone-platform=headless`) as a tarball-style run without the
  setuid helper (recording what happens) and as an installed `.deb` with `chrome-sandbox` setuid root, printing each
  renderer's seccomp mode and namespaces; Windows runs an informational smoke check, labelled as lacking the bootstrap
  sandbox; every platform reports the component's size. The stand-in job proves the pipes and shared memory on all three.
- **Fall back to an external window on a platform if** any of these holds after phase B: CEF cannot run off-screen there
  with Chromium's sandbox on (for Windows, if the bootstrap-DLL arrangement cannot host an off-screen helper); the painted
  rate while scrolling a 1280 x 800 logical pane at the platform's usual scale stays under 30 frames a second, or the UI
  thread's median `paint_image` time exceeds 8 ms; typing (letters, Enter, Backspace, arrows) or one CJK input method
  cannot be made to work; the GPUI overlay cannot be drawn above the page; or the helper cannot be restarted reliably
  after a renderer crash. On Linux and Windows, where nothing can be run by hand, CI's build and headless smoke check must
  pass, the visual criteria are judged from macOS (the frame path is the same code), and the result is labelled
  "CI-verified, not yet run on real hardware". On Windows, WebView2 as a windowed view is weighed against the external
  window before falling back.

## Open questions and what is unverified

- Everything CEF does at run time: that the helper starts with its own application object and loop, that `OnPaint`
  frames arrive and match the stand-in's numbers, that the key mapping and input methods work, that DevTools calls are
  answered, and the true on-disk size. Phase B.
- Shared textures on macOS and Linux (claimed by CEF's header text and the bindings, contradicted by one header
  comment).
- Windows: that the standard library's handle inheritance delivers the unnamed mapping to the helper (the stand-in CI
  job will show it); that a CEF helper started by `bootstrap.exe` with the sandbox works for off-screen rendering; whether
  inheritable standard handles reach Chromium's sub-processes.
- Linux: the package names for CEF's libraries on Ubuntu 26.04 (taken from 24.04), whether the runner restricts user
  namespaces, and whether a headless run is representative of a desktop session.
- Clipboard interception for P52 and the P33 question (a confined helper with Chromium's sandbox still engaging) are not
  covered by phase A.
- The 60-day freshness rule means a new CEF component about every four weeks; how it is built, signed (macOS: the helper
  bundles and framework signed and notarized by the project, since the component contains our helper) and published is a
  packaging question (Task 14 and M4).
- Video sites need proprietary codecs that CEF's published builds lack.

## Decision

Pending phase B. Working proposal, to be confirmed or replaced by the measurements: embed with CEF off-screen in a
helper process using option A on all three platforms, with dirty-tile uploads if needed; keep option B as a later GPUI
change only if the targets demand it; on Windows weigh WebView2 as a windowed fallback; ship the browser only in package
formats that can run Chromium's sandbox (on Linux the `.deb`).

## Who decided

Not decided yet. The method is a design call within the maintainer's decision 0004; downloading CEF onto the development
machine, and installing CMake and Ninja there, wait for the maintainer.

## Consequences

To be written in phase B. Already clear from phase A: the frame path costs one copy and one GPU upload per frame on the
app's UI thread; GPUI's atlas allocates a texture per full frame; any faster path is a change to the pinned GPUI; and the
Linux tarball cannot carry the browser where user namespaces are restricted.

## Revisit when

Phase B completes; GPUI gains an external-texture element on Linux and Windows (or BGRA surfaces on macOS); the pinned
`gpui-pre` snapshot changes how images reach the atlas; CEF's shared textures are confirmed on macOS and Linux; or a
measurement shows the CPU path missing the targets set in phase B.
