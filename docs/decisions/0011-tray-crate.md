# 0011: Tray crate

Status: accepted
Date: 2026-11-17

## Context

GPUI has no tray or status-item API (decision 0007, "What remains unverified"; design section 5.10), so the tray
icon and its popover come from a separate crate, chosen in the M0 platform trial (Task 11, decision 0010). The crate
must put an icon in the macOS menu bar, the Windows notification area and the Linux tray; report a click (and where
the icon is, so a popover window can open under it) or at least show a menu; and live beside GPUI's event loop, which
on macOS is `NSApplication`, on Windows a `GetMessageW` loop on the main thread, and on Linux a `calloop` loop with no
GTK main loop. Constraints: the licence policy in `deny.toml` (allow list; no GPL, AGPL or LGPL-only crate;
vulnerabilities, unsound notices and yanked crates fail everywhere; unmaintained notices fail only for direct
dependencies), exact pins, Rust 1.100.0, and the provenance rule.

Facts were checked on 2026-11-17 against crates.io's API, the crates' source in the cargo registry, the projects'
repositories, and a snapshot of the RustSec advisory database taken the same day. The licence check of the trial's
dependency graph was run here from `cargo metadata` against the allow list; cargo-deny itself was not run on the trial.

## Options considered

| Option | Trade-off |
|---|---|
| A. `tray-icon` 0.26.1 (chosen) | One API on all three; maintained (Tauri), widely used; native on macOS and Windows; on Linux either AppIndicator (GTK 3, LGPL system libraries, no click events, an unsound advisory in its tree) or KSNI (no GTK, but the `ksni` crate is Unlicense). |
| B. `ksni` 0.3.7 directly | Linux only, pure Rust over D-Bus, click events; licence Unlicense, not on the allow list. 0.3.7 was released 2026-10-20 (crates.io API, checked 2026-11-17); the previous releases were 2026-07-15, 2026-06-10, 2026-04-10 and 2025-12-19, so it is actively maintained, with about 2.4 million downloads. Still needs a macOS and a Windows crate. |
| C. `tray-item` 0.10.0 | Last release 2024-04-07; old Objective-C bindings (`cocoa`, `objc`); GTK or KSNI on Linux. |
| D. `trayicon` 0.4.1, `tray-indicator` 0.4.2 | Windows only. |
| E. `system_status_bar_macos` 0.1.3 | macOS only, last release 2023-07-20. |
| F. Own code | `NSStatusItem` through `objc2-app-kit` and `Shell_NotifyIconW` through `windows-sys` (both already in GPUI's tree), and a StatusNotifierItem over `zbus` 5 (already in GPUI's Linux tree): no new licence, more code to own and test. |

### A. tray-icon

- Who and what: github.com/tauri-apps/tray-icon, 0.26.1 published 2026-10-07, MIT OR Apache-2.0, MSRV 1.90, about
  45 million downloads. Menus come from `muda` 0.21.2 (same project and licence). Linux backends are crate features:
  `libappindicator` (default) and `ksni`.
- macOS: an `NSStatusItem` with a view of its own inside the button that turns mouse events into `TrayIconEvent::Click`
  with the cursor position and the icon's rectangle (`src/platform_impl/macos/mod.rs`). It must be created on the
  main thread after launch, which GPUI's `run` callback satisfies, and AppKit delivers its events through the same
  run loop. **Verified by the trial (a):** the icon was created, a synthetic click on its view reached a GPUI task
  through `TrayIconEvent::set_event_handler` and an `async_channel`, with the rectangle; a popover window opened under
  it and read back there; dropping the icon removed it (its view left the button and AppKit deallocated the status
  item). The rectangle is global, in physical pixels; mapping it to a GPUI display needs AppKit (decision 0010, E15).
- Windows: `Shell_NotifyIconW` with a hidden message window; clicks arrive as its callback message and become
  `TrayIconEvent::Click` with the icon's rectangle from `Shell_NotifyIconGetRect` (`src/platform_impl/windows/mod.rs`).
  Created on GPUI's main thread, its messages are dispatched by GPUI's own `GetMessageW` loop, which serves every
  window of the thread (`gpui-pre-windows-0.3.12/src/platform.rs`). (b): not run; the trial's `--selfcheck` posts the
  click message for a Windows tester.
- Linux, AppIndicator backend: `libappindicator` 0.9.0 links GTK 3 through `gtk` 0.18.2 and loads
  `libayatana-appindicator3.so.1` or `libappindicator3.so.1` at run time, and panics if neither is installed
  (`libappindicator-sys` `src/lib.rs`). GTK needs its main loop on the icon's thread, which GPUI does not run, so the
  trial runs the icon on its own thread with `gtk::init` and `gtk::main` (`spikes/platform/src/tray.rs`). This backend
  emits no click events at all (tray-icon documents it on `TrayIconEvent`): the icon shows its menu and nothing else,
  so there is no popover. Building needs `libgtk-3-dev` (CI installs it). (b): compiled in CI only.
- Linux, KSNI backend: `ksni` 0.3.7 publishes a StatusNotifierItem over D-Bus (`zbus` 5) on a worker thread, with no
  GTK; it emits left- and middle-click activation. `tray-icon`'s backend implements `activate(x, y)` and
  `secondary_activate(x, y)` and passes the host's x and y on as the click's `position`
  (`tray-icon-0.26.1/src/platform_impl/ksni/mod.rs` lines 171 to 192), but it sets the icon rectangle to the default,
  so there is a position hint and no rectangle. Some hosts, and Wayland sessions, may pass 0,0; reliability across
  hosts is not verified. The host handles right clicks itself. It needs a StatusNotifierItem host: KDE Plasma and most other desktops have one; GNOME
  shows these icons only with the AppIndicator extension (Ubuntu enables it by default). Not verified here.
  (b): type-checked in CI only.
  With no status-notifier host running, the trial did not check what the icon or the crate does (not verified).
  Proposal, not tested: the application detects that no host owns the `org.kde.StatusNotifierWatcher` D-Bus name (or
  that creating the icon fails) and falls back to a normal window, so the app never depends on the tray being visible.
- Licences and advisories of the trial's graph against `deny.toml`:
  - Default (AppIndicator) backend: every crate's licence is allowed (the new ones are MIT, Apache-2.0, ISC and
    combinations). But it brings `glib` 0.18.5, which has **RUSTSEC-2024-0429** (unsound, `glib::VariantStrIter`;
    fixed in 0.20). With `unsound = "all"`, the repository's `cargo deny check` would fail on it. Also new:
    `proc-macro-error` 1.0.4, RUSTSEC-2024-0370 (unmaintained, deep in the tree, so a warning under the scope rule).
    The gtk-rs GTK 3 "unmaintained" notices (RUSTSEC-2024-0412 to 0420) were withdrawn on 2026-08-14, when that
    project resumed work. Whether `tray-icon` or `muda` ever reaches the unsound function was not checked.
  - The system libraries this backend needs are LGPL: GTK 3 (its `COPYING` on the `gtk-3-24` branch is the GNU LGPL
    version 2 text) and libayatana-appindicator (its repository carries `COPYING.LGPL.2.1`, and GitHub detects
    LGPL-3.0). They are dynamically linked system packages, not crates, so `deny.toml` does not see them: this is a
    packaging and licence question for the `.deb` and tarball (Task 14), not a crate-policy failure.
  - KSNI backend: no GTK and no new advisory, but `ksni` is licensed **Unlicense**, which is not on the allow list,
    so the licence check would fail until the maintainer allows it.

### F. Own code

The trial's macOS probes already drive `NSStatusItem` objects through `objc2-app-kit`; a macOS-only status item is a
few dozen lines. A Windows notification icon is a hidden window plus `Shell_NotifyIconW`. A Linux StatusNotifierItem
is a D-Bus object with roughly ten properties and methods (an estimate, not counted here), plus a DBusMenu for its menu, on `zbus` 5, which GPUI's
Linux tree already contains. Together, a few hundred lines that this project would own, with no licence change; the
DBusMenu part is the largest. Not written in the trial.

## Decision

Use `tray-icon` 0.26.1, pinned exactly, for macOS and Windows. On Linux use its **KSNI backend** (pure Rust, no GTK,
click events with a position hint), which needs the maintainer to add `Unlicense` to the allow list in `deny.toml`. If the maintainer
declines, the Linux tray is written as a small StatusNotifierItem over `zbus` (option F) rather than taking the
AppIndicator backend, whose `glib` 0.18.5 fails `unsound = "all"` and which brings GTK 3 and LGPL system libraries
for an icon that cannot report a click. Until one of the two lands, the Linux tray is a section 8 exception.

## Who decided

A design call from the M0 trial. Allowing the Unlicense licence is the maintainer's decision, as every change to the
allow list is.

## Consequences

- One tray API across platforms; the popover is a GPUI `WindowKind::PopUp` window the app positions from the icon's
  rectangle on macOS and Windows (on macOS through AppKit's display arrangement, decision 0010).
- On Linux the tray cannot place a popover reliably: KSNI gives a position hint from the host (possibly 0,0, and no
  icon rectangle), and on Wayland no client can place a window. Treat the Linux tray as menu-first: the icon's menu,
  or a click that opens a normal window. This still follows, because a hint that may be 0,0 cannot anchor a window,
  though a window placed from the hint on X11 hosts that fill it in is possible and untried.
- GNOME users need the AppIndicator extension to see any tray icon; the app must work without one.
- `tray-icon`'s Linux backends are crate features that can change between releases; every upgrade rechecks the
  backend, its licence and its advisories.
- Tray events reach GPUI through `TrayIconEvent::set_event_handler` and `MenuEvent::set_event_handler` into an
  `async_channel` awaited by a foreground task: no polling (`spikes/platform/src/tray.rs`). The handlers are
  process-wide and can be set once.
- The icon must be removed before the app exits; on macOS `cx.quit()` ends the process from AppKit, so the app drops
  the icon first.

### What remains unverified

- Windows: the icon, the click and its rectangle were not run anywhere (CI has no visible notification area and does
  not open windows).
- Linux: neither backend was run; which desktops show a KSNI icon, whether hosts pass real click coordinates, and how
  either backend behaves with no tray host (proposed fallback above, untested).
- macOS: a person clicking the icon and the right-click menu (checklist step 8), and whether GPUI's timers keep running
  while that menu is open.
- Whether the AppIndicator backend's code paths reach the unsound `glib` function.

## Revisit when

A `tray-icon` release changes its Linux backends or moves to a `glib` without the advisory; the maintainer decides on
the Unlicense licence; GPUI gains tray support; or a Linux or Windows tester reports how the icon behaves.
