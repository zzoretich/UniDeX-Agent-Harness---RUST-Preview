# 0008: Streaming chat and terminal trial

Status: accepted
Date: 2026-11-17

## Context

Task 10 of M0 asks whether the pinned GPUI snapshot ([0007](0007-gpui-source-and-pins.md), `gpui-pre =0.3.12`) can
deliver the two most demanding everyday surfaces of the application, a long streaming chat thread and a terminal,
smoothly on macOS, Linux and Windows. The design sets the architecture and the bar: screen state free of GPUI types,
a Tokio core whose event streams the app forwards into GPUI, a UI thread that never waits on I/O, virtualised long
lists and incrementally parsed streamed markdown (section 5.10); a PTY layer, `alacritty_terminal` and our own grid
element for terminals (section 5.7); and the performance targets of section 9: cold start to first frame under 400 ms,
no periodic wake-ups when idle, a 5,000-message thread scrolled with no dropped frames at the display's refresh rate,
and the first streamed token on screen within 50 ms of arrival. The Electron predecessor was measured on the same
machine for comparison (the baseline record of Task 15, `docs/baseline/electron-2.7.0.md`, not yet merged when this
was written).

The trial's code is throwaway and lives in `spikes/chat-term/`, a standalone Cargo workspace outside the repository's
workspace. The PTY crate it chose is recorded separately in [0009](0009-pty-crate.md).

### What was built

- `core/` (no GPUI): a deterministic generator of a 5,000-message thread (`content.rs`; 3.5 MB of markdown, 18,070
  blocks: paragraphs with inline code, emphasis and links, lists, 1,414 fenced code blocks of 5 to 80 lines, 177
  tables, headings, emoji, CJK, Arabic and Hebrew, and a few long unbroken lines); a flat markdown model over
  `pulldown-cmark` with incremental parsing of a growing message (`md.rs`, `StreamDoc`: blocks before the last blank
  line outside a code fence are parsed once, only the open tail is parsed again); statistics; key and paste encoding
  (`keys.rs`); the two PTY backends behind one session type (`pty.rs`); and `headless-check`, which runs all of it
  without a display.
- `app/src/bin/chat.rs`: a window with GPUI's `list` element over 5,000 messages plus a reply that streams in. Tokens
  come from a task on a Tokio runtime at 200 a second (tokens of 2 to 6 characters) over a `tokio::sync::mpsc`
  channel; a GPUI foreground task awaits the channel, drains whatever has arrived and applies the batch. Each message's
  markdown is parsed when it is first drawn, turned into shared strings and text runs once, and kept. Two renderers:
  `plain` (our own blocks drawn with GPUI's `StyledText`) and `kit` (the widget kit's markdown `TextView` from
  `gpui-base`, cargo feature `kit`). Scripted scenarios write JSON.
- `app/src/bin/term.rs`: a terminal pane: a PTY running a shell, `alacritty_terminal` for emulation, and our own GPUI
  element that draws the grid (backgrounds merged per run, text shaped per run of same-styled cells with GPUI's
  `force_width` so glyphs land on cells, wide glyphs shaped alone at their column, the cursor, the selection), with
  keyboard input, paste, mouse selection, copy and scrollback. Scripted scenarios write JSON.
- Instrumentation, inside the app: a zero-size element painted last in each frame marks the end of the paint phase;
  the root view's `render` marks the start of the draw. "Paint interval" is the time between successive paint ends of
  drawn frames, "draw work" is render start to paint end (layout, prepaint and paint; not the hand-over to the GPU).
  Frame signals (`on_next_frame`) are recorded while a scenario drives the window. A token's delay is from the moment
  the Tokio task sent it to the paint end of the first frame that includes it. CPU time, context switches and peak
  resident memory come from `getrusage`, the physical footprint from `proc_pid_rusage`, and the process start from the
  kernel's record of it (`proc_pidinfo`). Idle wake-ups are also sampled from outside with `top` (IDLEW). A build with
  GPUI's own frame profiler (cargo feature `profiler`) splits the draw into its phases and times the present call.

### Conditions

Measured on 2026-11-17 on an Apple silicon desktop with 16 GB of memory (the machine of the Electron baseline), macOS
26.6, an external display at 100 Hz (GPUI reports a 10.0 ms refresh interval) with a scale factor of 1. Release build
(Cargo's release profile with line-table debug information), Rust 1.100.0. The chat window was 1,100 by 900 points,
the terminal 900 by 600. Other applications and several automated coding sessions were running and could not be
stopped; the load average was about 4. Each scenario ran in a fresh process; three runs of each chat scenario per
renderer, ten start-ups after one warm-up launch, three runs of each terminal timing scenario per PTY backend (idle:
three runs on `alacritty_terminal`, one on `portable-pty`). Windows were activated and GPUI's default 30 fps limit
for inactive windows was turned off (`inactive_frame_interval: None`) so a lost focus could not throttle a run. Commands: `sh spikes/chat-term/run-macos.sh
<folder> 3` (plain), `TARGET_DIR=target/kit CHAT_ARGS="--renderer kit" sh run-macos.sh <folder> 3 --features kit`,
`TARGET_DIR=target/profiler sh run-macos.sh <folder> 1 --features profiler`, `sh run-term-macos.sh <folder> 3`, and
`headless-check` three times; `python3 summarize.py <folder>` turns the JSON into the tables below. The raw result
files were not committed: the tables in this record are the kept evidence, and the trial code is deleted at the M0
gate.

### Sources and provenance

Read as reference, all permissively licensed: the pinned `gpui-pre` 0.3.12 crates (the `list` element, the window draw
loop and frame pacing, the profiler, the text system, keystroke parsing, the elements, and the `input.rs` and
`list_example.rs` examples), `gpui-pre-macos` (key names), `gpui-component` 0.10.0 and `gpui-base` 0.10.0 (the markdown
`TextView`, `TextViewState`, `MessageScroller` and the selection modules), `alacritty_terminal` 0.26.0, `vte` 0.15.0,
`portable-pty` 0.9.0 and `libc` 0.2.191, all from the cargo registry; crates.io and GitHub API pages for versions,
dates and licences (details in [0009](0009-pty-crate.md)); the design spec (sections 5.7, 5.10, 8, 9 and 11.4),
decision 0007, the cookbook and the Electron baseline record. The trial's code was written for the trial against those
crates' public APIs; the spike source carries no credit for copied code and the trial's report records none. Nothing was
read or copied from the editor project's crates that are not permissively licensed (its terminal view, markdown and
editor crates in particular).

## Results

### Chat, plain renderer (GPUI `list` and our own text runs)

Values from separate runs are separated by semicolons (run 1; run 2; run 3).

| Measure | Result |
|---|---|
| Start-up from a fresh process (OS file cache warm; the baseline calls this warm start), process start to first frame painted, 10 launches | median 152 ms, range 142 to 172 ms |
| Same, to the next frame signal (the first frame is presented by then) | median 164 ms, range 151 to 184 ms |
| First launch after a fresh build (one observation) | 627 ms painted; 318 ms of it before `main` (the OS checking a new binary) |
| Idle 30 s with the thread open: frames drawn, CPU time | 0; 0; 0 frames. 0.00; 0.00; 0.00 CPU-s |
| Idle: wake-ups from idle counted by `top` over 25 s | 2; 2; 0 |
| Idle physical footprint (thread open, never scrolled) | 39.9; 39.6; 39.6 MiB |

| Scenario (100 Hz; intervals in ms) | Frames | Paint interval median / p95 / p99 / max | Over 1.5x refresh / over 2x | Over 16.7 ms | Draw work median / p99 | CPU-s |
|---|---|---|---|---|---|---|
| Scroll at 2,000 px/s for 10 s from the top, first visit | 1,000 each | 10.0/11.9/15.9/21.0; 10.0/12.8/16.4/20.3; 10.0/10.7/11.8/14.4 | 20/1; 20/1; 0/0 | 6; 6; 0 | 1.34/7.30; 1.46/8.03; 0.43/2.15 | 2.6; 2.9; 1.0 |
| Scroll top to bottom at 30,000 px/s (46 s, the whole thread) | 4,616 each | 10.1/15.5/17.4/20.5; 10.1/15.1/17.2/20.2; 10.1/14.5/16.2/28.3 | 290/2; 252/1; 160/1 | 80; 76; 27 | 2.83/9.10; 2.53/8.58; 2.42/7.39 | 16.6; 15.6; 14.5 |
| Scroll at 2,000 px/s for 10 s from 45 %, second visit | 1,000 each | 10.0/11.2/14.6/18.4; 10.0/10.4/11.6/12.9; 10.0/10.9/14.0/18.2 | 8/0; 0/0; 4/0 | 1; 0; 3 | 1.41/5.83; 0.42/1.86; 1.12/5.21 | 2.7; 1.0; 2.3 |
| 60 jumps to random messages, every tenth frame | 660 each | 10.0/17.7/21.0/23.5; 10.0/12.4/13.5/14.3; 10.0/16.8/20.2/24.1 | 56/10; 0/0; 51/7 | 40; 0; 33 | jump frames only: 9.4/14.8; 2.9/4.6; 8.1/15.1 | 2.0; 0.8; 1.8 |
| Stream 30 s, following the end | 2,998 each | 10.0/10.9/11.8/16.9; 10.0/10.4/11.2/13.7; 10.0/11.3/12.0/15.0 | 1/0; 0/0; 0/0 | 1; 0; 0 | 2.69/4.41; 0.78/3.95; 1.85/3.50 | 11.7; 5.8; 8.6 |
| Stream 30 s, scrolled 1,000 messages up | 2,996 to 2,998 | 10.0/10.5/11.2/20.1; 10.0/10.2/10.6/20.1; 10.0/10.3/10.5/12.0 | 2/1; 2/1; 0/0 | 2; 2; 0 | 1.31/2.02; 0.37/1.57; 0.98/1.48 | 7.9; 3.8; 6.3 |
| Stream 30 s while the window is resized every frame (width 800 to 1,400) | 3,015 to 3,024 | 10.0/11.5/13.7/19.4; 10.0/10.8/13.8/19.6; 10.0/10.8/14.3/86.4 | 12/0; 23/0; 19/2 | 5; 9; 11 | 1.50/8.82; 1.46/9.28; 1.45/9.04 | 12.8; 13.2; 13.1 |

| Streaming (6,001 tokens per run, 24 KB of markdown) | Following the end | 1,000 messages up | While resizing |
|---|---|---|---|
| Token sent to frame painted, p50 / p95 / p99 (ms) | 8.0/12.5/13.5; 6.3/11.0/12.5; 7.0/11.6/12.5 | 6.6/11.0/11.5; 5.6/10.2/10.8; 6.1/10.7/11.2 | 7.3/11.9/15.7; 7.2/11.7/16.3; 7.2/11.9/18.0 |
| Worst token (ms) | 17.3; 14.4; 15.5 | 16.6; 11.7; 12.4 | 27.7; 24.7; 88.3 |
| Channel, Tokio task to GPUI task, p50 / p99 (ms) | 0.03/3.9; 0.02/3.0; 0.02/2.8 | 0.03/1.9; 0.01/1.2; 0.03/1.5 | 0.02/9.3; 0.02/9.5; 0.02/9.8 |
| Incremental parse per token, p50 / p99 (ms) | 0.014/0.041; 0.008/0.033; 0.014/0.042 | 0.015/0.044; 0.009/0.041; 0.016/0.044 | 0.006/0.026; 0.008/0.030; 0.008/0.029 |
| Peak resident / footprint at the end (MiB) | 96/111; 85/111; 97/112 | 92/104 each | 98/113 each |

Peak memory after scrolling the whole thread: 129 MiB resident, 145 to 149 MiB footprint. Every message the user has
seen keeps its prepared blocks; nothing is evicted in the trial.

Without a display (`headless-check`, release, 3 runs): generating the thread takes 24 to 41 ms and parsing all 5,000
messages from scratch 17.6 to 20.1 ms. Streaming a 40 KB reply token by token (10,093 tokens), the incremental parse
costs 0.001 ms per token at the median and 0.003 ms at p99, and a full parse of the whole message would cost 0.04 ms
at the median and 0.08 ms at p99 (0.08 ms near the end); the incremental result equals a full parse after every one of
the 10,093 tokens in all three runs.

### Chat, kit renderer (`gpui-base` 0.10.0 `TextView` inside the same GPUI `list`)

Same scenarios, same machine, three runs each; built with `--features kit` and run with `--renderer kit`. The kit's
`TextView::markdown(id, text)` parses on first use (synchronously up to 4 KB, on a background thread above that), keeps
its parsed state only while the row is drawn in consecutive frames (GPUI's keyed element state, read in its source),
and appends streamed text with `TextViewState::push_str`, parsing the last block again with the new text on a
background thread. A streamed token counts as painted only once the parse that covers it has landed.

| Measure | Kit renderer | Plain renderer (above) |
|---|---|---|
| Start-up to first frame painted / presented (medians of 10) | 169 / 182 ms (157 to 181 painted) | 152 / 164 ms |
| Idle 30 s: frames, CPU, footprint | 0 frames, 0.00 CPU-s, 46 MiB | 0 frames, 0.00 CPU-s, 40 MiB |
| Scroll 2,000 px/s, first visit: over 1.5x / 2x refresh; over 16.7 ms | 20/7; 20/1; 23/3. 13; 11; 9 | 20/1; 20/1; 0/0. 6; 6; 0 |
| Scroll top to bottom at 30,000 px/s: over 1.5x / 2x | 248/4; 267/5; 230/8 of about 4,945 | 290/2; 252/1; 160/1 of 4,616 |
| Scroll 2,000 px/s, second visit: over 1.5x / 2x | 9/2; 9/1; 7/2 | 8/0; 0/0; 4/0 |
| Jumps: late jump frames of 60; jump-frame draw work median / p99 | 46; 50; 47. 8.6/15.8; 9.0/15.9; 8.8/17.1 ms | 56; 0; 51. 9.4/14.8; 2.9/4.6; 8.1/15.1 ms |
| Stream, following: paint p99; over 1.5x | 12.6; 12.6; 12.6 ms. 1; 1; 1 | 11.8; 11.2; 12.0 ms. 1; 0; 0 |
| Stream, following: token sent to painted p50 / p99 | 7.6/13.1; 7.7/13.2; 7.6/13.1 ms | 8.0/13.5; 6.3/12.5; 7.0/12.5 ms |
| Stream, following: draw work median / p99; CPU-s | 2.3/4.3 ms each run; 10.4 to 10.5 | 0.8 to 2.7 / 3.5 to 4.4 ms; 5.8 to 11.7 |
| Stream while resizing: over 1.5x; token p99 | 31; 27; 40. 21.6; 20.8; 22.2 ms | 12; 23; 19. 15.7; 16.3; 18.0 ms |
| Footprint after scrolling the whole thread | 122 to 128 MiB (state dropped as rows leave) | 145 to 149 MiB (prepared blocks kept) |

The kit's runs agree with each other closely; the plain renderer's vary between runs (see above), so the two compare
best on their worst runs. On frame pacing the kit is behind the plain renderer in places, as the table shows: while
resizing, 27 to 40 intervals over 1.5 times the refresh interval against 12 to 23, and a token p99 of 20.8 to 22.2 ms
against 15.7 to 18.0 ms; on the first-visit scroll, 1 to 7 intervals over twice the refresh interval against 0 or 1; on
the fast pass, 4, 5 and 8 intervals over twice the refresh interval against 2, 1 and 1; and start-up 17 ms later at the
median. It is ahead on memory because it forgets rows it no longer draws (and parses them again when they return). The
kit adds 33 crates to the resolved graph, counted from `cargo metadata` over all targets and all dependency kinds; a
later recount over normal dependency edges only gave 31, and the difference was not traced. All are permissively
licensed; one, `instant`, carries an unmaintained notice.

### Where a frame's time goes (GPUI's profiler, plain renderer, one run)

One run of each scenario with `--features profiler` (GPUI records each draw's phases and each present). Medians and
p99 in ms. GPUI renders and lays out the list's rows, and shapes their text, inside its prepaint phase.

| Phase | request layout | prepaint | paint | whole draw | present call | Present-to-present intervals over 1.5x refresh (our paint intervals over 1.5x) |
|---|---|---|---|---|---|---|
| Scroll 2,000 px/s, first visit | 0.02 / 0.05 | 0.49 / 5.74 | 0.61 / 1.31 | 1.28 / 6.65 | 0.19 / 0.28 | 14 of 999 (13) |
| Scroll top to bottom, 30,000 px/s | 0.01 / 0.03 | 2.03 / 7.07 | 0.40 / 0.66 | 2.48 / 7.62 | 0.05 / 0.20 | 157 of 4,614 (157) |
| Scroll 2,000 px/s, second visit | 0.02 / 0.05 | 0.44 / 3.72 | 0.55 / 0.96 | 1.17 / 4.43 | 0.19 / 0.28 | 4 of 999 (4) |
| Random jumps | 0.02 / 0.05 | 0.40 / 10.22 | 0.54 / 1.05 | 1.11 / 11.14 | 0.17 / 0.27 | 44 of 659 (43) |
| Stream, following | 0.02 / 0.04 | 1.05 / 2.53 | 0.60 / 0.99 | 1.81 / 3.57 | 0.13 / 0.24 | 0 of 2,997 (0) |
| Stream, 1,000 messages up | 0.02 / 0.05 | 0.43 / 0.66 | 0.55 / 0.91 | 1.13 / 1.67 | 0.19 / 0.28 | 1 of 2,989 (1) |
| Stream while resizing | 0.01 / 0.02 | 1.03 / 8.90 | 0.35 / 0.55 | 1.49 / 9.36 | 0.07 / 7.30 | 15 of 3,023 (15) |

The profiler's present-to-present intervals and the probe's paint-to-paint intervals count the same slow frames, so the
probe (which runs in every build) tracks presented frames. The time that makes frames late is the prepaint of rows
that are new to the view, which includes shaping their text; the share that is text shaping is likely but not isolated,
because a row's first draw here also pays the trial's own markdown parse and run preparation. Pre-preparing every row
before the scroll and re-running the first-visit pass would isolate it; that was not done. While the window is being
resized, the present call itself can also take several ms (the platform resizing its drawable). Start-up in the
profiler build: median 166 ms to the first frame painted (141 to 176, ten launches).

### Terminal (our grid element over `alacritty_terminal`, both PTY backends)

| Measure (release, 3 runs per backend unless stated) | `alacritty_terminal::tty` | `portable-pty` |
|---|---|---|
| Correctness checks through the real element (one run each, below) | 13 of 13 pass | 13 of 13 pass |
| `seq 1 200000`: write of the command to the frame that shows the final line and the prompt | 151.1; 151.0; 171.2 ms | 200.9; 152.1; 151.0 ms |
| Same burst: frames drawn, paint interval median / max, over 1.5x refresh | 16 to 18 frames; 10.0 / 10.8 to 11.3 ms; 0 | 16 to 21 frames; 10.0 / 10.6 to 11.4 ms; 0 |
| Same burst: draw work median / p99 | 0.89 to 0.95 / 1.25 to 1.35 ms | 0.91 to 0.94 / 1.22 to 1.35 ms |
| Keystroke written to the frame showing its echo, 100 keystrokes at random gaps, p50 / p95 / p99 / max | 7.7/11.5/12.0/12.2; 6.9/11.1/11.5/12.0; 7.0/11.8/12.3/12.4 ms | 8.2/11.6/12.1/12.2; 7.1/11.4/11.7/12.4; 6.6/11.4/11.7/12.1 ms |
| Idle 30 s, no cursor blink: frames drawn, CPU | 1; 1; 1 frames. 0.01; 0.01; 0.03 CPU-s | 1 frame, 0.02 CPU-s (1 run) |
| Idle 30 s, cursor blinking every 500 ms: frames drawn, CPU | 58; 58; 58 frames. 0.09; 0.12; 0.17 CPU-s | 58 frames, 0.17 CPU-s (1 run) |

Wake-ups from idle counted by `top` over about 25 s: without blink 2, 0 and 1 (alacritty) and 1 (portable-pty);
with the cursor blinking 0, 3 and 87 (alacritty) and 51 (portable-pty). The blink wakes the app twice a second, which
should show as about 50; that two runs show almost none means IDLEW, which counts wake-ups of an idle processor, says
little on a machine this busy. The in-app counts (frames drawn, CPU time) are the evidence for "no periodic work".

The correctness checks, each observed through the emulator's state and the element's own records, not the screen:
a 24-bit foreground and background colour on the right cells; a 256-colour table; bold, italic and underline flags; two
CJK characters and an emoji taking two cells each and drawn as three separately placed wide glyphs, and a combining
acute accent kept on its base cell; `seq 1 20000` leaving exactly 10,000 lines of scrollback; `vi` switching to the
alternate screen, showing typed text, and after a window resize reporting through `:set columns? lines?` exactly the
grid the element computed (114 by 33 before, 89 by 23 after), then leaving the alternate screen on quit; bracketed
paste mode turned on by the program and pastes wrapped in `ESC[200~ ... ESC[201~`; ten GPUI keystrokes (parsed from
GPUI's own notation, for example `ctrl-c`, `alt-b`, `ctrl-right`, `shift-tab`) encoded to the expected bytes; and a
selection made with `alacritty_terminal`'s model copying exactly the selected text. Without a display,
`headless-check` adds: 33 encoded keystrokes (control and alt combinations, arrows in normal and application cursor
mode, function keys, a non-ASCII character) arriving byte for byte at a program reading in raw mode through each PTY;
resize seen by the child; exit codes; and `vi` on the alternate screen through each PTY. All of it passes on macOS in
all three runs; the PTY figures are in 0009.

### Against the design's targets and the Electron baseline

Read every verdict below as "in the trial, on this machine", with these caveats: the machine is an Apple silicon
desktop, not the design's reference machine (a laptop), and a 120 Hz display leaves 8.3 ms per frame instead of
10 ms; the trial's start-up is an unsigned release build with no home screen and no real work at start (the thread is
generated in memory); and the Electron start-up figure is a lower bound for the shipped app, not its figure (the
baseline ran the released JavaScript on the development Electron binary with a mock Keychain, so asar integrity checks,
the Keychain lookup, signature checks and Gatekeeper are not in it).

| Target (section 9) | This trial (macOS, release) | Electron 2.7.0, same machine | Verdict |
|---|---|---|---|
| Cold start to first frame under 400 ms | Fresh process, file cache warm: 152 ms painted, 164 ms presented (medians of 10); first launch after a build: 627 ms (one observation) | Fresh process, file cache warm: 724 ms visible, 665 ms rendered; first launch of a session: 2,835 ms (one observation) | Met in the trial, on this machine, for a fresh process with the thread generated in memory; the 627 ms first launch after a build is over the target; a first launch after a reboot was not measured |
| No periodic wake-ups when idle | Chat: 0 frames and 0.00 CPU-s in 30 s, three runs. Terminal: 1 frame without blink; 58 frames in 30 s with the cursor blinking | 625 wake-ups and 1.2 CPU-s per 60 s | Met in the trial for the chat only (no frames, no CPU). The terminal with its blink on draws about two frames a second. The verdict rests on frame counts and CPU time, not on the `top` counter |
| Idle memory under 150 MB | 40 MiB footprint idle; 145 to 149 MiB after scrolling all 5,000 messages | 239 MiB footprint idle | Met at start in the trial; at the limit after a full scroll, because nothing is evicted |
| 5,000-message scroll with no dropped frames | At 2,000 px/s, paint intervals: 0 to 20 of 1,000 over 1.5 times the refresh interval, 0 or 1 over twice it; 0 to 6 over 16.7 ms. At 30,000 px/s: 3.5 to 6.3 % over 1.5 times. Frame-signal intervals: see below | At 2,000 px/s: 2 of 9,998 over 1.5 times (15 ms), the longest 20.6 ms (over twice); at 10,000 px/s none over 15 ms | Not met as written, and on this measure worse than Electron: up to 20 of 1,000 first-visit intervals (2 %) against 2 of 9,998 (0.02 %), about a hundred times the rate; up to 6.3 % on the fast pass against none for Electron at 10,000 px/s (a slower pass, so that pair is not like for like) |
| First streamed token on screen within 50 ms | Rendering path only: p99 11 to 14 ms from an in-process channel send to the end of paint, for steady tokens into a row that already exists in the 5,000-message thread (add at most one refresh, 10 ms, for presentation); 16 to 18 ms while resizing | 304 to 641 ms from the CLI writing the reply to it on screen, in the 5,000-message thread (includes file watching and rendering) | Not comparable with the Electron figure and not the target's measure. The rendering path costs 11 to 14 ms per token at p99, which leaves room inside the 50 ms target; the first-token path through a real provider (process, network, creating the row) was not measured |

How the scroll was measured, and which measure to trust. Scrolling was driven programmatically (`list.scroll_by` from
a next-frame callback), which misses real trackpad input; the Electron baseline also set its scroll from script, and
records that a trackpad scroll was not measured. Two measures were recorded for each scroll run. The paint intervals
(the table above) are the time between the ends of successive paints. The frame-signal intervals are the time between
successive next-frame callbacks, the closest in kind to the baseline's `requestAnimationFrame` timestamps. On the
frame-signal measure the first-visit scroll at 2,000 px/s had 0, 0 and 0 of 999 intervals over 1.5 times the refresh
interval (longest 10.4 to 14.7 ms), the second visit 0, 0 and 0, the fast pass 0, 0 and 1 of about 4,615 (the one
also over twice, 20.0 ms), and the random jumps 2, 0 and 3 of 659. The two measures disagree, and the trial does not
explain why. The profiler build's intervals between successive present calls matched
the paint intervals (14 of 999 over 1.5 times at 2,000 px/s on a first visit; 157 of 4,614 on the fast pass), so the
paint-interval figures are the ones the verdict rests on, and the frame-signal figures show only the cadence of the
callbacks.

The Electron app lays out all 5,000 messages when the thread opens (322 ms, a 574,784 px tall page), so scrolling only
moves pixels the compositor already has; the GPUI list lays out only what is visible and shapes text as rows come into
view. That is why its start-up is far better and its scrolling worse on a first visit; the token latencies cannot be
compared (see the table). The slow frames concentrate where many new rows appear at once: the first pass at 2,000 px/s, the fast pass,
and jumps (the frame after a jump does 3 to 15 ms of draw work).

The runs disagree with each other more than the frames within a run do: in the second run of every chat scenario the
draw work was a third of the other runs' (0.4 against 1.3 ms median in the slow scroll). The likeliest cause is which
cores the main thread ran on and at what clock while other sessions loaded the machine; it was not confirmed, because
that needs a system trace, which needs elevated rights.

## Options considered

| Option | Trade-off |
|---|---|
| A. GPUI's `list` with our own markdown model and text runs (`plain`, measured) | Smallest dependency set and full control over parsing, caching and streaming; but selection, copy, link clicks and code highlighting are ours to build, and GPUI keeps shaped text only from one frame to the next. |
| B. GPUI's `list` with the widget kit's `TextView` per message (`kit`, measured) | Selection, copy, links, code-block hooks and streaming parse on a background thread come with it; 33 more crates, a parse that lands a frame later during a stream, and a renderer we do not control. |
| C. GPUI's `uniform_list` | Needs one fixed row height; chat rows vary from one line to hundreds. Rejected without building it. |
| D. `ListState::measure_all()` (measure every row up front, for an exact scrollbar) | Would lay out all 5,000 messages before the first frame, the cost the Electron app pays; not built. The trial draws no scrollbar. |
| E. The kit's `MessageScroller` | A thin wrapper over the same GPUI `list` with tail following (read in its source), adding a jump button and fade; nothing to measure beyond option B. |
| Terminal: our own GPUI element over `alacritty_terminal` (built) | The design's choice; the kit has no terminal or cell-grid component. |

## Decision

GPUI is adequate for both surfaces on macOS. Build the chat thread on GPUI's own `list` element, fed from the Tokio
core through a channel into a GPUI task, with each message's parsed markdown kept per message and a streaming message
parsed incrementally; build the terminal as our own element over `alacritty_terminal`, with the PTY from 0009.

**Widget kit:** do not adopt it for the thread list (GPUI's `list` is enough, and the kit's `MessageScroller` is a thin
wrapper over it) or for the terminal (it has nothing for a cell grid). For message bodies the recommendation is
provisional: adopt the kit's unstyled layer, `gpui-base`, and its markdown `TextView` (it already passes the licence
policy) for the selection, copy, link handling, code-block highlighting hooks and background, incremental streaming
parse that GPUI does not have and the plain renderer would have to build, if the checks below hold. Its measured cost is
worse than our own renderer's in places: while resizing, 27 to 40 intervals over 1.5 times the refresh interval against
12 to 23, and a token p99 of about 21 ms against 16 to 18 ms; on a first-visit scroll, 1 to 7 intervals over twice the
refresh interval against 0 or 1; on the fast pass, 4, 5 and 8 against 2, 1 and 1; and start-up 17 ms later (169 against
152 ms). It is smaller in memory after a full scroll. Two limits on the evidence: the kit was measured only in its
element-keyed form, which drops its parse whenever a row leaves the view, while the form recommended here (one
`TextViewState` per message in an app-level cache) was not measured; and selection and links, the reasons for adopting
it, were not exercised. Before the recommendation is final, measure: the cached-state form in the same scenarios
(scroll, jumps, stream, resize); selection and copy within a message and across messages in the virtualised list; link
activation; and that a long message whose parse lands a frame later does not shift the rows around it. None of that was
measured here. The styled layer (`gpui-component`) is not needed for these two surfaces.

## Who decided

A design call from the M0 trial (Task 10), within decision 0002 and design sections 5.7 and 5.10.

## Consequences

- The chat needs, as its own work: a cache of prepared messages with eviction (memory grows with every message seen
  otherwise), a persistent cache of shaped text or a smarter first-visit path if the scroll target is to be met
  strictly (GPUI keeps line layouts only from the previous frame, so a row that leaves the view is shaped again when
  it returns; that shaping is the likely, not isolated, cause of the slow first-visit frames), and a scrollbar that copes with unmeasured rows (estimated heights, or GPUI's uniform height hint).
- Streaming is probably not where the risk is: the rendering path for a token (Tokio task, channel, GPUI task,
  incremental parse, one row re-measured, frame) is about 7 ms at the median and 13 ms at p99 to a painted frame, for
  steady tokens into an existing row (the first token of a reply, and a real provider, were not measured), and the parse is a few
  microseconds per token. Even a full re-parse of a 40 KB message per token would fit (0.08 ms); incremental parsing
  matters more for very long messages and for keeping prepared blocks stable.
- Text selection across messages, link activation, syntax highlighting, IME input in the terminal, right-to-left
  layout and screen reader roles are not proven by this trial (see below); Task 11 covers accessibility.
- The terminal's drawing cost is small (about 1 ms of draw work per frame at 114 by 33 cells) and a 200,000-line burst
  is drawn in 16 to 21 frames with no slow frame, because PTY wake-ups are coalesced into one notification per frame
  (an atomic flag in the GPUI terminal).
- A 120 Hz display leaves 8.3 ms per frame. The p99 draw work during the fast scroll (7.4 to 9.1 ms) would miss more
  frames there; the reference machine of section 9 is a laptop whose display may run at 120 Hz.

### Patterns worth copying into the cookbook

- Tokio to GPUI: `produce` and the `cx.spawn` loop that drains a `tokio::sync::mpsc` receiver in batches and applies
  them in one `update`, in `spikes/chat-term/app/src/bin/chat.rs`.
- A long chat list: `ListState::new(.., ListAlignment::Top, overdraw)`, `FollowMode::Tail`, and
  `remeasure_items(last..last + 1)` for the streaming row, in `chat.rs` (`ChatView::on_tokens` and `render`).
- Prepared message bodies: markdown parsed once into shared strings and `TextRun`s and kept per message
  (`prepare`, `ThreadMsg::prepared` in `chat.rs`); incremental parsing of a growing message in
  `spikes/chat-term/core/src/md.rs` (`StreamDoc`).
- Frame instrumentation without the profiler: the `Probe` element and `Recorder` in
  `spikes/chat-term/app/src/common.rs`, and driving a scenario from `window.on_next_frame`.
- A custom grid element: `TermElement` in `spikes/chat-term/app/src/bin/term.rs` (runs shaped with `force_width`,
  wide glyphs placed alone, merged backgrounds, selection from the emulator's model), and coalescing PTY wake-ups with
  an atomic flag before notifying GPUI.
- A terminal session over `alacritty_terminal` that writes the emulator's replies back to the PTY
  (`spikes/chat-term/core/src/pty.rs`), and the key and paste encoding table in `spikes/chat-term/core/src/keys.rs`.

### What the framework lacks or fought

- No text selection or copy for `StyledText`: the plain renderer cannot select at all. The kit's `TextView` selects
  and copies within a message, and the kit has a window-scoped selection layer meant to span elements; selection
  across messages in a virtualised list (where rows come and go) was not exercised.
- Shaped text is cached only from one frame to the next, which makes rows that re-enter the view pay again.
- Rows that have never been laid out have no height, so a scrollbar cannot be exact without measuring everything.
- Inactive windows are limited to 30 fps by default (`WindowOptions::inactive_frame_interval`), which is right for the
  product but has to be turned off to measure.
- Links: drawn with colour and underline; clicking them needs GPUI's `InteractiveText` (present, not tried here).
- Code blocks: no highlighting in the plain renderer (the kit offers tree-sitter or syntect behind features, not
  enabled here); long lines wrap at the bubble's width instead of scrolling sideways.
- The terminal reads `KeyDownEvent`s only: dead keys and input methods need GPUI's input handler, not built. On macOS
  the Option key is treated as Meta.
- Zero-width-joiner emoji are split by `alacritty_terminal` into separate wide cells (as in many terminals).

## What remains unverified

- Everything visual on Linux and Windows. The runners have no display, so nothing visual was verified there: no window,
  frame time or glyph. CI (workflow run 44093008374 on commit 332cbf2, all three jobs green) shows that `chat` and
  `term` build in release on macOS, Linux and Windows, that `--features kit,profiler` type-checks there, and that their
  windowless self-tests pass (`--selftest`: every message's text runs cover its text on character boundaries; GPUI
  keystrokes map to the expected bytes). `headless-check` (markdown, keys, the emulator, both PTY backends with real
  child processes) passed with 48 of 48 on macOS, 46 passed and 0 failed on Linux with 2 warnings, and 38 passed and 0
  failed on Windows with 2 warnings. The Linux warnings are the `vi` resize check, which is intermittent (on both
  backends in that run, `vi` still reported 80x24 after a resize to 70x18 and the check timed out; on `portable-pty`
  only in a later run, 44097237928); the Windows warnings are the 33-key raw keystroke round trip on both backends,
  which stopped at the key "z" with no reply, so it is an open question for the real terminal on Windows. Details are in
  [0009](0009-pty-crate.md). Task 13 looks at rendering without hardware.
- What the frames look like on macOS: no screenshots were taken (the trial may not capture the screen), so glyph
  fallback for emoji and CJK, right-to-left shaping of the Arabic and Hebrew messages, wrapping of long lines and the
  terminal's colours were checked only through the emulator's state and the app's own records. The maintainer's
  hands-on check at the gate covers this.
- The reference machine of section 9 (a laptop, possibly 120 Hz) and a quiet machine: these runs shared the machine
  with other work.
- Presentation time: the instrumentation stops at the end of the paint phase; the next frame signal bounds when the
  frame was on the glass (at start-up: 12 ms later at the median).
- A thread read from disk and real provider streams; images, tool cards and other rich rows in the list.
- Selection across messages, link clicks, syntax highlighting, IME, screen readers.

## Revisit when

The GPUI snapshot is upgraded (re-run `run-macos.sh` and `run-term-macos.sh` and compare); Task 13 shows rendering on
Linux or Windows; the reference laptop is available for the gate measurements; the real thread's rows (tool calls,
images, diffs) replace generated markdown; GPUI or the kit gains a persistent text-layout cache or cross-element
selection; or the scroll target is redefined as the baseline record proposes (a share of slow frames rather than
none).
