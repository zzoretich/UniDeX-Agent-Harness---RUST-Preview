# udx-cookbook

Small, compiled examples of GPUI, the UI framework UniDeX draws with, on the exact snapshot this repository pins. Agents
and people write UI code from these examples and from the pinned source, not from memory of the framework. CI builds
every example on macOS, Linux and Windows, so an example that stops compiling fails the build. CI never runs them: each
opens a window.

Each example is one file in `examples/`, commented for someone learning the pattern: why it is done this way, which
decision record measured it, and what the trap is. The parts of a pattern that need no window live in the crate's
library (`src/`), use only the standard library, and are unit-tested on every platform by `cargo test`. The crate
publishes nothing and adds no dependency to the workspace: GPUI is its only dev-dependency.

Why this source and these pins: [decision 0007](../../docs/decisions/0007-gpui-source-and-pins.md).

The M0 trials' code was removed at the M0 gate; it remains in the repository's history at commit c6880ec, the last
commit on `main` that contains `spikes/`. The examples below were copied from it, and the list after them says what was
not carried and where it is.

## Examples

| Example | What it shows | Record | Run it by hand |
|---|---|---|---|
| `hello_window` | A window titled "UniDeX cookbook" with one label and one button that counts clicks | [0007](../../docs/decisions/0007-gpui-source-and-pins.md) | `cargo run -p udx-cookbook --example hello_window` |
| `streaming_list` | GPUI's `list` over 5,000 rows of different heights; a reply streamed into the last row from another thread through a channel, drained in batches by a foreground task and applied with one row re-measured; `FollowMode::Tail`; rows prepared once and kept; frame and token timing without the profiler, and why slow frames come from text new to the view | [0008](../../docs/decisions/0008-chat-and-terminal-trial.md) | `cargo run -p udx-cookbook --example streaming_list --release` |
| `text_input` | Two fields that implement `EntityInputHandler`: typed text, input-method composition with marked text, dead keys, the candidate window placed at the marked text (`invalidate_character_coordinates` after every edit), bound keys versus text, Tab between fields, and accessibility (role, label, value, text runs) | [0010](../../docs/decisions/0010-platform-integration-trial.md) | `cargo run -p udx-cookbook --example text_input` |
| `app_shell` | The native menu with shortcuts, key bindings and key contexts (and the `secondary-` binding that panics), several windows, window state saved as frame origin plus content size so a restored window does not grow, the untyped window handle, `unidex://` links from `on_open_urls` and from the arguments, accessible buttons, quitting when the last window closes | [0010](../../docs/decisions/0010-platform-integration-trial.md) | `cargo run -p udx-cookbook --example app_shell -- unidex://thread/42` |
| `frame_pane` | Frames produced off the UI thread drawn into an element with a GPUI card above them: slots handed back and forth by messages on a pipe, a line limit and a frame-size check before any pixel is copied, the newest frame in a single slot, `paint_image` and `drop_image`, the pane's hitbox in prepaint so the card occludes it. A stand-in thread produces the frames | [0012](../../docs/decisions/0012-embedded-browser-trial.md) | `cargo run -p udx-cookbook --example frame_pane --release` |
| `frame_check_scene` | The fixed scene the rendering trial drew so a captured frame can be checked: colour blocks, a state block, a text line and a button at known places. The check itself is `udx_cookbook::frame_check::check_frame` | [0013](../../docs/decisions/0013-rendering-in-ci-and-vms.md) | `cargo run -p udx-cookbook --example frame_check_scene` |

## The library

| Module | What it holds | Used by |
|---|---|---|
| `stats` | Nearest-rank percentiles, a frame recorder (paint intervals and draw work against the refresh interval) and token delays to the first painted frame | `streaming_list` |
| `channel` | A small unbounded channel a GPUI foreground task can await, standing in for Tokio's (the application uses Tokio, design 5.10) | `streaming_list`, `app_shell`, `frame_pane` |
| `text_model` | A field's text, selection and marked range, edited the way the input handler asks, with UTF-16 conversion and accessibility text runs | `text_input` |
| `window_state` | The saved window line (frame origin and content size), with limits so a damaged file falls back to the default | `app_shell` |
| `links` | `unidex://` links among the arguments | `app_shell` |
| `wire` | A line reader that refuses a line over a limit | `frame_pane` |
| `frames` | The frame-size check, the frame message, the newest-frame slot, and the cookbook's PPM stand-in for `RenderImage` | `frame_pane` |
| `frame_check` | The scene's layout and colours, and the frame check with its tests | `frame_check_scene` |

The examples run from 90 to about 340 lines with their comments, except `text_input` (about 415): the input handler is
nine methods and the field lays out and paints its own text, which is still half the size of the trial's field.

## Checking the examples by hand

Nothing here runs in CI. The expected results below are read from the code; a person has not yet checked them, except
that `frame_pane` was started once on the development Mac and ran for six seconds without an error.

- `hello_window`: one window; each click on "Count" adds one to the label.
- `streaming_list`: the list opens at its end and a reply grows in the last row for 30 seconds. Scrolling up during the
  stream stops the following; scrolling back to the bottom resumes it. When the stream ends the status line shows the
  token delays and the paint intervals, and the window does nothing until touched.
- `text_input`: typing goes into the focused field; with a Japanese or Chinese input source the composing text is
  underlined, the candidate list opens just below it (on the second line too, in Notes), Return commits and Escape
  cancels; Option-E then E gives é; Tab and Shift-Tab move between the fields; Enter starts a new line only in Notes.
  With VoiceOver, each field is read with its label and its contents.
- `app_shell`: on macOS the menu bar shows the application menu (Say Hello, Quit), "File" and "Window", with their
  shortcuts; Cmd-N opens a window; "Reopen from saved state" (Cmd-Shift-R) replaces the window with one at exactly the same place and size, and
  stderr shows the saved line; Cmd-Shift-H counts in every window; a link given as an argument is listed. Linux and
  Windows show no menu bar (GPUI draws none there) but the shortcuts work with Ctrl.
- `frame_pane`: a moving pattern fills the pane, and the frame follows the window's size up to 1280 by 800 device
  pixels; the card stays above it; the pointer is a crosshair over the pattern and an arrow over the card; a click on
  the card counts only as a card click.
- `frame_check_scene`: red, green, blue and grey blocks, the line "Render check 0123 ABC xyz", a blue "Count" button;
  the first click turns the grey block orange.

## Patterns not carried

These patterns, named in the decision records, did not become examples because each needs a crate the workspace does
not have (and the cookbook adds none) or is not a GPUI pattern. Paths are at commit c6880ec.

- Incremental markdown parsing of a streamed message (`StreamDoc`) and markdown turned into text runs:
  `spikes/chat-term/core/src/md.rs` and `spikes/chat-term/app/src/bin/chat.rs`. Needs `pulldown-cmark`. Decision 0008.
- The widget kit's markdown view as the message renderer (`--features kit` in `chat.rs`). Needs `gpui-base`, which is
  not adopted yet. Decisions 0007 and 0008.
- The terminal: the grid element `TermElement` (runs shaped with `force_width`, wide glyphs placed alone, merged
  backgrounds, selection from the emulator) and PTY wake-ups coalesced with an atomic flag
  (`spikes/chat-term/app/src/bin/term.rs`); the session over `alacritty_terminal` that writes the emulator's replies
  back to the PTY (`spikes/chat-term/core/src/pty.rs`); the key and paste encoding table
  (`spikes/chat-term/core/src/keys.rs`). Needs `alacritty_terminal`. Decisions 0008 and 0009.
- Process CPU time, context switches and memory read inside the app (`spikes/chat-term/core/src/procstat.rs`). Needs
  `libc`. Decision 0008.
- The tray: its events forwarded into GPUI through a channel with no polling, GTK on its own thread
  (`spikes/platform/src/tray.rs`), and the popover placed under the icon on the right display (`popover_bounds` in
  `spikes/platform/src/main.rs`, `locate_on_display` in `spikes/platform/src/mac.rs`). Needs `tray-icon`, and
  `objc2` for AppKit's display arrangement. Decisions 0010 and 0011.
- An app inspecting its own text input, menu and accessibility tree on macOS without driving the screen
  (`spikes/platform/src/selfcheck.rs`, `spikes/platform/src/mac.rs`). Needs `objc2`. Decision 0010.
- The `unidex://` metadata for each OS generated from one place (`spikes/platform/src/link_scheme.rs`,
  `spikes/platform/bundle/`). Not a GPUI pattern: it belongs with packaging, and the application's identity awaits the
  maintainer (decision 0014). Reading links from the arguments is carried, in `links` and `app_shell`. Decision 0010.
- The browser itself: the `BrowserView` trait (`spikes/browser/protocol/src/view.rs`); JSON messages
  (`protocol/src/wire.rs`; its line limit is carried); the unnamed shared-memory region the helper inherits
  (`protocol/src/shm.rs`, needs `libc` and `windows-sys`); the CEF helper with its re-launch guard, its own
  `NSApplication`, the input-method range rule and the edit shortcuts (`spikes/browser/cef-helper/src/`), the Windows
  bootstrap DLL (`spikes/browser/cef-helper-win/`), the macOS helper bundles (`cef-helper/bundle-macos.sh`), the pinned,
  hash-checked CEF unpacking (`spikes/browser/unpack-cef.sh`) and the setuid `chrome-sandbox` `.deb` step
  (`.github/workflows/spike-browser.yml`); input methods tested through GPUI's macOS view and pages read back with
  DevTools (`spikes/browser/app/src/mac_ime.rs`, `script.rs`); the sandbox verified per process
  (`protocol/src/tree.rs`). Needs CEF and its build, `objc2`, `libc` and `windows-sys`. Decision 0012.
- The in-app frame capture (`spikes/render-ci/src/bin/render-ci/capture.rs`). Needs GPUI's `test-support` feature,
  which release builds must not have, and `x11rb`. Decision 0013.
- The automation socket and its driver (`spikes/render-ci/src/bin/render-ci/automation.rs`,
  `spikes/render-ci/src/bin/render-ci-client.rs`). Its safety properties cannot be kept with the standard library
  alone: the peer check needs `getpeereid` or `SO_PEERCRED` (`libc`; the standard library's peer credentials are not
  stable) and the Windows named pipe's owner-only access list needs `windows-sys`; frames need `test-support`. A copy
  without the peer check would drop a property decision 0013 requires, so none is carried. Its line limit is in
  `wire`. Decision 0013.
- The rendering CI scripts (`spikes/render-ci/ci/scenario.sh`, `automation.sh`, `wayland.sh`, `capture-window.ps1`
  and the diagnostics) and the release-build marker check (`.github/workflows/spike-render-ci.yml`): CI for a harness
  that does not exist yet. Decision 0013.
- The packaging comparison: the trial workflow (`.github/workflows/spike-packaging.yml`), the cargo-deb probe with a
  setuid file and a maintainer script (`spikes/packaging/probe/`, `packager-setuid.toml`), the AppImage configuration
  and icon script (`packager-appimage.toml`, `make-icons.sh`) and `diag.sh`. The chosen tool's configuration is
  `packaging/` and `.github/workflows/nightly.yml`. Decision 0014.

The hands-on procedures for the platform and browser trials are kept in
[`docs/acceptance/hands-on/`](../../docs/acceptance/hands-on/README.md).

## The dependency lines

The pins live once, in the workspace `Cargo.toml`:

```toml
[workspace.dependencies]
gpui = { package = "gpui-pre", version = "=0.3.12" }
gpui_platform = { package = "gpui-pre-platform", version = "=0.3.12", features = ["font-kit", "x11", "wayland", "runtime_shaders"] }
```

A crate that draws UI copies these two lines into its own `Cargo.toml`, under `[dependencies]`:

```toml
gpui.workspace = true
gpui_platform.workspace = true
```

Code then uses `gpui::...` and starts the app with `gpui_platform::application()`. Never write a version in a member
crate, and never add another GPUI source (a git dependency, the `gpui` crate on crates.io, or a fork); a second copy of
GPUI does not work with the first. The widget kit (`gpui-kit`) is not adopted yet; decision 0007 says why.

The `gpui_platform` features: `font-kit` draws text on macOS, `x11` and `wayland` are the Linux window backends, and
`runtime_shaders` compiles the Metal shaders when the app starts instead of at build time, so building needs no Metal
compiler.

## Run an example

```sh
cargo run -p udx-cookbook --example hello_window
```

Any example runs the same way with its name; the table above gives each command. Close the window to end it.

Building on Linux needs a few system packages (the list is in decision 0007 and in `.github/workflows/ci.yml`). Before a
full build, `cargo check -p udx-cookbook --examples` is the cheaper way to see whether an example compiles.

## Share compiled dependencies between worktrees (sccache)

Every worktree has its own `target` directory, so without a shared cache each one compiles GPUI from scratch. `sccache`
keeps compiled crates in one cache that every worktree reads. It is optional and is not configured in the repository, so
machines and CI runners without it keep working. To turn it on for one shell session:

```sh
export RUSTC_WRAPPER=sccache
```

Then build as usual. `sccache --show-stats` shows the hit rate. Turn it off with `unset RUSTC_WRAPPER`. The cache lives in
sccache's default folder, limited to 10 GB unless `SCCACHE_CACHE_SIZE` says otherwise. Measured build times are in
decision 0007.

## Upgrade GPUI

An upgrade is its own change, on its own branch, never mixed with feature work.

1. Pick the new snapshot: the `gpui-pre` version that the newest `gpui-kit` release requires (read it from that release's
   dependencies on crates.io), unless decision 0007 has been superseded. Note the upstream commit it was cut from, which
   is in the crate's description and in `[package.metadata.gpui-pre]` of its `Cargo.toml`.
2. Change the version in both lines of `[workspace.dependencies]` in the root `Cargo.toml`, keeping the exact `=x.y.z`
   form, then run `cargo update -p gpui-pre -p gpui-pre-platform` so `Cargo.lock` follows (every other `gpui-pre-*`
   crate moves with them). Read the lockfile diff.
3. Check the snapshot against upstream: check out upstream at the recorded commit and `diff -r` each whole upstream
   crate folder (sources, `build.rs`, `vendor/`) against its `gpui-pre-*` copy in the cargo registry, and read every
   "Modified for gpui-pre" notice. Then compare each manifest's dependency tables (names, versions, features, and any
   swap from a git source to a registry release) with upstream's manifest and workspace `Cargo.toml`, including
   `[patch.crates-io]`. Any change that has no notice, or that decision 0007 does not document, stops the upgrade until it
   is understood and recorded.
4. Build and run the cookbook (`cargo build --workspace --examples --locked`, then
   `cargo run -p udx-cookbook --example hello_window`) and fix every example and UI crate the new snapshot breaks. Then
   run the full local gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
   `cargo test --workspace --locked`, `cargo deny --locked check`, `cargo audit`. List every advisory the new tree
   carries in the new decision record; ignoring one needs the maintainer's agreement.
5. Write a new decision record that supersedes 0007 with the new version, its upstream commit and date, and anything
   that changed (packages, licences, advisories, build times), and update this README.
6. Open the pull request with no feature work in it, and let CI build the cookbook on all three platforms before it
   merges.

## Provenance

GPUI and the `gpui-pre-*` snapshot crates are Apache-2.0. Much of the editor GPUI comes from is not permissively
licensed, including its terminal view. Read permissively licensed code (GPUI itself, its examples and docs, GPUI Kit) as
reference, and write the examples here in your own words. Never copy code from any crate or file that is not
permissively licensed. Reviewers check where code came from, and `cargo deny` checks every dependency's licence, including GPUI's, which the cookbook takes as a dev-dependency
(`include-dev = true` in `deny.toml`).

The examples and the library copied from the M0 trials are the project's own trial code, written for the trials
against the crates' public APIs. The trials' records list what was read as reference; `text_model` and `text_input`
keep the platform trial's note that GPUI's `examples/input.rs` (Apache-2.0) was read as reference, and `text_input`
keeps its note that the widget kit's `a11y_text.rs` does the same for screen readers, more fully.
