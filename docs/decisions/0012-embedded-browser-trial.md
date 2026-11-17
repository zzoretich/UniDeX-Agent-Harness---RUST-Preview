# 0012: Embedded browser trial

Status: accepted with open questions (the Linux GPU process's sandbox, the unsandboxed network service on Linux and
Windows, confining CEF's browser process (P33), containing page clipboard writes (P52), and a control for Chromium's
background traffic, without which the browser does not ship)
Date: 2026-11-17

This is the record of M0 Task 12, the trial of the riskiest unknown in the project: whether Chromium can be embedded
inside a GPUI window on macOS, Linux and Windows by rendering it off-screen in a helper process and drawing its frames as
an element. Phase A did the research and the trial code that needs no Chromium. Phase B ran real Chromium on the
development Mac and measured it through the programs' own instrumentation (scripted runs; no person operated it), and
wrote the CI workflow that builds and checks the helper on Linux and Windows. A fix round corrected this record after an
independent review and took Linux and Windows through CI as far as a bounded effort allowed; what CI built, ran and
could not exercise is in "Results on Linux and Windows". The open questions are listed in the status line and under
"What does not work".

## Context

Decision 0004 embeds Chromium off-screen in a helper process, drawn as an element, so app UI (ask cards, popovers, the
command palette) can sit above a page. Design section 5.8 adds a `BrowserView` trait, CEF as the first engine, pointer,
keyboard, text entry, cursor and popup forwarding, pages driven over the Chrome DevTools Protocol, Chromium's own sandbox
on everywhere, the component fetched on first use as a version-pinned, hash-checked download, and an external-window
fallback per platform. Section 8 allows the OS web view on Windows and sets the platform tiers (Tier 1, macOS: a
hands-on check by the maintainer at every gate; Tier 2, Linux and Windows: verified by CI only). The threat model (4.9)
requires a portless DevTools channel (P32) and a helper channel that is an inherited anonymous pipe plus shared memory,
never a named socket (P53), and asks the M0 trial two questions: whether CEF's browser process can be confined while
Chromium's sandbox still engages (P33), and whether page clipboard writes can be kept inside the pane (P52). GPUI is
pinned to the `gpui-pre 0.3.12` snapshot (decision 0007), whose `surface()` element, the only way it draws external
pixel buffers, is macOS-only and takes only NV12 buffers.

The closest precedent, a crate by the widget kit's own maintainers that wraps CEF off-screen rendering for GPUI (`wef`,
Apache-2.0), is described by its authors as experimental and not good enough; they ship the system web view instead. Its
open issues: special keys did nothing on macOS, its init re-ran the program's `main` in a loop, video sites failed. No
code was copied from it.

Facts below were checked on 2026-11-17 against the sources, logs or CI runs named; "unverified" marks what only
documentation or an unrun check supports.

## Options considered

| Option | Trade-off |
|---|---|
| A. CEF off-screen in a helper process, CPU frames through shared memory, drawn as GPUI images | Runs with the pinned GPUI on macOS (measured). On Linux and Windows, GPUI's CPU image path is the only one this snapshot has; in CI the GPUI app builds there and the CEF helper ran headless, sending frames through shared memory, but nothing was drawn in a window there. Each frame is copied twice on the CPU and uploaded to the GPU on the app's UI thread. Chosen. |
| B. As A, but CEF's shared textures (IOSurface, DXGI handle, dmabuf) drawn directly | Removes the CPU copies and the upload; needs an external-texture element in GPUI on every platform (a fork of the pinned snapshot) and a handle channel between the processes. |
| C. CEF as a native child view | Rejected in 0004: it sits above everything GPUI draws. |
| D. Windows only: the OS web view (WebView2) behind the same trait | OS-patched, DevTools without a port; under app-drawn UI only through DirectComposition visual hosting, which needs a GPUI change; as a windowed view it covers the overlays. |
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
- `unpack-cef.sh`, `build-cef-macos.sh`, `measure-macos.sh`, the CI scripts `ci-while-holding.sh`, `ci-linux-run.sh`
  and `ci-windows-tree.ps1`, and the temporary workflow `.github/workflows/spike-browser.yml`.

The CEF distribution for macOS (`cef_binary_156.0.30+g6028375+chromium-156.0.8078.96_macosarm64_minimal.tar.bz2`,
134,874,408 bytes) was downloaded once to the development machine with the maintainer's agreement, its SHA-256 and SHA-1
checked before unpacking, and unpacked by hand into the build folder in the layout the bindings expect, so the bindings'
build script fetched nothing. CMake and Ninja were installed for the bindings' C++ wrapper. CI downloads each platform's
archive on the runner, checks a pinned SHA-256 and unpacks it the same way.

### Sources and provenance

Read as reference: the `cef` 156.6.0 and `cef-dll-sys` bindings and `download-cef` (Apache-2.0 OR MIT): the README, the
build script, the macOS application object and bundle helpers, the `cefsimple` and `osr` examples, and the generated
bindings for macOS, Linux and Windows; CEF's headers, CMake files and `cef_sandbox_win.h` in the binary distribution,
under CEF's own licence (BSD-3-Clause-style; Chromium is BSD-3-Clause, with third-party notices in `CREDITS.html`);
CEF's general-usage and sandbox-setup pages; Chromium's Linux sandbox, setuid-sandbox and AppArmor user-namespace
documents; Microsoft's WebView2 pages on the DevTools protocol and on windowed and visual hosting; the pinned `gpui-pre`
0.3.12 crates (Apache-2.0); the precedent crate `wef` (Apache-2.0: README, issues and `crates/webview/src`, in phase A
only) and the README of `bevy_cef`; crates.io's API for versions and dates; the checksum files on CEF's build host; and
the logs of the trial's CI runs. The trial's code was written for the trial against those crates' public APIs; the
trial's report records no copied code. The sources read include none of the editor project's crates that are not
permissively licensed.

## Results on macOS (scripted checks on the development Mac)

Everything in this section was verified on the development Mac by scripted checks through the program's own
instrumentation; no person operated it, and the maintainer's hands-on check (Tier 1) is still owed.

Machine: an Apple silicon desktop (10 cores) with 16 GB of memory and external displays, macOS 26.6, release builds,
under ordinary desktop load. It is not the design's reference machine, which is a laptop whose display may run at
120 Hz. Most runs used a 4K panel at scale 2 and 60 Hz with a 1280 x 801 logical pane (2560 x 1602 device pixels); some
used a 2560 x 1440 panel at scale 1 and 100 Hz; the two complete input runs used a single display that GPUI reported as
1920 x 1080 at scale 1 (its refresh rate is not in the logs). Every run used a fresh profile in the build folder.
Commands are in `spikes/browser/measure-macos.sh`; numbers are medians over 3 runs with the range in brackets unless
stated. MB means 10^6 bytes throughout.

### It works: real Chromium in the window, with input and an overlay

`spike-app --helper cef --page form --scenario input` runs 34 scripted checks. What the logs show:

- At scale 2 (the 4K panel), the best run passed 32 of 34. The two failures, `backspace-and-left-arrow` and
  `cmd-a-cmd-c-cmd-v`, came from a bug in the script, not the browser: GPUI's layout-unaware keystroke parser turned the
  script's `!` into `1`, so the field held "hello1 " instead of "hello! ". The script was then fixed to send `!` as a
  US-layout event carries it.
- After the fix, two runs at scale 1 (the single 1920 x 1080 display above) passed 34 of 34. The full set was not re-run
  at scale 2 after the fix.
- Three of the 34 are weaker than their names: `ime-cancel` passes when the field is unchanged, so it also passes if the
  cancel never reached the page; `refusal-b-media` passes because the engine failed the camera request with
  `NotFoundError` before any permission prompt, so no refusal happened; and the codec check is not an input check.

What passed, read back from the page with DevTools `Runtime.evaluate`:

- Frames arrive in device pixels; at scale 2 the page reports `devicePixelRatio` 2 and a 1-device-pixel checkerboard
  alternates in the frame and in GPUI's readback of the window (sharp at scale 2).
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

"UI thread per window frame" runs from the start of the pane's `render` to the end of the last element's paint, so it
includes layout, prepaint and `paint_image`, and excludes GPUI's scene submission to the GPU and presentation.

| Scenario | Painted fps | Frame interval p50 / p95 / p99 ms | `paint_image` mean ms | UI thread per window frame p50 / p95 ms | App CPU % | Chromium processes CPU % |
|---|---|---|---|---|---|---|
| Steady wheel scrolling of a long local page, scale 2, 60 Hz (page's own rAF 60.1) | 59.5 (54.6-59.8) | 16.7 / 17.5 (17.3-33.3) / 19.1 (18.8-33.7) | 2.21 (2.19-3.05) | 2.54 / 3.07 | 25.0 (23.1-33.2) | 29.4 (27.7-33.6) |
| Animation page, scale 2, 60 Hz (page rAF 60.0) | 60.0 (59.9-60.0) | 16.7 / 17.1 / 17.7 | 2.48 (2.26-2.63) | 2.60 / 3.45 | 26.0 (24.8-26.8) | 30.2 (30.1-32.4) |
| Scrolling a public documentation page, scale 2 | 59.5 (56.1-59.9) | 16.7 / 17.4 (17.4-33.4) / 17.9 | 2.81 | 3.15 / 3.75 | 29.0 | 41.1 |
| Scrolling, scale 1, 100 Hz display, engine at 60 fps | 59.9 | 19.9 / 20.5 / 20.9 | 1.32 | 1.73 / 2.22 | 23.2 | 31.9 |
| Animation, scale 1, 100 Hz, engine at 100 fps (page rAF 100.0) | 96.8 (95.8-99.8) | 10.0 / 10.8 / 20.4 | 1.23 | 1.57 / 2.22 | 30.6 | 50.9 |
| Idle: the static form page, nothing focused, no timers; 15 s after a 3 s settle | 0 frames; 0, 2 and 0 window redraws | - | - | - | 0.1 | 1.0 |

- Per process while scrolling at scale 2: CEF's browser process 15 %, the GPU process 10 %, renderers 3 %, network and
  storage services 0.1 %. At idle: 0.4 %, 0.4 %, 0.01 % and 0.16 %. In the idle case above no frames arrived; the window
  was redrawn 0, 2 and 0 times in the three runs (the 2 are not explained).
- Frames painted below 60 come from clock phase, not load: CEF paints on its own 60 Hz timer and the app on the
  display's, so sometimes two frames land in one refresh and one is replaced unpainted (one scroll run: 601 received,
  547 painted). On the scale-2 runs the painted rate never fell below 54 fps.
- The engine can run faster than 60: at `windowless_frame_rate` 100 the animation reached 96 to 100 fps on the 100 Hz
  display. Wheel scrolling stayed at 60 fps there (not explained).
- Key to frame (20 keys a run into a caret-less field, 3 runs, no misses): to the frame reaching the app p50 8.3 ms
  (7.2-14.2), p95 17.5 ms; to `paint_image` p50 22.0 ms (19.6-23.2), p95 38.0 ms (25.9-39.2), max 40.5 ms. With 20 keys
  a run, a run's p95 is its 19th value. Presentation adds up to one display frame; the OS's own key delivery is not
  included. In these runs a frame arrives only on a key press, and the UI thread per window frame had a p95 of 5.2 to
  7.7 ms, higher than in the streaming runs above.
- Start-up, spawn to first frame received, warm file cache, across all 23 runs of the measurement sets on simple local
  pages (blank, form, long and animation pages; the two runs that came first after a build are excluded): median 356 ms,
  range 288 to 980 ms, 6 of 23 over 500 ms. They fall into two groups: 17 runs at 288 to 398 ms and 6 at 539 to 980 ms
  (two latency runs at 977 and 966 ms, three scale-1 scroll runs at 941, 980 and 539 ms, one animation run at 618 ms).
  In four of the six the helper was also slow to create its browser (616 to 722 ms, against 167 to 230 ms in the fast
  group), so most of the time went before the page loaded; in the other two (618 and 539 ms) it went after. The logs do
  not show why. Painting the first frame took 2 to 40 ms more. The heavy page took 2.2 to 3.7 s (the page's own
  generation); the public documentation page 489 to 652 ms (network). The two runs first after a build took 2.2 and 2.8
  s to the first frame (cold cache, observed, not controlled).
- Frame transport per frame at 2560 x 1602 (16.4 MB): a GPU-to-CPU readback inside Chromium (not measured), the
  helper's copy into shared memory 1.7 to 2.5 ms, the app's copy out on its reader thread 0.8 to 1.05 ms, and the upload
  into GPUI's atlas inside `paint_image` 2.2 ms on the UI thread. At 1280 x 801: 0.6 to 0.9, 0.35 and 1.3 ms.

### Memory (footprint)

| | Chromium processes total | Browser process | GPU process | Renderers | Network and storage | App |
|---|---|---|---|---|---|---|
| 1 blank tab, at rest | 265 MB (264-268) | 70 | 91 | 58 (the tab and a spare) | 46 | 177 |
| 1 content-heavy local tab, at rest | 793 MB (653-810) | 84 | 174 | 509 (349-509) | 44 | 176 |
| 3 content-heavy tabs (1 visible), at rest | 1,473 MB (1,331-1,476) | 111 | 215 | 1,103 (4 processes) | 44 | 177 |
| 1 public documentation page, at rest (1 run) | 362 MB | 69 | 151 | 93 | 49 | 240 |
| While scrolling the long local page at scale 2 | 506 MB (505-509) | 72 | 294 | 93 | 46 | 258 (241-258) |
| While scrolling the documentation page at scale 2 | 607 MB (601-609) | 84 | 367 | 106 | 49 | 258 (258-274) |
| While the animation page streams at scale 2 | 451 MB (450-469) | 82 | 257 | 66 | 46 | 295 (292-309) |

"At rest" rows were sampled after every tab loaded, with nothing moving. All rows are at scale 2 with a 1280 x 801 pane.
The app's footprint was 124 MB (118 to 136) in the idle runs and rose while frames streamed: by 117 to 134 MB scrolling
the long page, 134 to 150 MB scrolling the documentation page, and 168 to 185 MB on the animation page. The rise is
probably GPUI's atlas textures (each frame wider than 1024 px gets its own atlas texture) plus the reader thread's frame
copies; nothing measured it. GPUI's atlas counters or a GPU allocation trace of the app would show it. Whether either
process's footprint counts the shared frame region was not established: the region reserves 110.6 MB (three slots of
3840 x 2400 x 4 bytes and a 4 KiB header), of which frames of this pane size write at most 3 x 16.4 MB. A `ps` and
`footprint` cross-check of one animation run agreed with the app's own figures.

### Size of the component (macOS arm64)

What the app would ship: the helper's bundle with the framework and the five helper bundles. On disk, by `du`: 342,552
KiB as built, 350.8 MB (the framework binary alone 238 MB); with only the English locale and stripped helper binaries,
288.8 thousand KiB as recorded, about 296 MB (the trimmed copy was deleted, so this is converted, not re-measured). The
trimmed figure assumes a component with English-only Chromium strings (all 220 locale folders together were about
51 MB as recorded) and excludes CEF's `CREDITS.html` (5.3 MB), which must ship; with it, about 301 MB. Compressed, as recorded: as
built, xz -9 103.0, zip 150.8, zstd -19 116.9; trimmed, xz -9 95.0, zip 137.2. The report does not say whether those
compressed figures are bytes divided by a million or kibibytes divided by a thousand (as the on-disk figures were); in
the second case each is 2.4 % higher in MB (trimmed xz about 97.3 MB). Linux and Windows were not measured: their
minimal distribution archives are 333.3 MB and 175.2 MB as published, which include build files the component would not
ship, and are the only data for those platforms.

### The shared-texture path on macOS

Partly tried. CEF 156 on macOS does deliver `OnAcceleratedPaint` frames (BGRA, an IOSurface, about 60 a second), although
its macOS header still says Windows only. Drawing them is not reachable without changing GPUI: GPUI's surface element
accepts only NV12 pixel buffers (a 4:2:0 conversion would blur coloured text), and the IOSurface lives in the helper
process, so it would also need a Mach-port channel next to the pipe. What it would take: BGRA support in GPUI's Metal
surface path (a format branch and a shader), an IOSurface hand-off by Mach port, and a fork of the pinned snapshot.

## Results on Linux and Windows (CI only)

The workflow `spike-browser.yml` builds the real helper on all three runners with CEF downloaded and checked on the
runner, then runs a headless smoke check: the helper starts off-screen, loads a page, produces at least one frame into
the shared-memory transport, answers a DevTools evaluate, reports each sub-process's sandbox state, and exits by itself
with success; while it runs, the trial's processes and listening TCP sockets are listed (P32). The outcome of the
socket check and of the Linux tarball-style run is read from the job log; from this fix round on, each also appears as
a warning annotation and in the step summary. No step passes a switch that turns Chromium's sandbox off or weakens it.

What the sandbox check tests, per platform:
- macOS: every process but CEF's browser process reports itself sandboxed (`sandbox_check`), and a renderer exists.
- Linux: every process but CEF's browser process reports a seccomp-bpf filter (`Seccomp: 2` in `/proc/<pid>/status`),
  and a renderer exists, except the processes that only launch sandboxed ones (the setuid `chrome-sandbox` helper and
  the zygotes, whose forked children install the filter themselves), Chromium's crash handler, and any process Chromium
  itself starts with `--service-sandbox-type=none`. Each exempt process is still listed with its state, with its
  namespaces and thread names where the kernel shows them. A process forked from a zygote keeps the zygote's command
  line unless Chromium can rewrite its process title, so run 2's check took every forked child for a zygote. Since run 3
  the helper hands CEF the process's own argument memory on Linux, so Chromium's titles (`--type=renderer`,
  `--type=gpu-process` and so on) are visible; as a fallback a zygote is told apart by its single thread and a forked
  child's role is read from its thread names; every forked child is required. A required process without the filter is
  named in the failure and in an error annotation (first exercised in run 44147154193 on commit 74cb34d, where it named
  the GPU process in both confined routes). Phase B's check required the filter of every process but the browser, which
  the zygotes would fail although Chromium's design gives them none. The check runs three ways: as a tarball would
  install it (no setuid helper, no AppArmor profile); installed as a `.deb` with `chrome-sandbox` setuid root; and with
  an AppArmor profile that lets the helper create user namespaces (an unconfined profile, `flags=(unconfined)`, granting
  the `userns` permission, as Chromium documents). The last two are the two ways Chromium documents to have its sandbox
  on Ubuntu 23.10 and later.
- Windows: the process tree is not read by the smoke check; while it holds the helper, a script reads each process's
  integrity level, whether its token is restricted, and whether it runs in an AppContainer and a job. Since run 3 it
  applies the same rule as on Linux: every process but the browser process and those Chromium starts with
  `--service-sandbox-type=none` at Untrusted or Low integrity with a restricted token or an AppContainer, and a renderer
  exists (run 2 required only the renderers).

Not exercisable in CI on either platform: frame pacing, the overlay drawn in a window, input and input methods (shared
runners have no GPU, no desktop session and no person). They are not judged from macOS: the key mapping, the Command
shortcuts and `mac_ime.rs` are macOS-specific, and Linux and Windows input methods take different paths through GPUI. On
Linux and Windows they are unverified, and so is everything the GPUI app does there: it was built, never run.

Run 1, 44129982414 on commit c4996d6:

- `stand-in` on Linux (ubuntu-26.04) and Windows (windows-2025), jobs 134204070400 and 134204070636: passed. The format
  check passed; the protocol, the stand-in and the GPUI app were built in release (with GPUI's Linux platform crate and
  its `wgpu` renderer, and GPUI's Windows platform crate); the smoke check ran the stand-in through the real pipes and
  shared memory without a display: 60 frames of 800 x 600 with the stand-in's corner marker intact, both DevTools
  answers, and the stand-in's own exit with status 0. On Windows this also shows the inheritable unnamed file mapping
  reaching the child through the standard library's process spawning.
- `cef helper` on Linux, job 134204070160: the CEF archive downloaded, its SHA-256 matched and it unpacked; the
  bindings' build script used the unpacked copy (on Linux it builds no C++ wrapper and links `libcef.so` directly). The
  helper did not compile: two macOS-only parts of the bindings were used unconditionally (the cursor handle's type in
  `OnCursorChange` and the IOSurface field of the shared-texture information).
- `cef helper` on Windows, job 134204070580: the archive downloaded, matched its SHA-256 and unpacked. The bindings'
  build script failed building CEF's C++ wrapper: the compiler stopped with `fatal error C1083: Cannot open compiler
  generated file: '': Invalid argument` on three generated test sources. Every object file whose full path was 262
  characters or longer failed and every one of 258 or shorter compiled: Windows' 260-character path limit, reached
  because the bindings tell CMake not to shorten object file names (`CMAKE_OBJECT_PATH_MAX` 500) and the job's build
  folder was 152 characters deep. The runner has `LongPathsEnabled` set (run 2 printed it), so the compiler does not
  honour long paths. A 12-character build folder fixed it.
- macOS jobs: cancelled by the CI provider because no hosted macOS runner picked them up (capacity, not a result).

Run 2, 44139437250 on commit fb15a7c (after the fixes above; also found: under this cargo's build-script layout the
bindings copy CEF's runtime files to `release/build`, not beside the helper, so the workflow places them):

- `cef helper` on Windows, job 134237859658: passed, and exercised. CEF's `bootstrapc.exe` (in the minimal distribution
  beside `bootstrap.exe`), renamed `spike-cef-helper.exe`, loaded the helper DLL, which logged "Windows bootstrap
  sandbox information present"; the page loaded, one 800 x 600 frame crossed the shared memory (first after 0.9 s),
  `Page.navigate` and `Runtime.evaluate` answered (the title "smoke"), and the helper exited by itself with code 0. The
  process probe, while the smoke check held the helper: two renderers and the storage service at Untrusted integrity
  with restricted tokens, the GPU process at Low integrity with a restricted token, all in a job; the network service
  started by Chromium with `--service-sandbox-type=none`, unrestricted, at the runner account's High integrity; CEF's
  browser process also at High integrity, in a job, unrestricted (the hosted runner's account is an elevated
  administrator; on a normal account it would be Medium, which is expected, not observed). The verdict enforced only the
  renderers in this run (2 of 2); the GPU and storage processes were listed and are sandboxed by the same measure. No
  listening TCP socket. Both outcomes appeared as warning annotations.
- `cef helper` on macOS (the hosted macOS 26 runner), job 134237859564: passed. The bundle ran; the GPU process, the
  network service (sandbox type `network`), the storage service and two renderers all reported `sandbox_check=1`; one
  frame (first after 3.1 s on the cold runner), both DevTools answers (the evaluate ran before the navigation finished
  and returned an empty title; the check requires an answer, not a value), exit status 0, no listening socket. This is
  the first run of the macOS helper on a machine other than the development Mac. What the runner's run contacted over
  the network is not in the logs.
- `cef helper` on Linux (Ubuntu 26.04, kernel 7.0, `kernel.apparmor_restrict_unprivileged_userns = 1`, AppArmor
  enabled), job 134237859162: failed, in the sandbox check only. The helper compiled and linked, and `ldd` found every
  library. Tarball-style: Chromium refused to start with "The SUID sandbox helper binary was found, but is not configured
  correctly. Rather than run without sandboxing I'm aborting now", the expected finding, reported as a warning. `.deb`
  with `chrome-sandbox` setuid root, and the AppArmor profile route: in both the helper started with Chromium's sandbox,
  loaded the page, sent a frame, answered both DevTools calls (the title "smoke") and exited with status 0; no listening
  socket. In the `.deb` run the setuid helper put the sandboxed zygote in its own PID and network namespaces; in the
  AppArmor run the zygotes had their own user, PID and network namespaces (the namespace sandbox). In each run three
  processes forked from the sandboxed zygote reported a seccomp-bpf filter (`Seccomp: 2`, one filter, `NoNewPrivs`).
  The check failed because these forked children still carried the zygote's command line and were exempted as zygotes,
  leaving no renderer identified: a fault of the check, not of the confinement. Two facts it surfaced: the network
  service runs unsandboxed (`--service-sandbox-type=none`), as on Windows; and in each run one process forked from the
  unsandboxed zygote had no seccomp filter (`Seccomp: 0`, `NoNewPrivs` 1, the browser's namespaces). Its role was not
  identified in run 2.
- `stand-in` on all three platforms (jobs 134237859630, 134237859708, 134237859882): passed as in run 1.

Run 3, 44143297916 on commit 8449f48:

- `cef helper` on Linux, job 134251574335: failed, on one process, in both confined routes. Chromium's process titles
  were now visible, so every process had its role. Tarball-style: refused again with the same message (warning, as
  designed). `.deb` with the setuid helper, and the AppArmor route: page, frame (first after 1.6 s and 0.8 s), both
  DevTools answers (the title "smoke"), exit status 0, no listening socket. Sandboxed with seccomp-bpf (`Seccomp: 2`,
  one filter, `NoNewPrivs` 1): both renderers and the storage service (`--service-sandbox-type=utility`), in the AppArmor
  route each in its own user, PID and network namespaces (in the `.deb` route the kernel would not show them; the zygote
  above them had its own PID and network namespaces). Not sandboxed: **the GPU process** (`--type=gpu-process`, forked
  from the unsandboxed zygote: `Seccomp: 0`, `NoNewPrivs` 1, the browser's namespaces), which the check requires, so the
  job fails on exactly that. Its thread names show it rendering in software through the system's Mesa `llvmpipe` (four
  `llvmpipe` threads) under the virtual display: the runner has no GPU. Chromium logged no sandbox error at its warning
  level, so the logs do not say whether Chromium tried the GPU sandbox and failed or did not try. Chromium's own account
  (`SystemInfo.getInfo`) could not be read: Chromium answers it only on the browser target, and CEF's DevTools channel is
  the page's. The network service ran unsandboxed by Chromium's choice (`--service-sandbox-type=none`), listed, not
  required.
- `cef helper` on Windows, job 134251574421: passed. The same picture as run 2 under the full rule: "4 of 4 required
  processes sandboxed" (GPU at Low integrity, storage and two renderers at Untrusted, all with restricted tokens), the
  network service and the browser process listed, not required; frame, DevTools answers, exit code 0, no listening
  socket.
- macOS jobs: cancelled again for lack of a hosted runner (capacity, not a result); the macOS evidence from CI is run 2.
- `stand-in` on Linux and Windows (jobs 134251573884 and 134251574408): passed.

Sizes printed by CI (`du`, in binary units): the minimal archives 144 MiB (macOS), 318 MiB (Linux) and 168 MiB
(Windows), as published; the unpacked distributions, including build-only files, 353 MiB, 1.5 GiB and 427 MiB. On Linux
`libcef.so` alone is 1,480,659,000 bytes as distributed (whether it carries debug information, and what stripping it
would save, was not checked); on Windows `libcef.dll` is 294,738,432 bytes. The macOS bundle built on the runner was
335 MiB, matching the development Mac's 342,552 KiB. A Linux or Windows component was not assembled or compressed, so
its download size is still not measured.

### Windows: the OS web view, from documentation

WebView2 speaks the DevTools Protocol in-process (`CallDevToolsProtocolMethod`), so no port; it is patched by the OS, so
the project keeps nothing current; nothing has to be downloaded, but only where the WebView2 Evergreen runtime is
already installed (otherwise the installer has to bundle or fetch it); and its sandbox is the OS runtime's. But it can sit
under GPUI-drawn UI only through visual hosting into a DirectComposition tree, and GPUI's Windows renderer keeps its
composition device and visual private, so that needs a GPUI change. As a windowed control it covers the ask cards and
popovers the design requires above the page, unless those are drawn in separate owned popup windows above the web
view's window (a route not assessed further: each overlay becomes its own top-level window, positioned and kept in step
with the pane). It also offers no frame stream (only single-image captures). Assessment: WebView2 is the better Windows
engine only if GPUI exposes its composition tree; until then it is a fallback (embedded, with overlays only as owned
popup windows or not at all), better than an external window. Not built.

## Decision

Option A everywhere, with a fallback per platform, and one condition for every platform: **the embedded browser does not
ship until a working control stops or routes Chromium's background traffic to its vendor's servers** (observed on the
development Mac; see "Security"; P52).

- **macOS: embed with CEF off-screen in a helper process.** Verified on the development Mac by scripted checks through
  the program's own instrumentation; no person operated it, and the maintainer's hands-on check is still owed. Shown:
  Chromium's sandbox on in every sub-process (the operating system's own per-process check), 34 of 34 input checks at
  scale 1 after the script fix (32 of 34 at scale 2 before it, the two failures the script's; three of the 34 are weaker
  than their names, see the input results), the overlay above a live
  page, about 60 frames a second at scale 2 with a UI thread per window frame of 2.54 ms median and 3.07 ms at the 95th
  percentile, and 22 ms median key to frame.
- **Linux: embed with CEF off-screen, shipped only in the `.deb`**, which installs `chrome-sandbox` setuid root or an
  AppArmor profile; the tarball (and AppImage) falls back to an external browser window. Not yet verified, and the
  browser does not ship on Linux until it is. What CI showed (runs 2 and 3): with either of the two, the helper ran
  headless, sent a frame through shared memory and answered DevTools, and the renderers and the storage service ran
  with seccomp-bpf filters inside Chromium's namespace or setuid sandbox; without either, Chromium refused to start. But
  the GPU process ran without a seccomp filter on the runner, which has no GPU and rendered through Mesa's software
  `llvmpipe`, so Chromium's sandbox was not on in every process. Questions to answer before the browser ships on Linux:
  does the GPU process run with its seccomp filter on a machine with a GPU (a check on real hardware or a self-hosted
  runner with a GPU; the hosted runners cannot show it), and if it does not, or where the machine has no usable GPU,
  which supported Chromium setting confines that process or removes it. Candidates, none tried: making a failed GPU
  sandbox fatal (`--gpu-sandbox-failures-fatal=yes`), starting the GPU sandbox before the GL context
  (`--gpu-sandbox-start-early`), and rendering through Chromium's own SwiftShader by ANGLE instead of the system's Mesa;
  whether `--disable-gpu` still leaves a GPU process, and how it is sandboxed then, is not known. Switches that move the
  GPU work into the unsandboxed browser process or turn the GPU sandbox off (`--in-process-gpu`,
  `--disable-gpu-sandbox`) are not candidates. The network service also runs unsandboxed by Chromium's choice (see
  "Security"). Nothing was drawn in a window and no input was tried.
- **Windows: embed with CEF off-screen under CEF's bootstrap**, the same code as macOS as a DLL. CI-verified in runs 2
  and 3 (headless only): the bootstrap loaded the helper DLL with sandbox information, the helper ran, sent a frame and
  answered DevTools; in run 3 every process the rule requires (the renderers, the GPU process and the storage service)
  ran at Untrusted or Low integrity with a restricted token (run 2 enforced the rule for the renderers only and listed
  the others in the same state); the network service runs unsandboxed by Chromium's choice. Unverified: anything drawn
  in a window, input and input methods, and a run under a normal (not elevated) account. WebView2 as a windowed view
  (overlays only as owned popup windows) stays the fallback before an external window, should the bootstrap arrangement
  fail later.

Proposed targets for the browser component (section 9 left them to this trial), measured on the reference machine at
each gate. The trial's figures come from a desktop with external displays at 60 and 100 Hz, not the reference laptop,
whose display may run at 120 Hz (8.3 ms a frame); the frame-rate, frame-interval, UI-thread and latency targets are
stated for 60 Hz and have not been tried at 120 Hz.

| Target | Value | Measured here |
|---|---|---|
| Painted frame rate while scrolling or animating a 1280 x 800 pane at scale 2 on a 60 Hz display | median of runs at least 57 fps, no run under 50 fps | 59.5 to 60.0 (lowest run 54.6) |
| Frame interval p95 | at most 2 display refreshes now (34 ms at 60 Hz); 1 refresh (17.5 ms) once frame production follows the display | 17.1 to 33.4 |
| UI thread per window frame (render to end of paint, excluding scene submission) with a streaming pane, p95 | at most 5 ms at 2560 x 1600 | 3.0 to 4.3 in the streaming runs; 5.2 to 7.7 in the latency runs, where a frame arrives only per key |
| Key to frame painted | p50 at most 25 ms, p95 at most 40 ms | p50 22.0, p95 38.0 (each run's p95 is its 19th of 20 keys; one run reached 39.2 ms; max 40.5) |
| Idle: a static page with no focused field, 15 s after a 3 s settle | no engine frames, at most 2 app redraws, Chromium processes at most 2 % CPU together | 0 frames, 0, 2 and 0 redraws, 1.0 % |
| Memory, Chromium processes, 1280 x 800 pane at scale 2 | at most 300 MB with one blank tab at rest; at most 450 MB with one typical documentation page at rest; at most 650 MB while scrolling one such page; heavy pages measured, not capped | 265; 362; 506 and 607 (601-609) |
| Memory, the app's frame path, 1280 x 800 pane at scale 2 | at most 200 MB above the app's idle footprint while the pane streams | 117 to 185 (scroll 117 to 134, documentation page 134 to 150, animation 168 to 185) |
| Start-up, helper spawn to first frame of a simple local page, warm | not yet supported by the measurements: median 356 ms, but 6 of 23 runs took 539 to 980 ms for a reason not found, and cold start was not measured | median 356, range 288 to 980 |
| Download size (macOS arm64), English-only component including `CREDITS.html` | at most 100 MB compressed, at most 320 MB on disk | 95.0 (xz -9, trimmed, without `CREDITS.html`; unit caveat above) and about 301 (with it) |

How these sit against section 9. Section 9 sets the application's targets "without the browser component" (download
under 40 MB, idle memory under 150 MB) and says the component is measured on its own and its memory does not count
against the app's 150 MB. The trial confirms the component cannot fit inside the application's figures: about 95 to
103 MB compressed and about 300 MB on disk for macOS alone, against a 40 MB download for the whole app; Linux and
Windows not measured. It also shows a cost section 9 does not place: while a pane streams, 117 to 185 MB of memory is
added inside the app's own process (the frame path: GPUI's atlas and the frame copies), which by section 9's wording
counts against the app's 150 MB, and the trial's minimal app was already at 124 MB idle. The options are the browser as
a separately downloaded component with its own stated budget, a per-tab cost (about 265 MB for a blank tab, 362 MB for a
documentation page at rest) and a per-pane cost in the app's process, or revised application targets. That is a
question for the maintainer, not decided here.

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
  `$ORIGIN` run path, because the setuid sandbox drops `LD_LIBRARY_PATH`), and Chromium's sandbox enabled one of the two
  documented ways: `chrome-sandbox` owned by root with the setuid bit, or an AppArmor profile that lets the helper
  create user namespaces (an unconfined profile with the `userns` permission; on Ubuntu 23.10 and later, where
  unprivileged user namespaces are restricted by AppArmor). Either needs root at install time: a component downloaded by
  the user after installation can carry neither a setuid-root file nor a system AppArmor profile, so the `.deb` must
  install `chrome-sandbox` or the profile itself (or the whole component) even if the rest is fetched on first use.
  Where unprivileged user namespaces are allowed (not default Ubuntu 24.04 and later) Chromium uses them without either.
  The tarball cannot run the sandbox on default Ubuntu and ships no embedded browser; AppImage likewise. Either way of
  enabling the sandbox confined the renderers and the storage service in CI, but not the GPU process on a runner without
  a GPU (see the Linux decision).
- **Windows:** `libcef.dll` and resources, the helper as a DLL, and CEF's bootstrap executable renamed to the helper's
  name, all signed by the project; the bootstrap sets up the sandbox and calls the DLL (unverified, see "Results on Linux
  and Windows").
- Everywhere: CEF's `CREDITS.html` (Chromium's third-party notices) and CEF's licence.

### Keeping the bundled Chromium current

CEF follows Chromium stable about every four weeks, and the bindings follow within days: crates.io lists eight releases
of the bindings' 156 line in 24 days (156.0.0 on 23 October to 156.6.0 on 16 November 2026), tracking seven CEF 156
builds. Each update means: bump the pinned `cef` version, pin three new SHA-256 values, rebuild the helper and CEF's C++
wrapper (CMake and Ninja in CI), sign and notarize the macOS bundle, rebuild the `.deb`, sign the Windows bootstrap and
DLL, run the smoke check on three platforms and the input scenario on macOS, and publish a component per platform: for
macOS arm64 about 95 to 103 MB compressed as measured (see the size section for the unit caveat); Linux and Windows not
measured (their distribution archives are 333.3 MB and 175.2 MB). No delta updates without building them. With the
design's 60-day warning this is a recurring release task, not a one-off.

### Security against the threat model's browser boundary

Achieved on macOS (Windows in CI as well, by its own per-process measure; Linux in CI except the GPU process, see
"Results on Linux and Windows"; on the hosted runners of all three platforms no process of the trial listened on TCP,
P32):
- The helper is its own process; every renderer, GPU and utility process reported itself sandboxed (`sandbox_check`,
  the operating system's own per-process check) in every run. That check is the evidence for the sandbox.
- P32: DevTools runs through CEF's `SendDevToolsMessage`; no listening TCP socket existed in the app or the helper tree
  while it ran (lsof).
- P53 in part: the channel is the helper's inherited stdin and stdout plus an unnamed shared-memory region; Chromium's
  sub-processes inherit neither (their descriptors 0 and 1 are `/dev/null` and the log, and the region is closed on exec,
  checked with lsof); frame dimensions and line lengths are checked before use.
- P35 in part: popup windows, downloads, file choosers, permission prompts and page dialogs reach the helper and are
  refused there, so the app can decide each one; a dedicated profile in the build folder.

What the helper passes to Chromium, all of it:
- Command-line switches, added in the browser process (`OnBeforeCommandLineProcessing`): `use-mock-keychain` (on macOS,
  a fixed test key instead of a Keychain-held key; see below); and `disable-background-networking`,
  `disable-component-update` and `no-pings`, meant to stop background fetches, which they did not (see below). The
  last three are left out when the helper is started with `--allow-background-network`, a measurement switch.
- CEF settings: windowless rendering on; `no_sandbox` 0 (Chromium's sandbox on); the profile and cache in the folder
  given by `--profile-dir`; CEF's log file there at warning level; `remote_debugging_port` 0 (no port). Browser
  settings: `windowless_frame_rate` (60 unless `--frame-rate` says otherwise). Window information: windowless, with
  shared textures only under `--probe-shared-texture`.
- The helper's own arguments, read by the helper and ignored by Chromium: `--shm-fd` or `--shm-handle`, `--shm-size`,
  `--profile-dir`, `--start-url`, `--tabs`, `--hidden-url`, `--probe-shared-texture`, `--frame-rate` and
  `--allow-background-network`.
- A narrow guard: the helper refuses to start as the browser process with a switch that turns Chromium's sandbox off or
  opens a debugging port. Phase B's guard matched only the exact spellings `--no-sandbox` and `--disable-setuid-sandbox`
  and the prefixes `--remote-debugging-port` and `--remote-debugging-pipe`, so `--no-sandbox=1`, `-no-sandbox`,
  `--single-process`, `--in-process-gpu` and `--disable-gpu-sandbox` passed it. The fix round widened it to a short deny
  list read in any of Chromium's switch spellings (`spikes/browser/cef-helper/src/helper.rs`), but it remains a deny
  list: the sandbox state was established by the per-process check, not by this refusal, and the real implementation
  builds Chromium's command line from an allow list and passes nothing else from outside.

Not achieved or not tried:
- CEF's browser process is not sandboxed (by Chromium's design); confining it (P33, an open M0 question of the threat
  model) was not tried.
- Linux: the GPU process ran without a seccomp filter in both confined routes on the hosted runner (no GPU; software
  rendering through Mesa's `llvmpipe`). Chromium's sandbox, which design 5.8 keeps on "on every platform", was therefore
  not shown on in every process on Linux. The GPU process handles what pages send for drawing (WebGL, canvas, video),
  so a flaw reached through it would act with the user's reach rather than inside a sandbox. Not known: whether it is
  sandboxed on a machine with a GPU, and which supported setting confines it where it is not (see the Linux decision).
- The network service runs unsandboxed on Linux and Windows in this CEF build (Chromium starts it with
  `--service-sandbox-type=none`; on Windows it ran unrestricted at the account's integrity), while on macOS it runs in
  the `network` sandbox. It is the process that parses what comes off the network, so on Linux and Windows a flaw in it
  is outside Chromium's sandbox. Whether a Chromium feature or setting turns its sandbox on there was not tried.
- P52, background traffic, with its privacy consequence. On every start on the development Mac, before any navigation,
  Chromium's network service contacted its vendor's servers: the component updater, network time and a search endpoint,
  named by Chromium's own net log (the net logs themselves were not kept), and the saved inspections show the
  network-service process connected to the vendor's addresses while a local page was shown. Each such contact discloses
  the machine's public IP address, the Chromium version and the time of use to the vendor, and the component updater
  downloaded files from those servers into the profile folder. What was downloaded is not known: the net log named the
  hosts, not the files, and the per-run profiles were deleted. What the CI runs contacted is not in their logs.
  `--disable-background-networking`, `--disable-component-update` and `--no-pings` did not stop it. Candidate controls,
  none tried: disabling the responsible features by name (`--disable-features` for network time, the optimization guide
  and the component updater's services), Chromium policies where CEF honours them, a host-resolver rule or proxy that
  lets the browser resolve only the names it is sent to (which is what P52 asks for), a CEF build with the component
  updater compiled out, and an OS-level per-process firewall. This is why the Decision makes a working control a
  condition for shipping.
- `use-mock-keychain`: on macOS it replaces the Keychain-held key that protects the profile's cookies and saved
  passwords with a fixed test key, so that data at rest would be readable by any process running as the same user.
  Harmless for the trial's empty per-run profiles; the real implementation must not carry it forward. It needs instead
  Chromium's Keychain item (the "Safe Storage" key), created and read by a helper signed with a stable identity, so the
  item's access list stays valid across updates; macOS may ask the person to allow access, and that prompt names the
  helper's bundle, not the app, so the app has to say beforehand what it is and why. Not tried. The Linux analogue
  (Chromium falling back to a fixed key when no keyring service is present) was not examined.
- P52, clipboard (an open M0 question of the threat model). CEF has no clipboard hook. Command-C and Command-V typed into
  the pane pass through the helper, where they can be intercepted. CEF's `javascript_access_clipboard` browser setting
  governs script access to the clipboard without a user gesture; it does not stop a page writing to the clipboard on a
  click, and the agent's forwarded click is a user gesture to the page (threat model 4.9, attack 5). So page clipboard
  writes are not contained, and no means to contain them was found or tried.
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
- Start-up: a group of slow starts (539 to 980 ms) is unexplained, and cold start was not measured.
- Confining CEF's browser process (P33) and containing page clipboard writes (P52): open, see above.
- Linux: the GPU process without a seccomp filter on the runner (run 3), and whether it has one on a machine with a GPU;
  see the Linux decision.

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
  messages; frame dimensions checked before pixels are touched. On Windows the trial creates the mapping inheritable and
  passes it through ordinary process spawning, which lets any other child started with inheritance on receive it too; a
  real build must pass it through an explicit handle list (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`).
- `protocol/src/helper.rs` and the frame sink in `app/src/main.rs`: pipes read and written off the UI thread; the newest
  frame waits in a single slot so a slow UI drops frames instead of queueing them; the previous image is dropped from
  GPUI's atlas.
- `cef-helper/src/guard.rs`: the re-launch guard, before anything else runs.
- `cef-helper/src/mac.rs`: the helper owns CEF's `NSApplication`, so GPUI's application object is never touched.
- `cef-helper/src/helper.rs`: messages queued until the browser exists; the view size stored by the reader thread;
  macOS edit shortcuts performed on the focused frame; input-method calls with an explicit invalid range (a null range
  is silently ignored by libcef: the first IME runs failed on exactly this); refusals reported to the app; reload after a
  renderer crash; descriptors 0 and 1 moved off the pipes and the region closed on exec before CEF starts.
- `cef-helper/src/keys.rs`: explicit key codes (the precedent's special-key bug did not occur on macOS); its Linux and
  Windows key tables are unverified.
- `cef-helper/src/main.rs`: on Linux, CEF gets the process's own argument memory, not a copy, so Chromium's process
  titles name each sub-process's role and tools (and the sandbox check) can tell a renderer from a zygote.
- `app/src/main.rs`: the pane's hitbox inserted in prepaint so overlays occlude it; "pointer left" sent when the hitbox
  stops being hovered after a move, not from GPUI's hover state.
- `app/src/mac_ime.rs` and `app/src/script.rs`: testing input methods through GPUI's own text-input client and reading
  results back with DevTools; `protocol/src/tree.rs`: verifying the sandbox per process instead of assuming it.
- `unpack-cef.sh`, `cef-helper/bundle-macos.sh` and the `.deb` step in the workflow: pinned, hash-checked CEF with no
  download by the build script; the macOS helper bundles; the setuid `chrome-sandbox` packaging.

## Revisit when

the Linux GPU process is shown with its seccomp filter on a machine with a GPU, or a supported Chromium setting is found
that confines or removes it where it runs without one (until then the browser does not ship on Linux); a Chromium
setting is found that sandboxes the network service on Linux and Windows; a person runs the browser on real Linux or
Windows hardware; the maintainer's hands-on check on macOS; a control is found that stops or routes Chromium's
background traffic (P52), which the browser needs before it ships; confinement of CEF's browser process is tried (P33);
a way to contain page clipboard writes is found (P52); the maintainer answers how the component's budget relates to
section 9; the slow start-ups are explained; GPUI gains an external-texture element on Linux and Windows, BGRA surfaces
on macOS, or an exposed composition tree on Windows; the targets above are missed at a gate; 120 Hz panes become a
requirement; or CEF changes its off-screen rendering, its Windows sandbox arrangement or its macOS bundle layout.
