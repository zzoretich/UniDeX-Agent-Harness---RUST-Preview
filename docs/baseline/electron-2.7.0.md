# Electron 2.7.0 performance baseline

This file records how fast the Electron predecessor, UniDeX Agent Harness 2.7.0, is on the reference machine, so the
targets in section 9 of the design (`docs/design/specs/2026-11-16-unidex-rust-design.md`) can be checked against it and
the same measurements can be repeated on the Rust app. It covers start-up, idle wake-ups and CPU, idle memory, frame
pacing while scrolling a 5,000-message thread, the delay from a reply arriving to it being on screen, and download size.

Measured on 2026-11-17 against the 2.7.0 JavaScript exactly as released (see "Build" below), with a generated fixture
vault, never a real one. Every timing and memory figure comes from the app's own processes, read through a probe
loaded into its main process, the Playwright Electron connection, or process-level tools (`ps`, `top`, `footprint`).
Nothing used elevated rights, screen capture or Apple Events.

## Results

Times are in milliseconds from the moment the process was spawned unless stated. Memory is in MiB (2^20 bytes; `top`
and `footprint` label it MB); download sizes are in MB of 10^6 bytes. The design's memory target, "150 MB", is in the
same loose unit as the labels; 239 MiB is about 250 MB. "Runs" counts separate measurements; the spread is the full
range.

The start-up figures are for the development Electron binary running the released JavaScript with a mock Keychain
(see "Build"), with the OS file cache warm. They are a lower bound for the shipped application's start-up, not its
figure.

| Metric | Median | Spread | Runs |
|---|---|---|---|
| Warm start (fresh process): main window visible with its first frame (`show`) | 724 ms | 702 to 742 ms | 10 |
| Warm start: first frame rendered, window still hidden (`ready-to-show`) | 665 ms | 647 to 688 ms | 10 |
| Warm start: first paint (paint timing, presentation time) | 702 ms | 680 to 731 ms | 10 |
| Warm start: home screen's navigation rail in the DOM (preload probe) | 513 ms | 472 to 531 ms | 10 |
| Cold start, the only one: first launch of the session, window visible | 2,835 ms | single observation | 1 |
| Idle wake-ups in 60 s, all processes | 625 | 436 to 676 | 5 |
| Idle CPU in 60 s, all processes | 1.2 CPU-s (2.0% of one core) | 1.1 to 1.3 CPU-s | 5 |
| Idle memory, summed physical footprint | 239 MiB | 228 to 263 MiB | 5 |
| Idle memory, summed resident size (overcounts shared pages) | 426 MiB | 393 to 499 MiB | 5 |
| Opening a 5,000-message thread, click to every message drawn | 322 ms | 320 to 381 ms | 5 |
| Scrolling it at 2,000 px/s: median frame interval | 10.0 ms | 10.0 ms | 10 passes |
| Same: 95th / 99th percentile frame interval | 11.6 / 12.0 ms | 11.3-11.9 / 11.8-12.0 ms | 10 passes |
| Same: intervals over 15 ms / over 16.7 ms / over 33 ms | 2 / 2 / 0 of 9,998 | 0-1 / 0-1 / 0 per pass | 10 passes |
| Reply text on screen after the CLI wrote it, new thread | 15 ms | 11 to 21 ms | 13 |
| Same, in the 5,000-message thread | 304 ms | 290 to 313 ms | 5 |
| Same, 5,000-message thread, reply 200 ms after the prompt | 641 ms | 636 to 742 ms | 3 |
| Download: arm64 disk image / zip, as published | 136.3 MB / 133.6 MB | exact | 1 |
| Installed app bundle on disk | 345.6 MiB (362.4 MB) | exact | 1 |

The display refresh rate as the app sees it was 100 Hz (Electron reports 99.98 Hz; animation frames arrive every
10.0 ms).

## Conditions

- Hardware: an Apple silicon desktop with 16 GB of memory, on mains power. The design names "an Apple silicon laptop
  with 16 GB" as the reference machine; this is the same chip family and memory in a desktop, so thermal headroom and
  the display differ from a laptop's.
- Display: the app's window was placed on the primary display, an external one at 100 Hz with a scale factor of 1.
  Window size 1480 by 940 points, the app's own default.
- OS: macOS 26.6.2. The machine had been up for 20 days, so the OS file cache was warm except where noted.
- Load: other desktop applications and several automated coding sessions were running and could not be stopped for
  the measurement. At the end of the runs the 1, 5 and 15 minute load averages were 1.8, 2.4 and 2.6, and
  72% of memory was free.
- Temp folder: the app lists the user's temp folder in the background after creating its window. It held 8,632
  entries during the runs (far fewer than the several hundred thousand found there earlier). The app sweeps its own
  leftovers in the OS temp folder at start-up. A variant with an empty private temp folder is reported under start-up.
- Launches: the measurements took 67 launches of the app in total.
- Date: 2026-11-17.

### Build

- The 2.7.0 release's JavaScript, run unpackaged: `<electron-checkout>/app/out/` from the clean `v2.7.0` checkout. Its
  `out/main/index.js`, `out/preload/index.js`, `out/renderer/hud.js`, `index.html` and `hud.css` are byte-identical
  (SHA-256) to the files inside the released bundle's `app.asar`; the release build left `out/` there.
- The Electron binary is the checkout's development copy (`node_modules/electron`, Electron 44.5.1), the binary the
  project's own end-to-end harness runs. The released bundle carries the same Electron version.
- How it differs from the release: the release reads the app from `app.asar` with asar integrity checks, has cookie
  encryption on (backed by the Keychain), is signed with a Developer ID and notarised, and refuses debugging
  switches. The development binary has none of those fuses, and the probe launch also passes `--use-mock-keychain`,
  which replaces the Keychain with an in-memory store, so the cost of the real Keychain lookup for cookie encryption
  is not in any start-up figure. How much slower the released app starts than the figures here is not measured: asar
  reads and integrity checks, the Keychain lookup, signature checks and the fuses are each unmeasured, and the first
  launch after download adds Gatekeeper's verification. Measuring it would take launching the signed release bundle
  with a probe that does not need the development binary's switches, for example a start-up log written by the app
  itself, which this round did not do. So 724 ms is a lower bound for the shipped application's start-up, not its
  figure.
- The packaged test build in `<electron-checkout>/app/dist-test` is version 2.4.0, so it was not used, and nothing
  was rebuilt.

### Isolation

Every launch used the end-to-end harness's own isolation: the environment the fixture generator writes
(`env.json`: a fake `HOME` and `CFFIXED_USER_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `AOS_CONFIG` and `AOS_VAULT`
inside the fixture, a `PATH` of stubs and system folders only, both host CLIs hidden) plus the harness's additions
(`AOS_APP_VAULT`, `AOS_APP_USER_DATA` inside the fake home, `AOS_APP_WRITE` empty, a loopback start page). The fixture
came from the project's generator, `make-fixture-vault.mjs --out <scratch>/fixture --cache <scratch>/cache`. One
thing the harness does not redirect: macOS keeps the development Electron binary's preferences through its
preferences daemon, which ignores `CFFIXED_USER_HOME`, so that binary's own preferences file in the real home folder
is updated on every launch, as it is on every e2e run. That preferences file was the only write outside the scratch
folder that was found, and no real vault was touched.

## Start-up to first frame (warm start)

**Definition.** From spawning the app's process to the main window becoming visible with its first rendered frame.
The app creates its window hidden and shows it on Electron's `ready-to-show` event, which fires once the renderer has
produced its first frame; the window's `show` event then marks the moment macOS shows it. The headline is `show`.
`ready-to-show` (first frame rendered, still hidden) and the paint timing entry `first-paint` (Chromium's presentation
time of the first frame) are reported beside it. `first-contentful-paint` was identical to `first-paint` in every
run.

**Method.** A launcher spawns the Electron binary directly, with no debugger attached, the harness environment, and a
passive probe loaded into the main process with `-r` (the mechanism Playwright uses for its own loader) and the
`--use-mock-keychain` switch, which the e2e harness also uses and which bypasses the real Keychain. The probe
records wall-clock times of main's milestones and of the first window's events, reads the page's paint entries once
the home screen is up, writes them to a file and quits the app when the launcher sends `SIGUSR2`. All clocks are
`performance.timeOrigin + performance.now()` in each process, so times from the launcher, main and the page share the
system clock. One warm-up launch, then ten measured ones, each a fresh process; the app's data folder (with Chromium's
caches) is kept between launches, as on a user's machine.

```js
// launcher
const t0 = performance.timeOrigin + performance.now();
spawn(ELECTRON, ["-r", "probe.cjs", "--use-mock-keychain", APPDIR], { env: harnessEnv, detached: true });
// probe.cjs, in the main process
app.on("browser-window-created", (_e, w) => {            // the first window is the main window
  w.once("ready-to-show", () => rec.readyToShow = now());
  w.once("show", () => rec.show = now());
});
// once the rail exists: performance.getEntriesByType("paint") in the page, converted with performance.timeOrigin
```

A second probe variant registers a session preload that notes when the home screen's navigation rail
(`.aos-wb-railbtn`) enters the DOM (a `MutationObserver` from document start). It adds 20 to 35 ms to the other
milestones, so its runs give only the rail figure.

**Where the time goes** (medians, ten runs): Node starts in main at 50 ms, the app's main script at 121 ms, Electron
is `ready` at 193 ms, the window exists at 265 ms and its page starts loading at 269 ms; the first frame is rendered at
665 ms and the window is visible at 724 ms. In the preload-probe runs the rail is in the DOM at 513 ms and the first
frame is rendered at 694 ms, so the page spends about 180 ms between building the home screen and producing its first
frame. In every run the rail was in the DOM before `ready-to-show`: the first visible frame already shows the home
screen's navigation.

**Variants.**

- With the Sessions surface on, a host signed in and the 5,000-message thread on disk: `show` median 709 ms (687 to
  762, 5 runs), close to the read-only figure.
- With an empty private temp folder (`TMPDIR` and `MAC_CHROMIUM_TMPDIR`; the app's `app.getPath("temp")` follows only
  the second): `show` median 780 ms (723 to 888, 5 runs) against 745 ms for the same probe variant with the real temp
  folder. At 8,632 entries the temp folder does not slow start-up measurably.
- First launch of the session, before any other run: window visible at 2,835 ms, first frame rendered at 2,769 ms,
  with Node in main only at 1,726 ms. Most of the extra time passed before Node started, which points at the OS
  loading and checking Electron's framework rather than at the app. It cannot be repeated without a reboot or
  `sudo purge`, so it stands as a single observation.
- Launched through Playwright, as the e2e harness does: `first-paint` 557, 557 and 587 ms after the launch call in
  the three scroll runs, and 1,252 and 1,784 ms in the two token runs (different run modes, so the list is not one
  series). The driver-launched start is earlier than the direct launch's 702 ms `first-paint`, and that gap is
  unexplained. Adding the driver's Chromium switches to a direct launch did not reproduce it (four runs, `show` 720 to
  731 ms), so the switches do not explain it. The earlier figure is not evidence of a faster start. None of these
  figures is used.

**What it does not show.** A cold disk: the OS file cache held the binary in every measured run, so those are warm
starts of a fresh process, and the first launch above is the only cold figure (one observation). It does not show a
signed release build (see "Build"). The window's first presentation on the glass is bounded by `show` plus one refresh
interval, not observed directly; `first-paint` falls before `show`, so it does not mark the window being visible.

## Idle wake-ups and idle CPU

**Definition.** With the main window open on its home screen and nothing happening, over 60 seconds: idle wake-ups per
process (the `IDLEW` statistic of `top`, the count of wake-ups from idle the kernel attributes to the process, which
Activity Monitor shows as Idle Wake Ups) and CPU time per process.

**Method.** The same direct launch as for start-up, with the preload probe, which does nothing once the home screen
is up. The launcher then waits 30 seconds, so the runtime refreshes the app starts at launch (a vault scan and its
`git` calls) are finished, and samples:

```sh
top -l 61 -s 1 -n 4000 -stats pid,ppid,command,cpu,idlew,mem,threads   # IDLEW is cumulative: last minus first
ps -o pid=,time=,rss= -p <app pids>                                    # at the start and the end: CPU-time delta
footprint -p <pid> -p <pid> ...                                        # at the end
```

The app's processes are the main process, its descendants, their process groups, and anything whose command line
names the fixture; the list is refreshed every 2 seconds during the window, so a process started while idling would
be caught. Five runs.

**Result** (medians of five runs; the spread is the range):

| Process | Idle wake-ups in 60 s | CPU time in 60 s |
|---|---|---|
| GPU helper | 506 (379 to 554) | 0.3 s |
| Renderer | 93 (41 to 104) | 0.3 s |
| Main | 13 (6 to 25) | 0.6 s (0.6 to 0.7) |
| Network utility | 2 (1 to 5) | 0.0 s |
| All | 625 (436 to 676) | 1.2 s (1.1 to 1.3) |

The "All" row is the median of the per-run totals (625), not the sum of the per-process medians (614).

No process was started during any of the idle windows: the app spawns nothing periodically while idle. Watched from
inside the page through Playwright for 10 seconds (five launches), the home screen ran no CSS or Web animations and
made one batch of DOM changes. About ten wake-ups a second remain, four fifths of them in the GPU helper; their source
was not identified (that needs a system trace, which needs elevated rights).

**What it does not show.** The machine was not quiet (see "Conditions"), and other processes waking the CPU are not
counted here. `IDLEW` counts the process's wake-ups from idle, not every scheduling event. The window was visible but
other windows may have covered parts of it; the app sets `backgroundThrottling: false`, so covering it should not
change its timers.

## Idle memory

**Definition.** About 90 seconds after the home screen is up (the 30 second settle, then the 60 second sampling window
of the idle section), the physical footprint of each of the app's processes (what
`footprint` reports and `top` prints as `MEM`), summed. The summed resident set size is given for comparison only: it
counts the Electron framework's shared pages once per process.

**Method.** Taken at the end of each idle window above, with `footprint -p` for every process and `ps -o rss`.

**Result** (medians of five runs): GPU helper 96 MiB, renderer 72 MiB, main 64 MiB, network utility 6 MiB; summed
footprint 239 MiB (228 to 263). Summed resident size 426 MiB (393 to 499).

Most of the GPU helper's footprint is graphics memory: in one run's `footprint` output about 57 MB is owned graphics
memory and 14 MB is surface memory. That scales with the window size and the display, so it will not carry over to the
Rust app unchanged.

**What it does not show.** Memory after work: the vault is small (the fixture) and no thread or terminal was open.

## Frame pacing while scrolling a 5,000-message thread

**Fixture.** One thread in the format the app stores Sessions threads,
`<vault>/brain/_index/sessions/<workspace>/<thread>.jsonl`: a `meta` line, then for each of 2,500 turns a `prompt`
record (8 to 30 words), the agent's `text` record (1 to 3 paragraphs of 6 to 18 words per sentence; 30% end with a
three-item list, 15% name a file in inline code, 5% end with a fenced code block of 3 to 6 lines), a `usage` and a
`done` record; one `session` record in the first turn. That is 5,000 messages, 10,002 records and 2,140,999 bytes,
generated from a fixed seed from a made-up vocabulary. The generator is committed as
`docs/baseline/fixtures/long-thread.mjs` (with a README) and reproduces the fixture's size and its message text, not the
original's run-time timestamps and session id. Prompts run 8 to 30 words
(median 19); replies run 6 to 146 words (median 53) and 32 to 901 characters (median 323, 95th percentile 628). The
app is launched with the Sessions surface on and a stand-in `claude` CLI recorded as the host's binary, as the
project's Sessions spec does.

**What the app does with it.** It reads the whole file (it keeps only the newest 8 MB of a longer one) and lays out
every message: no virtualisation and no truncation. The open thread is 18,553 elements and 574,784 px tall in a
644 px viewport. From the click on the thread to all 5,000 messages in the DOM with their markdown rendered took a
median 322 ms (320 to 381, five launches).

**Definition.** The interval between successive animation frames while the thread scrolls steadily, read from
`requestAnimationFrame` timestamps inside the window.

**Method.** Through the Playwright Electron connection: open Sessions, open the thread, then for each pass set the
scroll position to a start point, wait one second, measure 60 idle frames for the refresh interval, and scroll at a
constant speed for 10 seconds, setting the position from the frame time:

```js
const y0 = sc.scrollTop; let s;
const step = (t) => { s ??= t; ts.push(t); sc.scrollTop = y0 + speed * (t - s) / 1000;
  if (t - s < ms) requestAnimationFrame(step); else done(); };
requestAnimationFrame(step);                       // intervals = differences of successive ts
```

Five passes per launch from 5%, 25%, 45%, 65% and 85% of the thread, two launches, 2,000 px/s; a long-animation-frame
observer ran during each pass. A stress variant ran five 5-second passes at 10,000 px/s.

**Result.** At 2,000 px/s, over 10 passes and 9,998 frames: median interval 10.0 ms in every pass; 95th percentile
11.6 ms (11.3 to 11.9), 99th percentile 12.0 ms (11.8 to 12.0); the longest interval 20.6 ms. Two intervals were over
16.7 ms, one in each launch at the same place in the thread, and none over 33 ms; no long animation frame (over 50 ms)
was reported. The display runs at 100 Hz, so 1.5 times the refresh interval is 15 ms, and the same two intervals are
the only ones over 15 ms. The longest interval, 20.6 ms, is over twice the refresh interval, so the Electron app
itself fails the rule proposed under "Comparison" (none over twice the refresh interval). At 10,000 px/s: 99th
percentile 12.0 ms or less, longest 12.1 ms, no interval over 15 ms.

**What it does not show.** A scroll set from script each frame is a main-thread scroll; the numbers show that the
page's main thread kept up with the display, not that every tile was rasterised in time (a blank tile during a fast
scroll would not show here). A trackpad scroll goes through the compositor and was not measured.

## First streamed token on screen

**Definition.** The delay from the reply's first text reaching the app to that text being on screen. The Electron app
gets a turn from the host CLI's stream-json output through the runtime's `sessions.js events` filter. It never shows
partial tokens: each text block arrives whole. So "arrival" here is the moment the stand-in CLI writes the line with
the first text block to its stdout, and "on screen" is the start of the second animation frame after the text entered
the DOM (by then the frame containing it has been produced; presentation is at most one refresh later). So the figure
depends on the refresh rate: the second frame is about 20 ms after the change at 100 Hz and up to about 33 ms at
60 Hz.

**Method.** A stand-in `claude` (no model and no network; it answers the catalog's questions like the Sessions spec's
stand-in) prints `init`, waits, prints one assistant text block carrying a marker, recording the time it wrote the
line, then the result. In main, the window's `webContents.send` is wrapped to time the event carrying the marker; in
the page, a `MutationObserver` notes when the marker enters the timeline and the next two animation frames.

```js
// main (through Playwright): time the IPC send that carries the marker
wc.send = (ch, p, ...r) => {
  if (p?.event?.kind === "text" && /T15ANSWER-\d+/.test(p.event.text)) log(now());
  return orig(ch, p, ...r);
};
// page: when the marker appears in the last rows of the timeline, take two animation frames
new MutationObserver(check).observe(document.body, { subtree: true, childList: true, characterData: true });
```

**Result.**

- New thread (13 turns over two launches): 15 ms median (11 to 21). Of that, the CLI's line reaches main's send in
  about 1 ms (up to 4 ms; it passes through main and the filter process), the DOM change follows 2 to 3 ms later (up
  to 8 ms), and the rest is waiting for frames.
- In the 5,000-message thread, with the reply 2 seconds after the prompt so nothing else is pending: 304 ms median
  (290 to 313, five turns). Every event re-renders the whole thread (all 5,000 messages, markdown included), which takes
  about 280 ms before the text is in the DOM. In the 2.7.0 UI source (the Sessions tab, `hud/src/views/SessionsTab.ts`)
  the handler for a thread event (`onEvent`) pushes the event onto the thread's event list and calls `renderTimeline`;
  that method empties the timeline container (`el.empty()`) and hands every row to the thread view again, which
  renders each text body with a markdown render. Nothing is reused between events.
- Same thread, reply 200 ms after the prompt: 641 ms median (636 to 742, three turns), because the prompt's own
  re-render is still running when the reply arrives.

**What it does not show.** Partial-token streaming, which the Electron app does not do. Cross-process times carry about
1 ms of clock uncertainty (a few CLI-to-main deltas came out slightly negative).

## Download size

**Definition.** The size of the released macOS arm64 artifacts as published, and the size on disk of the installed app
bundle.

**Method.** `gh release view v2.7.0 --json assets` on the predecessor's public repository (read only), and
`du -sk` and `du -skA` on the released bundle, which is the bundle inside the published zip (the local artifacts have
the published sizes to the byte).

**Result.** Disk image 136,346,310 bytes (136.3 MB, 130.0 MiB); zip 133,636,547 bytes (133.6 MB, 127.4 MiB). Installed
bundle 353,884 KiB on disk (345.6 MiB; 335.9 MiB apparent size), of which the Electron framework is 285.0 MiB, the
Chromium licence file 19.2 MiB, the runtime the app installs into a vault (Node scripts) 29.8 MiB, and the app's own
code (`app.asar`) 8.0 MiB.

## Comparison with the design's targets

This section is a proposal for the maintainer: each target from design section 9, the Electron figure, and whether
the target should change.

| Target (section 9) | Electron 2.7.0 | Proposal |
|---|---|---|
| Start to first frame under 400 ms | 724 ms visible, 665 ms rendered, as lower bounds | Keep; define the event |
| No periodic wake-ups from our code | 625 wake-ups and 1.2 CPU-s per 60 s | Keep, and add numbers |
| Idle memory under 150 MB | 239 MiB footprint (426 MiB resident) | Keep; name the statistic |
| 5,000-message scroll, no dropped frames | 2 of 9,998 intervals over 16.7 ms (and over 15 ms) | Keep, define it, add two |
| First streamed token within 50 ms | 15 ms new thread, 304 to 641 ms long thread | Keep; require it in long threads |
| Download under 40 MB | 136.3 MB disk image | Keep; name the artifact |

- **Start-up.** 400 ms is a little over half the Electron figure, and that figure is a lower bound for the shipped
  Electron app (development binary, mock Keychain, warm cache), so the real gap is at least this large. Keep the
  target, and define it as this file does: spawn to the main window visible with its first frame, that frame showing
  the home screen's navigation, median of ten warm launches. Also report the first launch after a reboot or install
  at each gate (Electron: 2.8 s, a single observation) without a pass mark until there is a Rust figure, since it
  measures the OS loading the binary more than the app.
- **Idle wake-ups.** "No periodic wake-ups from our code" cannot be checked from outside the process, because the OS
  frameworks wake too. Keep it as a rule checked in code and with an internal counter, and add a measured cap: at most
  60 idle wake-ups in 60 seconds across all the app's processes (about a tenth of Electron's 625), and under 0.1 CPU-s
  in the same minute (Electron: 1.2). Both numbers are placeholders: no floor has been measured on an empty native
  window, so replace them once one is (that takes a minimal native window run through the same idle method). A CI
  runner without a display may not show the same counts as the reference machine, so the CI check should count the
  app's own timer firings rather than `IDLEW`.
- **Idle memory.** 150 MB is about 60% of the Electron footprint (the design's "MB" against 239 MiB, about 250 MB).
  Keep it, and state that it is the summed physical footprint of all the app's processes about 90 seconds after the
  home screen is up (30 seconds to settle, then 60 seconds idle) with the fixture vault; summed resident size double
  counts shared pages and should not be used.
- **Scrolling.** By this measure the Electron app already scrolls the 5,000-message thread at the display's rate, so
  the target as written does not separate the two apps. Keep it and make it testable: over 10 seconds at 2,000 px/s
  and 5 seconds at 10,000 px/s, from five start points, at most one frame interval in 1,000 over 1.5 times the refresh
  interval (15 ms at 100 Hz) and none over twice it (20 ms), measured from presented frames rather than main-thread
  callbacks. Electron's own longest interval was 20.6 ms, so it does not meet the "none over twice" part; the rule is
  a target for the Rust app, not a description of Electron. Add the two places where the Electron app is slow: opening
  a 5,000-message thread (Electron: 322 ms to every message drawn; propose under 100 ms to the first frame showing the
  thread) and a new message appearing in that thread (below).
- **First streamed token.** 50 ms is easy in a new thread (Electron: 15 ms) and badly missed in a long one (304 to
  641 ms), where the cost is re-rendering the whole thread. Keep 50 ms but require it in a 5,000-message thread as well
  as a new one, with the first token 200 ms after the prompt; define arrival as the engine reading the bytes from the
  provider connection (the scripted mock provider of design section 10.1) and on screen as the first presented frame
  that contains the text.
- **Download size.** 40 MB is under a third of the Electron disk image. Keep it, and name the artifact (the compressed
  macOS arm64 disk image, without the browser component). The Electron bundle holds 29.8 MiB of the runtime's Node
  scripts that the app installs into a vault (scripts, not a Node binary). If the Rust app ships the same scripts,
  that alone is about 30 MiB unpacked, so the target would then need revisiting; if it also needs a Node runtime to
  run them, that adds more, which is not measured here.
- **First-frame event.** The Electron headline uses the window's `show` call or event, while the Rust definition asks
  for a presented frame. The two differ by at most about one refresh interval (10 ms at 100 Hz). Use the presented
  frame for the Rust app, since that is what the design defines, and compare it with the Electron `show` figure
  directly.
- **Reference machine.** This baseline was taken on a desktop, not the laptop the design names. Either name this
  machine as the reference in the design or repeat the start-up, scroll and token figures on the laptop before the
  first gate that uses them.

## How to repeat this on the Rust app

The same definitions, without Electron specifics. Use the same machine and conditions, a fixture vault generated by
the Rust project's own tooling with the same content (the long thread comes from the generator in
`docs/baseline/fixtures/`), and record the display's refresh rate, the temp folder's entry
count and what else is running.

- **Instrumentation.** Have the app write wall-clock timestamps (system clock, sub-millisecond) of its start-up
  milestones, its frames and its render of new messages to a file named by an environment variable, so no debugger or
  injected code is needed. Launch with the same isolation: a fake home, config folders and vault inside a scratch
  folder.
- **Start-up.** t0 is the launcher's clock just before it spawns the binary. Report the first frame presented in the
  main window, and the moment the window becomes visible if it is shown before its first frame. The frame must show the
  home screen's navigation. One warm-up launch, then ten; median and range. Report a first launch separately.
- **Idle.** Wait 30 seconds after the home screen is up, then sample 60 seconds with
  `top -l 61 -s 1 -stats pid,ppid,command,cpu,idlew,mem,threads`; per process, `IDLEW` at the end minus at the start,
  and the `ps -o time` delta for CPU; list any process started in the window. Five runs.
- **Memory.** At the end of each idle window, `footprint -p` for every process of the app; report the sum.
- **Scrolling.** Generate the 5,000-message thread with `node docs/baseline/fixtures/long-thread.mjs <output-file>`
  (see `docs/baseline/fixtures/README.md`: 2,500 prompts and 2,500 replies, seed 15015, 2,140,999 bytes), converting
  it to the Rust app's own thread storage if the formats differ. Time opening it from the click to the first frame
  showing it and to every message being available. Scroll at a constant speed set from the frame time: 10 seconds at
  2,000 px/s and 5 seconds at 10,000 px/s, from 5%, 25%, 45%, 65% and 85% of the thread, two launches. Take intervals
  between presented frames; report the median, 95th and 99th percentiles, the longest, and the counts over 16.7 ms,
  over 33.3 ms and over 1.5 times the refresh interval.
- **First streamed token.** Use the scripted mock provider: it sends the first text delta at a known time and notes
  when it wrote it; the engine notes when it read it; the UI notes the first presented frame that contains it. Run at
  least five turns in a new thread and five in the 5,000-message thread, with the first delta 200 ms and 2 seconds after
  the prompt.
- **Download size.** The byte size of the published macOS arm64 disk image from the release's metadata, and `du -sk`
  of the installed bundle.
