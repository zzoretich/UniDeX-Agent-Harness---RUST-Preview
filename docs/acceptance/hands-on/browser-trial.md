# Browser trial: a look at the embedded browser (macOS)

> **Warning.** Every start of the embedded Chromium contacts its vendor's servers before any page is shown (its
> component updater, network time and a search endpoint), which tells the vendor this machine's public IP address, the
> Chromium version and the time of use, and the component updater downloads files into the trial's profile folder.
> The switches the trial passed to stop this did not stop it, and no working control exists yet (decision 0012,
> "Security against the threat model's browser boundary", P52). This check waits for the maintainer's explicit word.

Decision 0012's results on macOS come from scripted runs; no person has operated the embedded browser, and Tier 1 asks
for the maintainer's hands-on check. This page is that check. It has not been run by a person: it is derived from the
trial's scripts (`unpack-cef.sh`, `build-cef-macos.sh`, `cef-helper/bundle-macos.sh`, `measure-macos.sh`), the app's
source, the trial's workflow (`.github/workflows/spike-browser.yml`) and decision 0012, all at commit c6880ec. The
trial's code was removed from `main` at the M0 gate.

## What it needs

- A Mac with Apple silicon (the pinned CEF build is for macOS arm64), the Xcode command line tools, and the pinned Rust
  toolchain (rustup installs it from the checkout's `rust-toolchain.toml`).
- CMake and Ninja, which the CEF bindings use to build CEF's C++ wrapper (`command -v cmake ninja` shows whether they
  are installed; the trial installed them with the maintainer's approval).
- The CEF minimal distribution the trial pinned:
  `cef_binary_156.0.30+g6028375+chromium-156.0.8078.96_macosarm64_minimal.tar.bz2`, 134,874,408 bytes (about 135 MB),
  from CEF's build host, checked against the SHA-256 the trial's workflow pinned before anything is unpacked. The
  trial's `unpack-cef.sh` refuses an archive holding an absolute path or a `..` component but does not check the hash
  itself, so the hash check below comes first, as it did in the trial.
- About 4.3 GB of disk in the scratch checkout (the figure measured on the development machine; yours will differ):
  the archive and the unpacked distribution, the helper's app bundle (about 350 MB as built) and the Rust build of the
  app, GPUI included.
- Network access: for the download, and because Chromium contacts its vendor on every start (see the warning).

## Get and build the trial

```sh
# From the root of your checkout of this repository:
git worktree add --detach ../unidex-m0-trials c6880ec
cd ../unidex-m0-trials/spikes/browser

# The CEF distribution, checked before it is unpacked. `shasum` must print "OK"; if it does not, delete the file
# and stop.
file='cef_binary_156.0.30+g6028375+chromium-156.0.8078.96_macosarm64_minimal.tar.bz2'
mkdir -p target/cef-download
curl --proto '=https' --tlsv1.2 -sSfL -o "target/cef-download/$file" \
  "https://cef-builds.spotifycdn.com/$(printf '%s' "$file" | sed 's/+/%2B/g')"
echo "731b599e78c71bf33530d65bfa18963f7d1d59902c1f381f5452188e5116b855  target/cef-download/$file" \
  | shasum -a 256 -c -

# Unpacked into the layout the bindings expect, so their build script downloads nothing.
./unpack-cef.sh "target/cef-download/$file" target/cef 156.0.30 cef_macos_aarch64

# The CEF helper, the app and the stand-in, then the helper's app bundle in target/release.
./build-cef-macos.sh
```

`build-cef-macos.sh` points `CEF_DOWNLOAD_URL` at a host that does not exist, so if the bindings' build script ever
tried to download something the build would fail instead. It ends by printing the size of
`target/release/spike-cef-helper.app` and `bundled target/release/spike-cef-helper.app`.

## Start it

```sh
target/release/spike-app --helper cef --page form --seconds 900
```

The window has a dark toolbar ("Engine: ...", "Navigate", "Evaluate", a status text, and a line with frames painted
per second, paint time, latency and the app's CPU) above the browser pane. At the top right of the pane GPUI draws a
stand-in permission card ("Allow this page to use the camera?", amber border) above the page. `--seconds 900` ends the
app after 15 minutes whatever happens. Other pages: `--page long` (a long page to scroll), `--page anim` (an
animation), `--page heavy` (about 21,000 elements and 300 images). Each start uses a fresh browser profile in
`target/release/cef-profiles/`. "Navigate" loads a public web page (`https://example.com/`); it is not needed here.

## What to do and look at

| # | Do this | Expected result |
|---|---|---|
| 1 | On the form page, click into a text field and type `hello`; use Backspace, the arrow keys, Tab and Enter; then Command-A, Command-C, click another field, Command-V. Move the pointer over a field, the link and empty space. Click the drop-down list and choose an entry. | The text appears as typed and edits as expected; Tab moves to the next field; Command-A, C and V select, copy and paste inside the page (the trial's helper performs them, because Chromium leaves those shortcuts to the application's menu on macOS); the pointer is an I-beam over fields, a hand over the link and an arrow elsewhere; the list opens as a popup drawn by the app and the chosen entry shows. Note: Command-C puts the text on the system clipboard. |
| 2 | Switch to a Japanese or Chinese input source. In a text field type `kanji` (Pinyin: `hanzi`), press Space, then Return. Start another word and press Escape. Switch back. | While composing, the text is underlined in the page's field; Return commits 漢字 (or 汉字); Escape removes the uncommitted text. The candidate list should appear next to the field, not in a corner of the screen: the trial never saw the candidate window, so this is the first look at it. |
| 3 | Quit (close the window), start again with `--page long`, and scroll with the trackpad or mouse wheel, slowly and fast, down and back up. | The page scrolls in the direction of the gesture, smoothly; the toolbar shows about 60 frames painted per second on a 60 Hz display while scrolling (the trial measured 54.6 to 59.8 at scale 2). |
| 4 | Quit, start again with `--page anim`. Watch the card at the top right, then click it. | The card stays drawn above the moving page and the page keeps changing under it; a click on the card does not reach the page. |
| 5 | Drag the window's edge to make it smaller and larger. | The page lays itself out again for the new size within a moment; text stays sharp on a Retina display (the page sees a device pixel ratio of 2). |
| 6 | Close the window with its close button (the trial app has no Quit menu item). Within a few seconds run `pgrep -fl 'spike-app\|spike-cef-helper'`. | The window closes and `pgrep` prints nothing: in the trial, Chromium's sub-processes were gone within 0.4 s of the app's exit in every run. |

Note anything that differs, with the page and the step.

## Remove it

The downloaded archive, the unpacked CEF, the helper bundle and the browser profiles (including what the component
updater downloaded) are all inside the scratch checkout. Remove it from your own checkout:

```sh
cd <the root of your checkout>
git worktree remove ../unidex-m0-trials
git worktree prune
```

Add `--force` only if git refuses because of files you changed or added there; build output is ignored by git and
does not stop it. If CMake and Ninja were installed only for this check, removing them (`brew uninstall cmake ninja`)
is the maintainer's choice.
