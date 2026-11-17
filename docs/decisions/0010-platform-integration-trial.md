# 0010: Platform integration trial

Status: accepted
Date: 2026-11-17

## Context

The design (sections 2, 5.10 and 8) assumes the pinned GPUI snapshot (decision 0007, `gpui-pre` 0.3.12) can carry a
real desktop application on macOS, Linux and Windows: text entry with input methods and dead keys, a tray icon with a
popover (GPUI has none, so a separate crate), the application menu, several windows, open and save dialogs,
`unidex://` links with a hand-off to the running instance, and a quit gate. Goal G10 makes every screen usable from
the keyboard and lists screen readers as an exception "until GPUI provides it"; section 8 says GPUI's screen-reader
support is unconfirmed and M0 tests it. M0 Task 11 tested all of this in a throwaway trial, `spikes/platform/`.

The trial could not operate the machine as a person does, so every result says how it was verified:

- **works (a)**: verified by the program itself. `a/os`: the app called the OS-facing entry point of its own objects
  (the method the OS calls, such as `setMarkedText:` on GPUI's view or `application:openURLs:` on its delegate) and
  read the result back through AppKit or GPUI. `a/gpui`: the app drove GPUI's own entry point (key dispatch, the
  `InputHandler` trait the platform layer wraps, window and menu APIs) and read the result back.
- **implemented, not run (b)**: the platform layer implements it for that OS (file and function cited from the
  pinned crates); nothing ran.
- **needs the maintainer (c)**: a step in `spikes/platform/CHECKLIST.md`.
- **missing**: the platform layer does not provide it; **broken**: it runs and gives a wrong result.

Method. `cargo run -- --selfcheck` opens the trial's windows and tray icon for about 8 seconds, runs 35 checks and
cleans up; it passed 35 of 35 on an Apple silicon Mac (U.S. keyboard layout, more than one display attached). `cargo test` runs the
text-field checks headless on GPUI's test platform. `.github/workflows/spike-platform.yml` builds the trial and runs
the headless parts on `macos-26`, `ubuntu-26.04` and `windows-2025`; its result belongs to the pull request (CI is
the only evidence that the Linux and Windows code even compiles). Sources read, all permissively licensed: the pinned
`gpui-pre-*` 0.3.12 crates (GPUI core, `gpui-pre-macos`, `gpui-pre-linux`, `gpui-pre-windows`) and their examples
(`input`, `a11y`, `set_menus`), `accesskit_macos` 0.26.3, `accesskit_windows` 0.34.0, `accesskit_unix` 0.22.1,
`tray-icon` 0.26.1, `muda` 0.21.2, `libappindicator-sys` 0.9.0, and the widget kit's `gpui-base` 0.10.0. None of the
editor's non-permissive crates was read. References below are to files in the cargo registry, for example
`gpui-pre-macos-0.3.12/src/window.rs`.

## Options considered

The trial answers three questions the design left open.

| Option | Trade-off |
|---|---|
| Screen readers: keep the blanket exception on all platforms | Simple, but no longer true on macOS: the app publishes a working accessibility tree (below). |
| Screen readers: narrow the exception to what is unverified (chosen) | macOS counts as supported once the maintainer's VoiceOver step passes; Linux (AT-SPI) and Windows (UI Automation) are implemented in GPUI but unverified, like everything Tier 2; the app takes on the rules below. |
| Screen readers: drop the exception | Claims Linux and Windows without evidence. |
| Text fields: own minimal field | The trial's `text_field.rs` shows the whole contract is small; but selection by mouse, word movement, undo, wrapping and bidirectional text are all extra work. |
| Text fields: the widget kit's input (`gpui-base`, passes the licence policy alone, decision 0007) (recommended) | Implements the same input-handler contract and already publishes text runs for screen readers (`input/base/a11y_text.rs`); one more pinned dependency. |
| Links on Linux and Windows: rely on GPUI | Not possible: GPUI stores the callback and never calls it there (below). |
| Links on Linux and Windows: the app reads its arguments and hands them to the running instance itself (chosen) | A small single-instance channel per OS (a Unix socket or D-Bus name on Linux, a named pipe on Windows) owned by the app. |

## Decision

The snapshot carries the application on macOS with the gaps and rules listed below; Linux and Windows have the same
platform-layer coverage in source but are unverified beyond compiling (CI) and GPUI-level tests. The tray comes from
`tray-icon` (decision 0011). Section 8's screen-reader exception narrows to Linux and Windows only once the maintainer
has used VoiceOver on the macOS app (checklist step 13); until then macOS is verified through the OS accessibility API
only. The application supplies, itself: link delivery and the single-instance hand-off on Linux and Windows, in-window
menus on Linux and Windows, a macOS quit gate that also covers the Dock and log-out, and the global display arrangement
on macOS.

### Results by feature and platform

Linux has two columns because GPUI has a separate backend for each display server.

| Feature | macOS | Linux X11 | Linux Wayland | Windows |
|---|---|---|---|---|
| Typed text reaches a field (key event, then the field's input handler) | works (a/gpui) [E1] | (b) [E8] | (b) [E9] | (b) [E7] |
| Composition: marked text, updates, commit | works (a/os) [E2]; typing with a real input method (c) step 3-4 | (b) XIM [E8] | (b) text-input-v3 [E9] | (b) IMM32 [E7] |
| Candidate window placed at the marked text | works (a/os) [E3]; seen by a person (c) step 3 | (b) [E8] | (b) [E9] | (b) [E7] |
| Dead keys | works at the text-input level (a/os) [E4]; on the keyboard (c) step 6 | (b) xkb compose [E8] | (b) xkb compose [E9] | (b) [E7] |
| Cancelled composition | works (a/os) [E5]; Escape in an input method (c) step 5 | (b) [E8] | (b) [E9] | (b) [E7] |
| Key repeat, press-and-hold | (b), (c) step 7 [E7a] | (b) X server autorepeat [E8] | (b) GPUI's own repeat [E9] | (b) [E7] |
| Bound shortcuts consume keys, unbound keys become text | works (a/gpui) [E1] | (b) same GPUI core: `Window::dispatch_keystroke` and the key bindings are platform independent (`gpui-pre-0.3.12/src/window.rs`) | (b) same | (b) same |
| Tab and Shift-Tab between fields | works (a/gpui) [E10] | (b) same GPUI core: `Window::focus_next` and `focus_prev` (`gpui-pre-0.3.12/src/window.rs`) | (b) same | (b) same |
| Tray icon created and removed | works (a/os), decision 0011 | (b) AppIndicator or KSNI | (b) same | (b) |
| Tray click reaches the app, with the icon's position | works (a/os): a synthetic mouse event sent to the tray crate's own view, not a click by a person (checklist step 8) | missing with AppIndicator (no click events); KSNI: click with a host-supplied position hint and no icon rectangle, reliability across hosts not verified [E24] | same as X11; the hint may be 0,0 [E24] | (b), click and position (a self-check exists for a Windows tester) |
| Popover window placed under the icon | works (a/os) [E15]; seen by a person (c) step 8 | (b) override-redirect window [E15] | missing: a client cannot place a window [E15] | (b) [E15] |
| Native application menu with shortcuts | works (a/os) [E11]; seen by a person (c) step 9 | missing natively: the app draws it [E12] | missing natively [E12] | missing natively [E12] |
| Shortcuts through the keymap | works (a/gpui) | (b) same GPUI core (`Window::dispatch_keystroke`) | (b) same | (b) same |
| Second window: open, list, activate, close | works (a/gpui) [E13] | (b) `open_window`, `active_window`, `window_stack` (`gpui-pre-linux-0.3.12/src/linux/x11/client.rs`) | (b) the same three (`wayland/client.rs`) | (b) `open_window` and `active_window` (`gpui-pre-windows-0.3.12/src/platform.rs`); no `window_stack` found there, not checked |
| Window on a chosen display | works (a/gpui) [E13] | (b) `display_id` handled in `x11/window.rs` (line 484) | missing (compositor decides) | (b) `display_id` handled at window creation in `gpui-pre-windows-0.3.12/src/window.rs` (line 520) |
| Global position of displays | missing in GPUI: every display's origin is 0,0 [E13] | not checked | n/a | not checked |
| Window state for restoring | broken if used naively, works with a correction (a/gpui) [E14] | (b) `window_bounds` in `x11/window.rs`; nothing else checked | (b) `window_bounds` in `wayland/window.rs`; nothing else checked | (b) `window_bounds` in `gpui-pre-windows-0.3.12/src/window.rs`; nothing else checked |
| Last window closed, quit, quit gate | (b) gate gaps [E16] | (b) [E16] | (b) [E16] | (b) [E16] |
| Open and save dialogs | (b) native panels [E17], (c) steps 11-12 | (b) portal only [E17] | (b) portal only [E17] | (b) shell dialogs [E17] |
| `unidex://` declared | metadata built and linted, not registered (`plutil -lint` shows only that the property list parses); registration is optional checklist step (c) [E18] | desktop entry written, untested (CI runs `desktop-file-validate`, a syntax check) [E18] | same | `.reg` file written with a hard-coded install path, untested [E18] |
| A link reaches the running app | works at the app delegate's `application:openURLs:` entry (a/os), not through the OS's launch services [E19]; real link (c) optional | missing in GPUI: the app reads its arguments [E20] | same | same |
| Second launch hands the link over | (b) LaunchServices does it for a bundle; (c) optional | missing [E20] | missing [E20] | missing [E20] |
| Accessibility tree: roles and labels | works (a/os) [E21]; VoiceOver itself (c) step 13 | (b) AT-SPI [E23] | (b) AT-SPI [E23] | (b) UI Automation [E22] |
| A text field's contents | works (a/os): value always; characters and caret only with text runs [E21] | (b) | (b) | (b) |
| Focus reported to assistive technology | works (a/os) [E21] | (b) | (b) | (b) |
| Plain strings and styled text | missing: not published unless the app adds nodes [E21] | same (GPUI core) | same | same |

### Evidence

- **E1** Self-check: `Window::dispatch_keystroke` with unbound keys put "ab" into the field through the input handler
  registered on the real macOS window; a bound shortcut (Cmd-Shift-H) ran its action and inserted nothing. GPUI core:
  `gpui-pre-0.3.12/src/window.rs` `dispatch_keystroke`. The headless tests repeat this on every CI runner.
- **E2** Self-check, through GPUI's own view: `setMarkedText:selectedRange:replacementRange:` three times
  ("k", "か", "かん"), then `insertText:replacementRange:`. `markedRange` went (2,1) then (2,2), `selectedRange`
  (4,0), the field showed "abかん" while composing and "ab漢" after; `hasMarkedText` false after. Implementation:
  `gpui-pre-macos-0.3.12/src/window.rs` `set_marked_text`, `insert_text`, `unmark_text`, `marked_range`,
  `selected_range`, and `handle_key_event`, which sends keys to `inputContext handleEvent:` while composing.
- **E3** `firstRectForCharacterRange:actualRange:` returned a 30 x 22 point rectangle inside the window. Asked before
  GPUI repainted the marked text, it returned a zero-width rectangle for the old text: a field must call
  `Window::invalidate_character_coordinates()` after each edit, which on the next frame tells the input method to ask
  again (`gpui-pre-macos-0.3.12/src/window.rs` `update_ime_position`, `NSTextInputContext
  invalidateCharacterCoordinates`). The trial's field does.
- **E4** The accent as marked text, then `insertText` with "é", gave "ab漢é". A real dead key depends on the keyboard
  layout's translation inside the OS input layer: a synthetic key event (Option-E, then E) sent to the window carried
  fixed characters and the input context did not translate it (it inserted "" and then "e"), so that step is (c).
- **E5** `setMarkedText` with an empty string, then `unmarkText`, left the text as before the composition.
- **E6** Headless tests (`spikes/platform/src/text_field.rs`, run by CI on three platforms): composition, a dead key,
  a cancelled composition and Enter, in both fields, through `ElementInputHandler` (the `InputHandler` GPUI's
  `PlatformInputHandler` wraps on every OS) and `dispatch_keystroke`, on GPUI's test platform.
- **E7** Windows, `gpui-pre-windows-0.3.12/src/events.rs`: `handle_keydown_msg` (key repeat from bit 30 of `lparam`),
  `WM_CHAR` to `handle_char_msg` (committed text; dead keys are composed by `TranslateMessage` in the message loop,
  `platform.rs` line 585), `WM_IME_STARTCOMPOSITION` to `handle_ime_position`, `WM_IME_COMPOSITION` to
  `handle_ime_composition_inner` (`GCS_COMPSTR` becomes marked text, `GCS_RESULTSTR` is committed), and
  `update_ime_position` (`ImmSetCompositionWindow`, `ImmSetCandidateWindow`). IMM32 only: no Text Services Framework
  code. **E7a** macOS: `handle_key_event` handles held keys and `apple_press_and_hold_enabled`.
- **E8** X11, `gpui-pre-linux-0.3.12/src/linux/x11/client.rs`: XIM through the `xim` crate with
  `PREEDIT_CALLBACKS` and a `SpotLocation` for the candidate window, `xim_handle_preedit`, `xim_handle_commit`; without
  an input method, dead keys through the xkb compose state (`compose_state.feed`). Repeats arrive as the X server's
  autorepeat presses.
- **E9** Wayland, `gpui-pre-linux-0.3.12/src/linux/wayland/client.rs`: `zwp_text_input_v3` (`PreeditString`,
  `CommitString`, `Done`; `set_cursor_rectangle` for the candidate window), xkb compose for dead keys, and key repeat
  generated by GPUI from the compositor's repeat rate. Needs a compositor with text-input-v3.
- **E10** Tab moved focus from Name to Notes and Shift-Tab back, once the field's tracked focus handle was itself a
  tab stop (`cx.focus_handle().tab_stop(true)`); `div().tab_stop(true)` affects only a `focusable()` div's own handle.
- **E11** `cx.set_menus` builds the NSMenu (`gpui-pre-macos-0.3.12/src/platform.rs` `set_menus`). Read back from
  `NSApp.mainMenu`: every item with its key equivalent (for example "Show Popover" shift-cmd-P). Performing "Trial >
  Say Hello" through `performActionForItemAtIndex:` ran the GPUI action.
- **E12** Linux and Windows `set_menus` only store the menus for `get_menus` (`gpui-pre-linux-0.3.12/src/linux/
  platform.rs`, `gpui-pre-windows-0.3.12/src/platform.rs`); nothing native is shown. The design must draw an
  in-window menu bar there. The Windows dock menu becomes a jump list (`destination_list.rs`); Linux's is a to-do.
- **E13** A second window opened, `cx.windows()` counted two, `window_stack` and `activate_window` worked, a window
  opened with another `display_id` reported that display, and closing left one window. GPUI reports every macOS
  display with origin 0,0 (`gpui-pre-macos-0.3.12/src/display.rs` `bounds` keeps only the size), so where displays sit
  relative to each other comes from AppKit (`spikes/platform/src/mac.rs` `locate_on_display`).
- **E14** `window_bounds()` on macOS returns the frame including the title bar, while `WindowOptions` takes the size
  as the content size: reopening with it grew the window by the title bar (252 to 284 points). Restoring with the
  origin from `window_bounds()` and the size from `viewport_size()` gave the same window. `native_window_state()`
  exists and was not tried.
- **E15** The tray click arrived with the icon's rectangle; a `WindowKind::PopUp` window opened at the computed
  position on the display holding the icon and read back there. The rectangle is in global coordinates from the main
  display's top left, in physical pixels; GPUI places windows relative to one display, in points. X11 makes PopUp
  windows override-redirect (`x11/window.rs`), Windows gives them a pop-up style (`window.rs`). Wayland's shell
  protocol has no request to place a toplevel window, and GPUI's Wayland window has no PopUp handling; its
  `LayerShell` kind works only on compositors with that protocol.
- **E16** `QuitMode::Default` keeps a macOS app running with no windows and quits elsewhere
  (`gpui-pre-0.3.12/src/app.rs`); the trial quits on the last window everywhere. `Window::on_window_should_close` can
  refuse a window's close, and the app's own Quit action can refuse to quit. But GPUI's macOS delegate implements no
  `applicationShouldTerminate:` (`gpui-pre-macos-0.3.12/src/platform.rs`, delegate methods), and `on_app_quit` cannot
  cancel, so Quit from the Dock and log-out bypass a quit gate on macOS.
- **E17** macOS: `NSOpenPanel` and `NSSavePanel` (`platform.rs` `prompt_for_paths`, `prompt_for_new_path`). Linux:
  the xdg-desktop-portal FileChooser through `ashpd`, with no fallback (`FILE_PICKER_PORTAL_MISSING`). Windows:
  `IFileOpenDialog` and `IFileSaveDialog` (`platform.rs`). Never opened by the self-check.
- **E18** `spikes/platform/bundle/`: `Info.plist` with `CFBundleURLTypes` (passes `plutil -lint`), a desktop entry with
  `MimeType=x-scheme-handler/unidex;` and `Exec=... %u`, and the `HKCU\Software\Classes\unidex` keys with `URL Protocol`
  and `shell\open\command`. Nothing was registered, so whether any OS honours the scheme is not verified. The Windows
  `.reg` file carries a hard-coded install path and is untested; the Linux desktop entry is untested.
- **E19** Calling the app delegate's `application:openURLs:` delivered `unidex://thread/42?from=selfcheck` to the
  callback registered with `Application::on_open_urls` (which must be registered before `run`).
- **E20** Linux and Windows store the `on_open_urls` callback and never call it (`linux/platform.rs`
  `on_open_urls`, `windows/platform.rs` `on_open_urls`); `register_url_scheme` returns "unimplemented" there. The app
  reads links from its arguments (`link_scheme::links_in_args`) and needs its own hand-off to a running instance.
- **E21** macOS, self-check through `NSAccessibility` on the window's content view, which AccessKit subclasses
  (`accesskit_macos` `SubclassingAdapter::for_window`, called from `gpui-pre-macos-0.3.12/src/window.rs` `a11y_init`).
  GPUI builds the tree only after the first question from assistive technology (`gpui-pre-0.3.12/src/window/a11y.rs`);
  the self-check's question activated it. Published: the labelled button as `AXButton` "Count"; the fields as
  `AXTextField` "Name" and `AXTextArea` "Notes" with their text as the value; `text!` text as `AXStaticText`; the
  heading with the role string "Heading"; the focused field as the focused element (when the window is key). An
  unlabelled button took its name from its `text!` child; the untitled popover window published its contents and has
  an empty title. Not published: plain `&str`, `String`, `SharedString` and `StyledText` children (only `Text` and
  `InteractiveText` have a role, `gpui-pre-0.3.12/src/elements/text.rs`). Without `TextRun` children a field reported
  0 characters and caret (0,0) although its value was "ab漢é"; with one run per line it reported 4 characters and the
  caret at 4, which a screen reader needs to read by character and follow the caret.
- **E22** Windows: `accesskit_windows::Adapter` (`gpui-pre-windows-0.3.12/src/window.rs` `a11y_init`), answering
  `WM_GETOBJECT` (`events.rs` `handle_wm_getobject`) as a UI Automation provider.
- **E23** Linux: `accesskit_unix::Adapter` for both X11 and Wayland windows (`x11/window.rs`, `wayland/window.rs`
  `a11y_init`), over AT-SPI on D-Bus, active when the AT-SPI bus reports it is enabled (`accesskit_unix` `context.rs`).
- **E24** Tray position on Linux, `tray-icon-0.26.1/src/platform_impl/ksni/mod.rs`: the KSNI backend implements
  `activate(x, y)` and `secondary_activate(x, y)` (lines 171 to 192), and `emit_click` sends the host's x and y as
  `position` in `TrayIconEvent::Click`, with `rect: Rect::default()`. So a click carries a position hint from the
  host but no icon rectangle. Whether hosts fill the hint in reliably, and whether Wayland sessions pass 0,0, was not
  verified.

### Other platform features (item 8, all (b) unless said)

- Clipboard: text, images and file lists on all three (`ClipboardEntry`; `pasteboard.rs`, `clipboard.rs`, the X11 and
  Wayland clipboards), plus the primary selection on Linux.
- File drag and drop into a window: macOS (`window.rs` dragging methods), Windows (`IDropTargetHelper`), X11 and
  Wayland (`client.rs`).
- System notifications: macOS UserNotifications, Linux D-Bus notifications, Windows toasts (`system_notifications.rs`
  in each backend). Not tried; macOS notifications normally need a bundled, signed app.
- Sleep and wake on all three (Linux through logind, `linux/platform.rs`), appearance and keyboard-layout changes,
  macOS thermal state and power mode. The self-check read the appearance and thermal state.
- Keychain, Secret Service and Credential Manager behind one API (`write_credentials`, `read_credentials`).
- Missing from the platform layer (no code found): system-wide shortcuts, a Dock or taskbar badge, start at log-in,
  and the tray. Each needs a separate crate or the app's own code.

## Who decided

A design call from the M0 trial, within decisions 0002 and 0007. Section 8's wording follows from it; the maintainer's
hands-on checklist confirms or corrects the macOS rows marked (c).

## Consequences

- **Section 8, screen readers.** Replace "GPUI's support is unconfirmed ... an exception on all platforms" with: on
  macOS the tree is verified through the OS accessibility API (named window, labelled button, text fields with
  contents when text runs are published; plain strings and styled text are not published); use with a screen reader
  is pending checklist step 13, and the exception narrows only once that step passes. On Linux (AT-SPI) and Windows
  (UI Automation) the same code is implemented but unverified, and stays an exception until a person runs Orca and
  Narrator. If the VoiceOver step fails, macOS stays in the exception with the failure noted.
- **Rules the app takes on for accessibility:** all visible text goes through `text!` (or `Text::new`) or an
  `aria_label`; every control has an id, a role and a label, and its visible label uses `Text::new_inaccessible` so it
  is not read twice; text fields publish `TextRun` children (the kit's input does); styled text, the markdown view
  included, publishes nodes of its own, because `StyledText` publishes nothing. These belong in the cookbook.
- **Section 8, other exceptions to add:** the Linux tray is menu-first: a click's position is only a host-supplied hint,
  so a popover cannot be placed reliably on X11 and not at all on Wayland (decision 0011); Linux file dialogs need
  xdg-desktop-portal; Linux and Windows have no native menu bar (the app draws one); macOS Quit from the Dock and
  log-out bypass the quit gate until the app adds `applicationShouldTerminate:` handling; Windows input methods go
  through IMM32 only.
- **Work the app owns:** link delivery from arguments and a single-instance hand-off on Linux and Windows; mapping a
  global point to a display on macOS; storing window state as origin plus content size; calling
  `invalidate_character_coordinates` from text fields; `on_open_urls` registered on the `Application` before `run`.
- **Patterns for the cookbook** (from `spikes/platform/src/`): `text_field.rs` (the input-handler contract, the
  candidate window, a tab-stop focus handle, role, label, value and text runs); `main.rs` `button` (an accessible
  button) and `popover_bounds` with `mac.rs` `locate_on_display` (a window at a point on the right display);
  `tray.rs` (tray events into GPUI through a channel, no polling; GTK on its own thread); `link_scheme.rs` (metadata
  generated from one place, links from arguments); `selfcheck.rs` and `mac.rs` (an app inspecting its own text input,
  menu and accessibility tree, a way to test those on macOS without driving the screen). Two traps to note:
  `KeyBinding::new` panics on `secondary-` (it is resolved only by `KeyBinding::load`), and a typed
  `WindowHandle::update` leases the root view, so dispatching a keystroke inside it panics (use the untyped handle).

### What remains unverified

- Everything marked (c): real input methods, real dead keys, press-and-hold, the tray, menus and dialogs as a person
  sees them, VoiceOver's actual speech, and (optional) a real `unidex://` link and the second-launch hand-off.
- Linux and Windows beyond compiling and the headless tests: no window was opened there, no input method, tray host,
  portal or screen reader was involved. The CI run itself is the pull request's.
- Windows input methods that work only through the Text Services Framework; Wayland compositors without
  text-input-v3; Narrator and Orca with AccessKit's adapters.
- `native_window_state()` restoration, system notifications from an unbundled app, and whether GPUI's timers keep
  running while the tray menu is open on macOS.
- For one trial run the bundle `target/UniDeX Platform Trial.app` existed for a few minutes (built by
  `make-bundle-macos.sh` to check the script) and was deleted unopened; whether LaunchServices indexed it in that
  time was not checked. The checklist's undo command covers it.

## Revisit when

A GPUI upgrade changes the platform layer (input handling, accessibility, menus, URL delivery or windows), GPUI adds
tray support or calls `on_open_urls` on Linux and Windows, the maintainer's checklist or a Tier-2 tester contradicts a
row above, or the app adopts the widget kit's input.
