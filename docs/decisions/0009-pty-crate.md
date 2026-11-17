# 0009: PTY crate

Status: accepted
Date: 2026-11-17

## Context

Design section 5.7 puts terminals on a PTY layer (ConPTY on Windows), `alacritty_terminal` for emulation and our own
GPUI element for the grid, and leaves the PTY crate to the M0 trial. Section 8 names ConPTY for Windows and leaves macOS
and Linux to the trial. Section 5.10 says the core runs on Tokio and the UI thread never waits on I/O. The two
candidates the plan names are `portable-pty` and the `tty` module inside `alacritty_terminal`.

Method: the Task 10 spike (`spikes/chat-term/`, see [0008](0008-chat-and-terminal-trial.md)) wraps both behind one
session type (`core/src/pty.rs`) that feeds the same `alacritty_terminal::Term`, and exercises them two ways:

- `headless-check` (`core/src/bin/headless_check.rs`), with no display, for each backend: a helper child (this binary in
  `child` mode) in a real PTY that echoes a command, reports its terminal size before and after a resize, reads 33
  encoded keystrokes in raw mode and prints the bytes it received, prints 200,000 lines, and exits with code 7; the
  system shell (`/bin/sh`, or `cmd.exe` on Windows) running a command, reporting its size after a resize (`stty size`,
  or `mode con` on Windows) and exiting with code 3; and on Unix `vi` on the alternate screen, resized, then quit. It is
  the part of the trial the CI workflow `.github/workflows/spike-chat-term.yml` runs on macOS, Linux and Windows.
- The GPUI terminal (`app/src/bin/term.rs`) with each backend: correctness checks through the real element, a
  200,000-line burst, keystroke-to-echo latency and idle CPU (numbers in 0008).

Facts about the crates were read from crates.io and the projects' GitHub repositories on 2026-11-17, and from the
crates' own source in the cargo registry.

### Sources and provenance

Read as reference, all permissively licensed: the source of `alacritty_terminal` 0.26.0 (its `tty` module for Unix and
Windows including ConPTY, the event loop, `Term`, the grid, selection and events), `vte` 0.15.0, and `portable-pty`
0.9.0 (the library, its command builder and the Windows pseudoconsole code), plus `libc` 0.2.191, all from the cargo
registry; and crates.io and GitHub API pages for versions, release dates, licences, wezterm commits under `pty/`, wezterm
issues #8338, #6783, #7025, #4205 and #4206, and Alacritty commits under `alacritty_terminal/` and its open ConPTY
issues. The spike's session type calls both crates through their public APIs; the spike source carries no credit for copied code and the trial's report records
none. Nothing was
read or copied from the editor project's crates that are not permissively licensed (its terminal view, markdown and
editor crates in particular).

## Options considered

| Option | Trade-off |
|---|---|
| A. `portable-pty =0.9.0` (wezterm) | Small blocking API (`Read`, `Write`, `resize`, `wait`), easy to wrap; but the last release is 21 months old with Windows fixes unreleased, and its ConPTY flags require the host to answer a cursor-position request at start. |
| B. `alacritty_terminal::tty` with `alacritty_terminal`'s `EventLoop` (chosen) | Already the emulator dependency, released this year, ConPTY created without handshake flags, and its I/O thread already parses into the `Term` with lock fairness, read coalescing and synchronized-update handling; but it owns its threads, installs a process-wide SIGCHLD handler on Unix, and its API is shaped for Alacritty. |

### Evidence

| Point | `portable-pty` 0.9.0 | `alacritty_terminal::tty` 0.26.0 |
|---|---|---|
| Builds and headless check, macOS (local, release, 3 runs) | Builds; every hard check passes in all 3 runs | Builds; every hard check passes in all 3 runs |
| Builds and headless check, Linux (x86_64, CI) | Builds; 46 passed, 0 failed, 2 warnings (the `vi` resize check, below) | Same: builds; 46 passed, 0 failed, 2 warnings (the `vi` resize check) |
| Builds and headless check, Windows (CI) | Builds; 38 passed, 0 failed, 2 warnings (the raw keystroke round trip, below) | Same: builds; 38 passed, 0 failed, 2 warnings (the raw keystroke round trip) |
| Builds and headless check, macOS (CI, same workflow run) | Builds; 48 passed, 0 failed, 0 warnings | Same: 48 passed, 0 failed, 0 warnings |
| Licence | MIT. New transitive crates: `nix` 0.28 (MIT), `serial2` (BSD-2-Clause OR Apache-2.0), `shared_library` 0.1.9 (Apache-2.0/MIT, last released 2018), `winreg` 0.10 (MIT), `shell-words` (MIT/Apache-2.0) | Apache-2.0. New transitive crates: `vte` 0.15 (Apache-2.0 OR MIT), `rustix-openpty` (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT), `signal-hook` (MIT OR Apache-2.0), `miow` (MIT OR Apache-2.0), `home`, `cursor-icon`, `windows-sys` 0.59 |
| Releases | 0.9.0 on 2025-02-11, 0.8.1 on 2023-03-13. Lives in the wezterm monorepo, whose last tagged release is from 2024-02 | 0.26.0 on 2026-04-06, 0.25.1 on 2025-10-18; commits to the crate in August, October and November 2026 |
| Open issues that matter | wezterm #8338 (2026-11-14): 0.9.0's Windows `kill()` reports success as failure and failure as success; fixed on main 2026-06-07, not released. #6783 (open since 2025-03): "0.9.0 doesn't work on Windows", which the maintainer traced to ConPTY's cursor-position request going unanswered; downstream projects carry workarounds. #4205/#4206 (2023): PATH and write problems on Windows. 26 open issues mention the crate. The move from `winapi` to `windows-sys` (2026-08) is also unreleased | In the Alacritty repository: 31 open issues mention ConPTY, most of them old and about Alacritty's behaviour rather than the crate's API (for example #8147, a freeze on heavy colour output on Windows 10) |
| ConPTY | Prefers a side-loaded `conpty.dll` if the library search finds one (for example next to the program), else `kernel32`. Creates the console with `PSEUDOCONSOLE_INHERIT_CURSOR \| RESIZE_QUIRK \| WIN32_INPUT_MODE`: ConPTY then sends `ESC[6n` at start and waits for the reply, and asks for win32-input-mode keys | Prefers a `conpty.dll` if the library search finds one (its comments mention the search path), else `kernel32`. Creates the console with flags 0: no start-up handshake |
| Resize | `MasterPty::resize(PtySize)` from whichever thread owns the master (our control thread) | `Msg::Resize` to the event loop (`TIOCSWINSZ` or `ResizePseudoConsole`). In both, the `Term` grid is resized by the caller |
| Child exit | Blocking `Child::wait()` on a thread of ours gives the exit code. On Windows the reader sees end of file only after the pseudoconsole is closed, so the session closes the master when the child exits | Unix: a SIGCHLD handler (via `signal-hook`, process-wide) wakes the event loop, which reports `ChildExit(status)` then `Exit`; Windows: a wait registration on the process handle. `Pty`'s `Drop` sends SIGHUP and then blocks in `wait()` |
| Async or blocking, fit with Tokio | Blocking only: needs dedicated threads (or `spawn_blocking`); on Unix the master's raw fd could be wrapped in Tokio's `AsyncFd` (not tried) | Its own `polling`-based I/O thread (non-blocking fd on Unix, blocking pipes with helper threads on Windows). It does not run on Tokio; the core bridges its events into channels, as `core/src/pty.rs` does with a callback |
| Measured (macOS, release, 3 runs) | Keystroke round trip through the PTY, median 0.024 to 0.025 ms. 200,000 lines from the child: 33.7 to 36.4 ms, 2,448 to 2,449 wake-up events (one per read) | 0.024 to 0.055 ms. 26.4 to 36.6 ms, 72 to 75 wake-up events (reads are coalesced up to 64 KiB under one lock) |

What bit us: with no explicit shell, `alacritty_terminal::tty` on macOS runs the shell through `/usr/bin/login -flp`
(so it appears as a login session), and its `setup_env` helper sets `TERM` and `COLORTERM` in the whole process with
`set_var`; the spike passes the program and the variables explicitly instead. Both crates leave answering the
emulator's replies (`ESC[6n`, device attributes) to the host; the session writes `Term`'s `PtyWrite` events back into
the PTY, which is what makes `portable-pty` usable with ConPTY at all. `portable-pty`'s reader produced a wake-up per
read, about 34 times as many as `alacritty_terminal`'s loop for the same burst, so a host must coalesce them (the GPUI
terminal does, with an atomic flag); `alacritty_terminal`'s loop also implements the synchronized-update timeout (DEC
mode 2026) that a hand-written reader would have to copy.

## Decision

Use `alacritty_terminal`'s own `tty` module and `EventLoop` for the PTY on all three platforms, pinned exactly with the
emulator (`alacritty_terminal = "=0.26.0"`), behind a session type of our own in `udx-term` so the backend can be
replaced. `portable-pty` is not adopted.

## Who decided

A design call, within design section 5.7 (the PTY crate is chosen in the M0 trial).

## Consequences

- One crate for emulation and PTY, released together; no second set of Windows console code to follow.
- `udx-term` gets a dedicated I/O thread per terminal, owned by `alacritty_terminal`, and forwards its events (new
  content, title, child exit) into the Tokio core and on to GPUI. The UI thread only locks the `Term` to draw.
- On Unix, `alacritty_terminal` installs a SIGCHLD handler through `signal-hook`. Other code in the process that
  reaps children with a blanket `waitpid(-1)` would race with it; the engine's own child processes (tools, add-on
  engines) must wait on their specific process ids, which `std::process` and Tokio do.
- Dropping a session blocks until the child is reaped after SIGHUP; shutting a terminal down belongs on a worker thread,
  not the UI thread.
- The API is shaped for Alacritty (for example `EventLoop` takes the `Term` behind its `FairMutex`); an upgrade of
  `alacritty_terminal` is its own change, like a GPUI upgrade, and is checked with `headless-check`.
- `portable-pty` remains the fallback if `alacritty_terminal`'s PTY fails on a platform; the spike's session type shows
  the swap is about 120 lines.

## What remains unverified

- The Windows raw keystroke round trip. On both backends it stopped at the key "z" with no reply (33 keys sent), so it
  does not separate the two options, and whether a real terminal on Windows passes every key through ConPTY intact is
  an open question. The checks that did pass on Windows on both backends: spawn and first output, a command's output,
  the 80x24 size, a resize to 100x30, the 200,000-line burst arriving complete, 10,000 lines of scrollback, exit
  codes 7 and 3, the reader ending after the child exits, and `mode con` reporting 90 columns and 20 lines after a
  resize.
- The `vi` resize check on Linux. On both backends `vi` (vim-tiny 9.1) switched to the alternate screen, showed typed
  text and quit correctly, but after a resize to 70 by 18 it still reported 80 columns and 24 lines and the check
  timed out. The other Linux resize checks passed on both backends (the child helper at 100x30 and `stty size`
  reporting 20 90), so the PTY layer does carry the resize; why `vi` did not see it was not determined. On macOS the
  same check passed on both backends, locally and in CI.
- That `portable-pty` works with ConPTY when the host answers the cursor-position request (the session does): the
  Windows CI run shows it spawning, echoing, resizing, bursting and reporting exit codes, so it does work for those,
  but the raw key round trip, the one check on which a ConPTY flag choice would matter for input, failed there on both
  backends.
- Nothing visual on Linux and Windows: the runners have no display, so no window, frame or glyph was verified there.
- The macOS CI job ran the same workflow and passed with no warnings (see the table above).
- The GPUI terminal on Linux and Windows (no display on the runners; Task 13 looks at rendering without hardware).
- Real-world shells on Windows beyond `cmd.exe` (PowerShell, WSL), a side-loaded `conpty.dll`, and how either crate
  behaves under a screen reader.
- Long-running behaviour: many terminals at once, a child that ignores SIGHUP, a terminal open for days.

## Soft checks in the headless check

A soft check is reported as a `[WARN]` line and does not fail the run; the spike's workflow now also turns each into a
CI annotation and a step summary entry, so none passes unseen. Why each is soft, and what CI showed on commit 332cbf2
(workflow run 44093008374):

| Soft check | Why it is soft | macOS | Linux | Windows |
|---|---|---|---|---|
| Raw keystroke round trip (33 keys through a child in raw mode) | Windows translates input into console records and back; expected to be platform-uncertain. Hard on Unix | pass both backends | pass both | WARN both: stopped at "z", no reply |
| Scrollback stays at 10,000 lines after the 200,000-line burst | ConPTY repaints from its own buffer, so what reaches the emulator is a finding there. Hard on Unix | pass | pass (hard) | pass both |
| Shell resize reported by `mode con` (Windows only; Unix uses `stty size`, hard) | The output format of `mode con` is unproven | not applicable | not applicable | pass both |
| Reader ends after the child exits | Marked soft on every platform because end-of-file timing differs by platform (on Windows the reader ends only after the session closes the master) | pass | pass | pass both |
| `vi` on the alternate screen: draws the file; typed text appears; resize reported by `vi`; quitting leaves the alternate screen (Unix only) | Depends on a `vi` being installed and on its behaviour; soft on every Unix platform | pass all four, both backends | pass three; the resize check WARN on both backends (timed out, `vi` reported 80x24) | not run |

## Revisit when

CI shows `alacritty_terminal`'s PTY failing on Linux or Windows while `portable-pty` passes; `alacritty_terminal`
stops releasing or changes its `tty` or `EventLoop` API in a way that costs more than a small adapter; `portable-pty`
publishes a release with its Windows fixes and a choice of ConPTY flags; or the engine needs PTY sessions on Tokio
directly (for example thousands of short-lived ones) rather than one thread per terminal.
