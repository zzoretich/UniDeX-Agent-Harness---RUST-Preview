# 0012: Embedded browser trial

Status: accepted with open questions
Date: 2026-11-17

This is the record of M0 Task 12, the trial of the riskiest unknown in the project: whether Chromium can be embedded
inside a GPUI window on macOS, Linux and Windows by rendering it off-screen in a helper process and drawing its frames as
an element. Phase A did the research and the trial code that needs no Chromium; phase B ran real Chromium on the
development Mac, measured it, and built the helper on Linux and Windows in CI. The open questions are the CI results on
Linux and Windows (the workflow had not run when this was written) and the items listed under "What does not work".

## Context

Decision 0004 embeds Chromium off-screen in a helper process, drawn as an element, so app UI (ask cards, popovers, the
command palette) can sit above a page. Design section 5.8 adds a `BrowserView` trait, CEF as the first engine, pointer,
keyboard, text entry, cursor and popup forwarding, pages driven over the Chrome DevTools Protocol, Chromium's own sandbox
on everywhere, the component fetched on first use as a version-pinned, hash-checked download, and an external-window
fallback per platform. Section 8 allows the OS web view on Windows and sets the platform tiers (macOS hands-on, Linux and
Windows CI-verified). The threat model (4.9) requires a portless DevTools channel (P32) and a helper channel that is an
inherited anonymous pipe plus shared memory, never a named socket (P53). GPUI is pinned to the `gpui-pre 0.3.12`
snapshot (decision 0007), whose `surface()` element, the only way it draws external pixel buffers, is macOS-only and
takes only NV12 buffers.

The closest precedent, a crate by the widget kit's own maintainers that wraps CEF off-screen rendering for GPUI (`wef`,
Apache-2.0), is described by its authors as experimental and not good enough; they ship the system web view instead. Its
open issues: special keys did nothing on macOS, its init re-ran the program's `main` in a loop, video sites failed. No
code was copied from it.

Facts below were checked on 2026-11-17 against the sources or runs named; "unverified" marks what only documentation or
an unrun CI job supports.

## Options considered

| Option | Trade-off |
|---|---|
| A. CEF off-screen in a helper process, CPU frames through shared memory, drawn as GPUI images | Works with the pinned GPUI on all three platforms; each frame is copied twice on the CPU and uploaded to the GPU on the app's UI thread. Chosen. |
| B. As A, but CEF's shared textures (IOSurface, DXGI handle, dmabuf) drawn directly | Removes the CPU copies and the upload; needs an external-texture element in GPUI on every platform (a fork of the pinned snapshot) and a handle channel between the processes. |
| C. CEF as a native child view | Rejected in 0004: it sits above everything GPUI draws. |
| D. Windows only: the OS web view (WebView2) behind the same trait | OS-patched, nothing to download, DevTools without a port; under app-drawn UI only through DirectComposition visual hosting, which needs a GPUI change; as a windowed view it covers the overlays. |
| E. An external browser window | The fallback per platform where embedding fails. |

## What was built

Everything is in `spikes/browser/`, a standalone cargo workspace outside the repository's (throwaway code; dependencies
pinned exactly with their licences in `Cargo.toml`; CEF bindings `cef = "=156.6.0"`, Apache-2.0 OR MIT, for CEF 156.0.30,
Chromium 156.0.8078.96).

- `protocol/`: the `BrowserView` trait (`view.rs`); JSON-line messages on the helper's inherited stdin and stdout with a
  16 MiB line limit (`wire.rs`); three shared-memory frame slots in one region the app creates and the helper inherits,
  never named (`shm.rs`: a POSIX object unlinked on creation and passed as descriptor 3, or an inheritable unnamed
  mapping on Windows), frame dimensions checked before any pixel is touched; `HelperBrowser` (`helper.rs`), which spawns
  the helper and keeps the UI thread off the pipes with a reader and a writer thread; the process tree with each process's
  sandbox state (`tree.rs`); `spike-smoke`, a headless check of any helper.
- `cef-helper/`: CEF's browser process with its own `NSApplication` subclass on macOS (`mac.rs`) and CEF's own message
  loop, one visible windowless browser plus optional hidden tabs, `OnPaint` into the slots, input mapped onto CEF's
  off-screen calls with explicit key codes (`keys.rs`), DevTools relayed with `SendDevToolsMessage`, a re-launch guard
  that runs before anything else (`guard.rs`), refusal and report of popups, downloads, file choosers, permission
  prompts and page dialogs, and reload after a renderer crash (`helper.rs`). On macOS the sub-processes run from five
  helper bundles (`bundle-macos.sh`) and enter Chromium's sandbox through `libcef_sandbox.dylib` first. On Windows the
  same code is a DLL (`cef-helper-win/`, exporting `RunConsoleMain`) started by CEF's `bootstrapc.exe` renamed to
  `spike-cef-helper.exe`, the arrangement Chromium's Windows sandbox needs since CEF M138.
- `app/`: the GPUI window. A canvas element draws the newest frame with `paint_image` and drops the previous image; the
  pane forwards pointer, wheel, key, text and input-method events; cursors and the `<select>` popup come from the helper;
  a stand-in permission card is drawn above the page. Scripted scenarios (`script.rs`) drive input through GPUI's own
  dispatch, and input-method compositions through GPUI's macOS `NSTextInputClient` view the way an input method calls it
  (`mac_ime.rs`); results are read back from the page with DevTools.
- `pages/`: local test pages (a form at fixed positions, a long page, an animation, a content-heavy page with 21,012 DOM
  nodes and 300 generated images).
- `unpack-cef.sh`, `build-cef-macos.sh`, `measure-macos.sh`, `ci-while-holding.sh`, and the temporary workflow
  `.github/workflows/spike-browser.yml`.

The CEF distribution for macOS (`cef_binary_156.0.30+g6028375+chromium-156.0.8078.96_macosarm64_minimal.tar.bz2`,
134,874,408 bytes) was downloaded once to the development machine with the maintainer's agreement, its SHA-256 and SHA-1
checked before unpacking, and unpacked by hand into the build folder in the layout the bindings expect, so the bindings'
build script fetched nothing. CMake and Ninja were installed for the bindings' C++ wrapper. CI downloads each platform's
archive on the runner, checks a pinned SHA-256 and unpacks it the same way.

## Results on macOS (hands-on, through the programs' own instrumentation)

Machine: an Apple silicon desktop (10 cores) with 16 GB of memory, macOS 26.6, release builds, under ordinary desktop
load. Most runs on a 4K panel at scale 2 and 60 Hz with a 1280 x 801 logical pane (2560 x 1602 device pixels); some on a
2560 x 1440 panel at scale 1 and 100 Hz. Every run used a fresh profile in the build folder. Commands are in
`spikes/browser/measure-macos.sh`; numbers are medians over 3 runs with the range in brackets unless stated.

### It works: real Chromium in the window, with input and an overlay

`spike-app --helper cef --page form --scenario input`: 34 of 34 checks passed at scale 2 and again at scale 1.

- Frames arrive in device pixels; the page reports `devicePixelRatio` 2; a 1-device-pixel checkerboard alternates in the
  frame and in GPUI's readback of the window (sharp at scale 2).
- The stand-in permission card reads back above the page; over the animation page, two readbacks half a second apart
  showed the page changing under the card (3 of 3 runs).
- Cursor shapes from the page (hand, I-beam, arrow); click focus; typing; Backspace; arrows; Tab; Enter; Command-A,
  Command-C and Command-V within the page (performed by the helper on the focused frame, because Chromium leaves those
  shortcuts to the application's menu on macOS).
- Input methods: Japanese and Chinese compositions, committed, cancelled and kept with `unmarkText`, with the page seeing
  `compositionstart`, `compositionupdate` and `compositionend`; the engine's character bounds reach GPUI's
  `firstRectForCharacterRange`. Exercised from GPUI's view inward; not exercised: a real input source, the candidate
  window, and keys passing through an active input method.
- Wheel scrolling with the right sign; the `<select>` popup through CEF's off-screen popup path, painted by the app and
  driven by keyboard; focus out and back in; pane resize; DevTools navigate and evaluate without a port; a renderer crash
  recovered by reload with frames resuming.
- Clean start and shutdown: Chromium's sub-processes were gone within 0.4 s of the app's exit in every run (about 45
  runs), and when the app aborted the helper saw its pipe close and exited cleanly. The re-launch guard never had to act.

### Frames, CPU and latency

| Scenario | Painted fps | Frame interval p50 / p95 / p99 ms | `paint_image` mean ms | UI thread per window frame p50 / p95 ms | App CPU % | Chromium processes CPU % |
|---|---|---|---|---|---|---|
| Steady wheel scrolling of a long local page, scale 2, 60 Hz (page's own rAF 60.1) | 59.5 (54.6-59.8) | 16.7 / 17.5 (17.3-33.3) / 19.1 (18.8-33.7) | 2.21 (2.19-3.05) | 2.54 / 3.07 | 25.0 (23.1-33.2) | 29.4 (27.7-33.6) |
| Animation page, scale 2, 60 Hz (page rAF 60.0) | 60.0 (59.9-60.0) | 16.7 / 17.1 / 17.7 | 2.48 (2.26-2.63) | 2.60 / 3.45 | 26.0 (24.8-26.8) | 30.2 (30.1-32.4) |
| Scrolling a public documentation page, scale 2 | 59.5 (56.1-59.9) | 16.7 / 17.4 (17.4-33.4) / 17.9 | 2.81 | 3.15 / 3.75 | 29.0 | 41.1 |
| Scrolling, scale 1, 100 Hz display, engine at 60 fps | 59.9 | 19.9 / 20.5 / 20.9 | 1.32 | 1.73 / 2.22 | 23.2 | 31.9 |
| Animation, scale 1, 100 Hz, engine at 100 fps (page rAF 100.0) | 96.8 (95.8-99.8) | 10.0 / 10.8 / 20.4 | 1.23 | 1.57 / 2.22 | 30.6 | 50.9 |
| Idle, a static page, 15 s | 0 frames, 0 to 2 window redraws | - | - | - | 0.1 | 1.0 |

- Per process while scrolling at scale 2: CEF's browser process 15 %, the GPU process 10 %, renderers 3 %, network and
  storage services 0.1 %. At idle: 0.4 %, 0.4 %, 0.01 % and 0.16 %. An idle page causes no periodic work in the app:
  no frames arrive and the window is not redrawn.
- Frames painted below 60 come from clock phase, not load: CEF paints on its own 60 Hz timer and the app on the
  display's, so sometimes two frames land in one refresh and one is replaced unpainted (one scroll run: 601 received,
  547 painted). On the scale-2 runs the painted rate never fell below 54 fps.
- The engine can run faster than 60: at `windowless_frame_rate` 100 the animation reached 96 to 100 fps on the 100 Hz
  display. Wheel scrolling stayed at 60 fps there (not explained).
- Key to frame (20 keys a run into a caret-less field, 3 runs, no misses): to the frame reaching the app p50 8.3 ms
  (7.2-14.2), p95 17.5 ms; to `paint_image` p50 22.0 ms (19.6-23.2), p95 38.0 ms (25.9-39.2), max 40.5 ms. Presentation
  adds up to one display frame; the OS's own key delivery is not included.
- Start-up, spawn to first frame (warm file cache): helper ready 175 to 220 ms, first frame 300 to 370 ms for a blank or
  simple page, painted 10 to 20 ms later; 2.2 to 3.7 s for the heavy page. The first runs after a build took up to 2.6 s
  to the browser and 2.8 s to the first frame (cold cache, observed, not controlled).
- Frame transport per frame at 2560 x 1602 (16.4 MB): a GPU-to-CPU readback inside Chromium (not measured), the
  helper's copy into shared memory 1.7 to 2.5 ms, the app's copy out on its reader thread 0.8 to 1.05 ms, and the upload
  into GPUI's atlas inside `paint_image` 2.2 ms on the UI thread. At 1280 x 801: 0.6 to 0.9, 0.35 and 1.3 ms.

### Memory (footprint, after every tab loaded, 3 runs)

| | Chromium processes total | Browser process | GPU process | Renderers | Network and storage | App |
|---|---|---|---|---|---|---|
| 1 blank tab | 265 MB (264-268) | 70 | 91 | 58 (the tab and a spare) | 46 | 177 |
| 1 content-heavy local tab | 793 MB (653-810) | 84 | 174 | 509 (349-509) | 44 | 176 |
| 3 content-heavy tabs (1 visible) | 1,473 MB (1,331-1,476) | 111 | 215 | 1,103 (4 processes) | 44 | 177 |
| 1 public documentation page (1 run) | 362 MB | 69 | 151 | 93 | 49 | 240 |
| While scrolling at scale 2 | 506 MB | 72 | 294 | 93 | 46 | 258 |

The app's footprint rises from about 124 MB idle to about 258 MB while frames stream at scale 2 (GPUI's atlas textures).
A `ps` and `footprint` cross-check agreed with the app's own figures.

### Size of the component (macOS arm64)

What the app would ship: the helper's bundle with the framework and the five helper bundles. 342.5 MB on disk as built
(the framework binary alone 238 MB); 288.8 MB with only the English locale and stripped helper binaries. Compressed as
built: xz 103.0 MB, zip 150.8 MB, zstd -19 116.9 MB; trimmed: xz 95.0 MB, zip 137.2 MB. The Linux and Windows archives
are 333 MB and 175 MB compressed as published; their unpacked sizes are printed by CI (pending).

### The shared-texture path on macOS

Partly tried. CEF 156 on macOS does deliver `OnAcceleratedPaint` frames (BGRA, an IOSurface, about 60 a second), although
its macOS header still says Windows only. Drawing them is not reachable without changing GPUI: GPUI's surface element
accepts only NV12 pixel buffers (a 4:2:0 conversion would blur coloured text), and the IOSurface lives in the helper
process, so it would also need a Mach-port channel next to the pipe. What it would take: BGRA support in GPUI's Metal
surface path (a format branch and a shader), an IOSurface hand-off by Mach port, and a fork of the pinned snapshot.

## Results on Linux and Windows (CI only; pending the first run)

The workflow builds the real helper on all three runners with CEF downloaded and checked on the runner, then runs a
headless smoke check: the helper starts off-screen, loads a page, produces at least one frame into the shared-memory
transport, answers a DevTools evaluate, reports each sub-process's sandbox state, and exits by itself with success; while
it runs, listening TCP sockets are listed (P32).

- Linux: under a virtual display, a tarball-style run (no setuid helper; on Ubuntu 24.04 and later it is expected to meet
  the AppArmor restriction on user namespaces and fail, recorded as a finding) and an installed `.deb` with
  `chrome-sandbox` owned by root with the setuid bit, which must show every renderer, GPU and utility process with a
  seccomp filter. Unverified until the run.
- Windows: the bootstrap arrangement (CEF's `bootstrapc.exe` loading the helper DLL). The helper logs whether the
  bootstrap handed it sandbox information; the processes' sandbox state is not read on Windows, so a pass shows the
  arrangement starts, not that every sub-process is sandboxed. Unverified until the run.
- Not exercisable in CI on either: frame pacing, input, input methods and the overlay (shared runners have no GPU and no
  person), which are the same code as on macOS and are judged from there.

### Windows: the OS web view, from documentation

WebView2 speaks the DevTools Protocol in-process (`CallDevToolsProtocolMethod`), so no port; it is patched by the OS, so
nothing is downloaded and nothing has to be kept current; and its sandbox is the OS runtime's. But it can sit under
GPUI-drawn UI only through visual hosting into a DirectComposition tree, and GPUI's Windows renderer keeps its
composition device and visual private, so that needs a GPUI change; as a windowed control it covers the ask cards and
popovers the design requires above the page. It also offers no frame stream (only single-image captures). Assessment:
WebView2 is the better Windows engine only if GPUI exposes its composition tree; until then it is a fallback (embedded
but without overlays), better than an external window. Not built.

## Decision

Option A everywhere, with a fallback per platform:

- **macOS: embed with CEF off-screen in a helper process.** Verified hands-on: Chromium's sandbox on in every
  sub-process, every input check passing, the overlay above a live page, 60 frames a second at scale 2 with about 2.2 ms
  of UI thread per frame, 22 ms key to frame.
- **Linux: embed with CEF off-screen, shipped only in the `.deb`**, which installs `chrome-sandbox` setuid root; the
  tarball (and AppImage) falls back to an external browser window. Unverified: CI only, and the CI run is pending.
- **Windows: embed with CEF off-screen under CEF's bootstrap**, the same code as macOS as a DLL. Unverified: CI only, and
  the CI run is pending. If the bootstrap arrangement cannot host the off-screen helper with the sandbox on, Windows uses
  WebView2 as a windowed view (no overlays) before an external window.

Proposed targets for the browser component (section 9 left them to this trial), measured on the reference machine at
each gate:

| Target | Value | Measured here |
|---|---|---|
| Painted frame rate while scrolling or animating a 1280 x 800 pane at scale 2 on a 60 Hz display | median of runs at least 57 fps, no run under 50 fps | 59.5 to 60.0 (lowest run 54.6) |
| Frame interval p95 | at most 2 display refreshes now (34 ms at 60 Hz); 1 refresh (17.5 ms) once frame production follows the display | 17.1 to 33.4 |
| UI thread per window frame with a streaming pane, p95 | at most 5 ms at 2560 x 1600 | 3.0 to 4.3 |
| Key to frame painted | p50 at most 25 ms, p95 at most 40 ms | 22.0 and 38.0 |
| Idle | no frames, no app redraws, Chromium processes at most 2 % CPU together | 0, 0 to 2 redraws in 15 s, 1.0 % |
| Memory, Chromium processes | at most 300 MB with one blank tab; at most 450 MB with one typical documentation page; heavy pages measured, not capped | 265, 362 |
| Memory, the app's frame path | at most 150 MB above the app's idle footprint while a scale-2 pane streams | about 134 |
| Start-up, helper spawn to first frame of a simple page | at most 500 ms warm | 300 to 370 |
| Download size (macOS arm64) | at most 100 MB compressed, at most 300 MB on disk | 95 and 289 trimmed |

## Who decided

Design call from the M0 trial, within the maintainer's decision 0004. Downloading the CEF distribution to the
development machine and installing CMake and Ninja were the maintainer's approvals.

## Consequences

### What the application must ship, per platform

- **macOS:** a helper app bundle containing the CEF framework and five helper bundles (`<name> Helper.app`, `(GPU)`,
  `(Renderer)`, `(Plugin)`, `(Alerts)`), each an `LSUIElement`; the bundle runs from anywhere (the trial ran it from the
  build folder), so it can live in the app's support folder after a first-use download. It contains the project's own
  helper code, so the project must build, sign (hardened runtime, with the entitlements Chromium's helpers need) and
  notarize it; it cannot be a raw download from CEF's host.
- **Linux:** `libcef.so`, its resources and locales, the helper executable (found beside `libcef.so` through an
  `$ORIGIN` run path, because the setuid sandbox drops `LD_LIBRARY_PATH`), and `chrome-sandbox` owned by root with the
  setuid bit. A component downloaded by the user after installation cannot carry a setuid-root file, so the `.deb` must
  install `chrome-sandbox` itself (or the whole component) even if the rest is fetched on first use. Where unprivileged
  user namespaces are allowed (not default Ubuntu 24.04 and later) Chromium uses them instead. The tarball cannot run the
  sandbox on default Ubuntu and ships no embedded browser; AppImage likewise.
- **Windows:** `libcef.dll` and resources, the helper as a DLL, and CEF's bootstrap executable renamed to the helper's
  name, all signed by the project; the bootstrap sets up the sandbox and calls the DLL (unverified until CI runs).
- Everywhere: CEF's `CREDITS.html` (Chromium's third-party notices) and CEF's licence.

### Keeping the bundled Chromium current

CEF follows Chromium stable about every four weeks, and the bindings followed within days (six releases of the 156 line
in three and a half weeks). Each update means: bump the pinned `cef` version, pin three new SHA-256 values, rebuild the
helper and CEF's C++ wrapper (CMake and Ninja in CI), sign and notarize the macOS bundle, rebuild the `.deb`, sign the
Windows bootstrap and DLL, run the smoke check on three platforms and the input scenario on macOS, and publish a
component of about 95 to 105 MB per platform (no delta updates without building them). With the design's 60-day warning
this is a recurring release task, not a one-off.

### Security against the threat model's browser boundary

Achieved (macOS verified, others pending CI):
- The helper is its own process; every renderer, GPU and utility process reported itself sandboxed (`sandbox_check`) in
  every run; switches that turn the sandbox off or open a debugging port are refused by the helper.
- P32: DevTools runs through CEF's `SendDevToolsMessage`; no listening TCP socket existed in the app or the helper tree
  while it ran (lsof).
- P53 in part: the channel is the helper's inherited stdin and stdout plus an unnamed shared-memory region; Chromium's
  sub-processes inherit neither (their descriptors 0 and 1 are `/dev/null` and the log, and the region is closed on exec,
  checked with lsof); frame dimensions and line lengths are checked before use.
- P35 in part: popup windows, downloads, file choosers, permission prompts and page dialogs reach the helper and are
  refused there, so the app can decide each one; a dedicated profile in the build folder.

Not achieved or not tried:
- CEF's browser process is not sandboxed (by Chromium's design); confining it (P33) was not tried.
- P52: Chromium's network service contacted Google's servers on its own while showing a local page (the component
  updater, network time and a search endpoint, named by Chromium's own net log), and `--disable-background-networking`,
  `--disable-component-update` and `--no-pings` did not stop it. The profile hardening P52 asks for needs another means
  (CEF settings or policies, or blocking at the network level), still to be found. Page clipboard writes: CEF has no
  clipboard hook; Command-C and Command-V pass through the helper (interceptable), and a browser setting can deny
  script access to the clipboard (`javascript_access_clipboard`, not tried).
- P53: the messages are JSON with size limits but no schema, rate limits or fuzzing; a binary schema with a fuzzed
  decoder is for the real implementation.

### What does not work, or was not reached

- Proprietary codecs: `canPlayType` says no for H.264, AAC and HEVC (VP9, AV1 and Opus play), as expected of CEF's
  published builds; no H.264 file was played, since none could be made or downloaded here.
- Camera: the request failed in the engine with `NotFoundError` before any permission was asked.
- Printing: not reached (the print path needs a `CefPrintHandler`; with none, the helper could open a native dialog).
- Drag and drop: not reached (needs `StartDragging` and the drag-target calls, with GPUI's drag events forwarded).
- Accessibility of page content: not reached (CEF's off-screen accessibility handler would have to feed GPUI's, whose
  screen-reader support is itself unconfirmed).
- A click without a hold between press and release did not open a `<select>` popup; a person's click does.
- Frame production is not aligned with the display, so a few frames are replaced unpainted; aligning them needs CEF's
  external begin frames driven by the app (which wakes the app every refresh unless the page's need for frames can be
  known) or a one-frame queue (adds up to a frame of latency).
- 120 Hz displays were not tried; 100 Hz worked for an animation at the engine's raised frame rate.

### What changing GPUI would buy and cost on Linux and Windows

An external-texture element in GPUI's DirectX renderer (Windows: a DXGI shared handle duplicated into the app and opened
as a D3D11 texture) and its `wgpu` renderer (Linux: dmabuf planes passed over an inherited socket pair and imported as
Vulkan external memory) would remove both CPU copies and the atlas upload: about 2 to 3 ms of the UI thread and 3 to 4 ms
of other threads per 2560 x 1600 frame at 60 Hz, and the memory GPUI's atlas holds for frames, and would make full-screen
and high-refresh panes affordable. The cost is a fork of the pinned GPUI snapshot (decision 0007) carried across every
upgrade, in renderers this project does not otherwise touch, plus CEF's shared textures on Linux, which are claimed but
unverified. On Windows the same fork could expose GPUI's DirectComposition tree, which is what WebView2 visual hosting
needs. Not worth it for M0's targets; worth revisiting if the targets tighten (high-refresh or full-screen panes).

### Patterns worth keeping for the real implementation

- `protocol/src/view.rs`: the `BrowserView` trait, with no GPUI types.
- `protocol/src/shm.rs` and `wire.rs`: frames in an inherited, unnamed shared-memory region with slot ownership passed by
  messages; frame dimensions checked before pixels are touched.
- `protocol/src/helper.rs` and the frame sink in `app/src/main.rs`: pipes read and written off the UI thread; the newest
  frame waits in a single slot so a slow UI drops frames instead of queueing them; the previous image is dropped from
  GPUI's atlas.
- `cef-helper/src/guard.rs`: the re-launch guard, before anything else runs.
- `cef-helper/src/mac.rs`: the helper owns CEF's `NSApplication`, so GPUI's application object is never touched.
- `cef-helper/src/helper.rs`: messages queued until the browser exists; the view size stored by the reader thread;
  macOS edit shortcuts performed on the focused frame; input-method calls with an explicit invalid range (a null range
  is silently ignored by libcef: the first IME runs failed on exactly this); refusals reported to the app; reload after a
  renderer crash; descriptors 0 and 1 moved off the pipes and the region closed on exec before CEF starts.
- `cef-helper/src/keys.rs`: explicit key codes (the precedent's special-key bug did not occur).
- `app/src/main.rs`: the pane's hitbox inserted in prepaint so overlays occlude it; "pointer left" sent when the hitbox
  stops being hovered after a move, not from GPUI's hover state.
- `app/src/mac_ime.rs` and `app/src/script.rs`: testing input methods through GPUI's own text-input client and reading
  results back with DevTools; `protocol/src/tree.rs`: verifying the sandbox per process instead of assuming it.
- `unpack-cef.sh`, `cef-helper/bundle-macos.sh` and the `.deb` step in the workflow: pinned, hash-checked CEF with no
  download by the build script; the macOS helper bundles; the setuid `chrome-sandbox` packaging.

## Revisit when

The first CI run of `spike-browser.yml` reports on Linux and Windows (this record then gets a follow-up decision if
either fails); a person runs the browser on real Linux or Windows hardware; GPUI gains an external-texture element on
Linux and Windows, BGRA surfaces on macOS, or an exposed composition tree on Windows; a way is found to stop Chromium's
background traffic to Google (P52); the targets above are missed at a gate; 120 Hz panes become a requirement; or CEF
changes its off-screen rendering, its Windows sandbox arrangement or its macOS bundle layout.
