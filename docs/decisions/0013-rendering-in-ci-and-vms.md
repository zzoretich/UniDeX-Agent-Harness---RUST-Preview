# 0013: Rendering in CI and in virtual machines

Status: accepted with open questions
Date: 2026-11-17

## Context

Design section 8 supports Linux (x64) and Windows (x64) as Tier 2, "verified by CI only", and asks that "CI-verified"
be made as strong as it can be without hardware: the build, unit tests, view-model tests and end-to-end tests that
drive the real app through its automation socket (section 10.3). M0 also checks whether virtual machines on the
development machine can render the app; if they can, they become a hands-on step for Tier 2. Until this trial,
nothing visual had been verified on Linux or Windows: decision [0008](0008-chat-and-terminal-trial.md) records builds
and headless checks only.

The questions: can a GPUI window open and render on GitHub's hosted Linux and Windows runners, which have no GPU; can
a frame be captured as evidence and checked automatically; can a small automation socket send an action, read the
element tree back and capture a frame, without becoming a way into a user's running app (the threat model: a
test-only feature in a release build is threat 4 of the supply-chain section, with controls P37 and L13; endpoints
are owner-only and peer-checked, L28 and P39; entry points take named verbs and validated arguments, L5); what
"CI-verified" can honestly claim per platform; and whether virtual machines on the development machine (an Apple
silicon laptop with 16 GB) should become a hands-on step. GPUI is the pinned `gpui-pre =0.3.12` snapshot (decision
[0007](0007-gpui-source-and-pins.md)).

Step 3 of the task (running virtual machines on the development machine) was not run: the maintainer has not approved
installing virtual machine software. It was researched instead (below).

## What was built

Throwaway code in `spikes/render-ci/` (a standalone Cargo workspace, not a member of the repository's workspace) and
`.github/workflows/spike-render-ci.yml`.

- The app (`src/bin/render-ci/main.rs`) draws fixed content at fixed logical positions: red, green and blue 60 px
  blocks, a state block (grey until a button is clicked, orange after), the line "Render check 0123 ABC xyz" at 20 px,
  a "Count" button and a "Clicks: N" label. `--scenario <dir>` waits for the window to draw, reads its own frame back,
  checks it, writes `frame-inapp.png` and `scenario.json` (GPUI's GPU description, scale factor, bounds, render count,
  time to first render, the check), then holds the window open so CI can capture the display too. `--exit-after`
  always ends the process.
- In-app frame capture (`capture.rs`): on macOS and Windows, GPUI's `Window::render_to_image`; on Linux X11, the app
  asks the X server for its own window's pixels (core-protocol GetImage). Details below.
- The frame check (`src/lib.rs`, `src/bin/frame-check.rs`): described under "The frame check".
- The automation socket (`automation.rs`, feature `automation`) and its driver (`src/bin/render-ci-client.rs`):
  described under "The automation socket".
- CI scripts (`ci/`): `scenario.sh` (run, capture the display, check both frames), `automation.sh` (the round trip,
  optionally inside a D-Bus session with AT-SPI on), `wayland.sh` (headless sway), `linux-diagnostics.sh`,
  `windows-diagnostics.ps1`, `capture-window.ps1`.

## Results

Evidence: workflow run 44128026417 on commit `d03c3e1` (round 1), and workflow run 44132309583 on commit `e192261`
(round 2: the round 1 code plus the fixes below), run four times (attempts 1 to 4; jobs listed under "Stability and
cost"). Artifacts `render-ci-linux` and `render-ci-windows` hold the frames, JSON and logs. Every scenario logged its
own renderer, adapter and check results, and the figures here come from those logs, read for every attempt, not from
the step results. Round 1's Linux job failed only on the release-build check, which was a fault in the check (below);
all four round 2 attempts were green on both jobs.

| Platform | Display | Renderer and adapter (as GPUI logged them) | In-app frame | Display capture | Socket round trip |
|---|---|---|---|---|---|
| Linux, X11 + Vulkan | Xvfb 21.1.22, 1280 x 800 x 24, no window manager, no compositor | wgpu Vulkan on lavapipe: "llvmpipe (LLVM 21.1.8, 256 bits)", Mesa 26.0.8, device type CPU | X11 GetImage: all checks pass | `xwd` of the root window: all checks pass | passes; AccessKit tree with AT-SPI on |
| Linux, X11 + OpenGL | same | wgpu GL through EGL on llvmpipe, OpenGL 4.5 core (Vulkan drivers hidden with `VK_DRIVER_FILES`) | X11 GetImage: pass | `xwd`: pass | not run here (the socket does not depend on the renderer; the X11 + Vulkan row covers it) |
| Linux, Wayland | sway 1.11, headless back end, pixman renderer | wgpu Vulkan on lavapipe | not possible (see below) | `grim`: all checks pass | action and tree pass, AccessKit tree with AT-SPI on; frame not possible |
| Windows | the runner's interactive desktop: console session 2, 1024 x 768, display adapter "Microsoft Hyper-V Video" | Direct3D 11 at feature level 11.1 on "Microsoft Basic Render Driver" (WARP, Microsoft's software rasteriser), with and without DirectComposition | `render_to_image`: pass | screen pixels under the window's client rectangle (.NET `CopyFromScreen`): pass | passes; AccessKit tree |

What each row shows, and how it could have been weaker than it looks:

- **The window really rendered.** On every row except the in-app Wayland column, two independent frames passed the
  check: the app's own readback and a capture taken from outside by a standard tool (Linux) or by .NET (Windows). The
  display captures are real presented pixels: Xvfb's screen, sway's output, the Windows desktop. On Windows the capture
  was cut to the window's client rectangle (origin found at 0,0 in the capture), so it shows that DWM composed the
  window, not only that the renderer drew offscreen.
- **Linux.** GPUI's wgpu context enumerated two adapters (lavapipe on Vulkan, llvmpipe on GL), sorted them (CPU devices
  last, Vulkan before GL) and took the first that could configure the window's surface. Software adapters are accepted
  at start; only device-loss recovery rejects them. Mesa printed "DRI3 error: Could not get DRI3 device" (Xvfb has no
  DRI3) and fell back to software presentation. GPUI also logged "Found no xinput mouse pointers" (Xvfb has a virtual
  core pointer but no XInput pointer device). Time to first render on X11: 232 to 466 ms over the round 2 attempts
  (Vulkan and OpenGL alike); 2.17 s once in round 1, when the text system took 1.94 s to parse 53 font faces from a
  cold disk cache (1 to 206 ms in every later launch).
- **Fonts.** On Linux GPUI maps its default UI font (".SystemUIFont") to "IBM Plex Sans", which the runner does not
  have; GPUI warned and fell back to an installed font (DejaVu was installed for this reason; which face it chose is
  not logged). The text check passed with the fallback. The app must name fonts it ships on Linux.
- **Windows.** The session is interactive (`UserInteractive` true; the process's session is the active console
  session; not a remote session). DXGI enumerated WARP as an adapter and GPUI took it; GPUI's own description says
  `is_software_emulated: false` for it, so that flag cannot be trusted to detect software rendering on Windows. Time to
  first render 77 to 124 ms. DirectComposition on and off made no difference here.
- **Wayland.** The window rendered (render count 5, lavapipe; time to first render 1.2 to 2.2 s) and `grim` captured
  it, but the app cannot read its own window back: GPUI's Wayland window does not implement `render_to_image`, and
  Wayland gives a client no way to read the screen. The automation run on Wayland (round 2) clicked the button, saw the
  state change and read the AccessKit tree; its two frame steps are reported as warnings, by design. sway also logged
  that it could not start Xwayland (not installed; irrelevant here).
- **The AccessKit tree did not come from a screen reader.** It came from activating accessibility per platform (below).
  On Linux the AT-SPI bus was told `IsEnabled = true` before the app started; on Windows the app sent itself a UI
  Automation root query; on macOS it asked its own content view for its accessibility children.

### Stability and cost

Five runs in all: round 1 (run 44128026417) and four attempts of run 44132309583 (jobs: attempt 1 Linux 134212393494,
Windows 134212394071; attempt 2 Linux 134221676932, Windows 134221676569; attempt 3 Linux 134229304590, Windows
134229303883; attempt 4 Linux 134238190583, Windows 134238191369). In every one:

- the same adapters were chosen (lavapipe on Vulkan, llvmpipe on GL when Vulkan was hidden, lavapipe on Wayland, WARP
  on Windows);
- every in-app frame and every display capture passed every check, with the five sampled colours matching the
  expected values exactly (not only within tolerance), the same text measurements each time (Linux: 9.6 percent ink
  over 278 logical pixels; Windows: 6.7 percent over 240; the fonts differ, each platform is deterministic) and the
  same window positions in the display captures;
- every automation round trip passed (three per attempt: X11, Wayland without frames, Windows), with a 7-node AccessKit
  tree before and after the click;
- the release build carried no marker (round 2 onwards) and refused `--automation` with exit code 2.

What varied: time to first render (above) and job duration. The only warnings were the three expected Wayland ones
(no in-app readback; two frame steps). No step was retried and nothing flaked; five runs cannot rule out a rare flake.

Job durations, cold (no cache), standard hosted runners: Linux 11 min 8 s to 14 min 7 s over the four round 2 attempts
(build 300 to 390 s; packages 13 to 34 s; three scenarios and two automation runs 32 to 33 s; the second
build for the release check 282 to 369 s); Windows 7 min 4 s to 7 min 50 s (build 362 to 382 s; diagnostics with
`dxdiag` 11 to 28 s; two scenarios and the automation run 20 to 21 s). The trial's own work is about half a minute per
platform; the rest is building GPUI, twice on Linux. Standard hosted runners are free for public repositories
(GitHub's Actions billing documentation), so on every pull request this costs about 20 minutes of runner time and
nothing in money; at the documented private-repository rates (Linux $0.006 and Windows $0.010 per minute, for 2-core
runners on which builds would be slower) one run would cost about $0.15. A real harness that reuses the build of the
main CI job and its cache would add roughly the half minute of scenarios plus the Linux packages, and would keep the
release-build check for nightly artifacts.

## The frame check

`check_frame` finds the red block in any capture (the bounding box of pixels within 40 per channel of its colour),
derives the origin and scale from it, then samples a patch at the centre of the green, blue and state blocks, the
background at an empty spot and the inside of the button, and counts dark pixels in the text line's region (between 2
and 60 percent ink, at least 120 logical pixels wide). It also requires at least six distinct colours. Six unit tests
cover correct frames at scales 1, 1.5 and 2 with offsets, and failures for a missing text line, a blank frame, swapped
red and blue channels and the wrong state colour.

It catches: a blank, uniform or black frame; the wrong window; missing fills; swapped channels; a wrong scale; text
that did not draw; a state change that did not reach the screen. It cannot catch: which font was used or whether
glyphs are correct and legible (fonts differ per platform, and the runner used a fallback); antialiasing and subpixel
quality (software rasterisers differ from GPUs at edges, which the patch sampling avoids on purpose); layout errors
outside the sampled points; colour management. Pixel-exact baselines across platforms would be brittle for the same
reasons; design 8's note that screenshot baselines run on macOS first stands.

## The automation socket

Shape: line-delimited JSON over a local endpoint, one request per line (at most 64 KiB), one response per line; verbs
`hello`, `state`, `tree`, `a11y_activate`, `click {id}`, `frame`, `quit`; anything else is refused (the client checks
that a `shell` verb is refused). No verb takes a path or runs anything; a frame comes back inline as base64 PNG.

Safety against the threat model:

- Test builds only. The socket is behind the cargo feature `automation`; CI builds the release binary without it,
  checks that the marker string the socket code carries is absent (present in the test build) and that `--automation`
  is refused with exit code 2. This is a sketch of P37's release check. In round 1 the check itself was faulty: GitHub
  runs bash with `-e`, so `grep -c` returning 1 for a count of zero ended the step before the comparison; locally the
  release binary had 0 occurrences and the test binary 1, and the step now turns `-e` off and checks every result. In
  all four round 2 attempts it reported "test build 1, release build 0" and exit code 2 for `--automation`.
- Off unless the app is started with `--automation`.
- Never a TCP port. Unix: a fresh directory created with mode 0700 (creation fails if the name exists, so a planted
  directory is never reused; owner and mode are verified after creation), the socket created under umask 0177 and
  set to 0600, and each connection's peer user id compared with the app's (SO_PEERCRED on Linux, getpeereid on macOS).
  Windows: a named pipe with a protected DACL that grants access to the current user's SID only (not administrators,
  not Everyone), FILE_FLAG_FIRST_PIPE_INSTANCE (creation fails if another process owns the name) and
  PIPE_REJECT_REMOTE_CLIENTS; the client's process id is logged. One client at a time.
- Not yet done, and needed in the real harness: a stronger peer check on Windows (compare the client process's token
  user, not only the DACL), an idle timeout, a per-connection rate limit, and the endpoint's name passed to the test
  driver through an inherited handle or file rather than stdout.

What the framework lets the app see:

- **GPUI's own tree** is the AccessKit tree it builds during prepaint, read with `Window::debug_a11y_tree_json`. It has
  roles, labels, values, supported actions (the button lists `Click`) and parent-child structure, for elements that
  have both an id and a role. Missing: bounds (GPUI sets them on the AccessKit nodes, but the debug JSON leaves them
  out), element ids (included only in debug builds, as provenance) and anything without a role (the colour blocks).
  GPUI builds the tree only while accessibility is active, that is after a platform adapter asked for it. The app can
  cause that itself: macOS, by calling `accessibilityChildren` on the window's content view, which AccessKit subclasses
  (GPUI's own view is a subview, and asking it does nothing); Windows, by sending its own window `WM_GETOBJECT` with
  `UiaRootObjectId` from another thread; Linux, only from outside, by setting `org.a11y.Status.IsEnabled` on the
  session's AT-SPI bus. All three worked.
- **The app's registry** is its own bookkeeping: a `canvas` inside each tagged element records the bounds GPUI laid it
  out at, with an id, role and label the app chose. It is what the click uses to find an element, and it is the only
  source of bounds and ids in a release-mode test build.
- **Actions** go through `Window::dispatch_event` (mouse move, down and up at the element's centre), so GPUI's hit
  testing and the real click handler run; this is also exactly what GPUI's AccessKit `Click` fallback does. It does
  not go through the operating system's input path (no X11 or Wayland input events, no Win32 messages).
- **Frames** are `render_to_image` on macOS and Windows (GPUI's `test-support` feature: the current scene rendered
  again into an offscreen texture of the same renderer, so it is what the renderer draws, not what the compositor
  shows) and X11 GetImage on Linux (what was presented). There is no in-app frame on Wayland.

## What "CI-verified" covers

Wording design section 8 can adopt:

| | Linux (x64) | Windows (x64) |
|---|---|---|
| Covered | The app builds and its tests pass; its window opens and renders on X11 (a virtual display) through both of GPUI's Linux paths, Vulkan and OpenGL, on Mesa's software rasterisers, and on Wayland under a headless wlroots compositor; a frame is captured and checked automatically for the expected colours, layout and the presence of text; the automation socket clicks a control, reads GPUI's accessibility tree and the app's element list, and captures a frame (on X11; on Wayland action and tree only) | The app builds and its tests pass; its window opens and renders on a real Windows desktop session with Direct3D 11 on Microsoft's software rasteriser (WARP), with and without DirectComposition; the composed window on screen is captured and checked; the automation socket over a user-only named pipe clicks a control, reads GPUI's accessibility tree and captures a frame |
| Not covered | Real GPU drivers (Intel, AMD, NVIDIA; DRI3, hardware presentation); real desktop compositors and window managers (GNOME, KDE; decorations, focus, tiling); fractional and per-monitor scaling (the runner is at scale 1); input methods and dead keys; input through the display server (actions are dispatched inside GPUI); the system UI font and real font configuration; the tray, notifications and portals; file dialogs; the screen reader itself (Orca); performance and frame pacing; ARM Linux | Real GPU drivers and hardware Direct3D; scaling other than 100 percent and multiple monitors; input methods and dead keys; input through Win32 messages; the tray and notifications; file dialogs; the screen reader itself (Narrator, NVDA); performance and frame pacing; Windows on ARM |

A build that passes is still labelled "CI-verified, not yet run on real hardware", as section 8 says.

## Packages installed in CI

Linux, on top of `ci.yml`'s build packages (`pkg-config`, `libfontconfig-dev`, `libxcb1-dev`,
`libxkbcommon-x11-dev`): `xvfb` (the virtual display); `x11-apps` and `netpbm` (`xwd`, `xwdtopnm`, `pnmtopng` for the
display capture); `x11-utils` (`xdpyinfo`, diagnostics); `mesa-vulkan-drivers` (lavapipe); `libvulkan1` (the Vulkan
loader); `vulkan-tools` (`vulkaninfo`, diagnostics); `libgl1-mesa-dri`, `libegl1`, `libegl-mesa0` (llvmpipe through
EGL); `mesa-utils` (`glxinfo`, `eglinfo`, diagnostics); `fonts-dejavu-core` (a known font); `dbus`, `at-spi2-core`
(the session bus and AT-SPI bus); `sway`, `grim` (the headless Wayland compositor and its capture). Versions are the
runner image's, unpinned; every run (round 1 and the four round 2 attempts) got Xvfb 21.1.22, Mesa 26.0.8 with LLVM 21.1.8, sway 1.11, grim 1.4.0, at-spi2-core
2.60.4. Windows: nothing installed; PowerShell, .NET and `dxdiag` are on the image.

The only action added is `actions/upload-artifact`, pinned to `cf430e030ddbb5b0abf93d22962f4752f3646cd9` (v7.0.2).
Verified with `gh api repos/actions/upload-artifact/git/ref/tags/v7.0.2`: the ref's object is that commit directly (a
lightweight tag, so there is no tag object to dereference), the `v7` tag points at the same commit, the commit's
signature is verified, and the v7.0.2 release was published on 2026-10-07. Artifacts hold only the trial's window and
logs: Linux captures are of virtual displays that hold nothing else, the Windows capture is cut to the window's client
rectangle, and the pipe's SID is not logged.

## Virtual machines (researched, not run)

What a virtual machine on the development machine would add, and what it would take. Only Arm guests run at useful
speed on Apple silicon, so a guest tests the aarch64 builds of the app, not the x64 builds Tier 2 names. Windows 11 on
Arm can run an x64 build under its Prism emulator (user-mode code only; Microsoft recommends native Arm64 builds), and
Linux has no comparable built-in path.

| Option | Licence and cost | Download | Linux guest graphics | Windows 11 on Arm guest |
|---|---|---|---|---|
| UTM 5.0.8 (QEMU and Apple's Hypervisor framework) | Apache-2.0, free from GitHub (a paid Mac App Store copy only adds automatic updates) | 304 MB `.dmg` | OpenGL through VirGL; Vulkan 1.3 through VirtIO Venus, new and experimental (its release notes say Linux desktop rendering in Vulkan does not work yet) | Experimental DirectX driver, first in a stable release with 5.0.8 (October 2026), or display only (then WARP, as on CI); not on Microsoft's list of authorized ways to run Windows on Apple silicon |
| Parallels Desktop | Proprietary; Standard about $99.99 a year or $219.99 one time, Pro $119.99 a year (launch coverage; the vendor's page did not show prices to the fetch); free trial | not checked | not researched | Authorized by Microsoft (Windows 11 Pro and Enterprise on Arm, Parallels 18 to 20); 3D at "DirectX 11.1 and OpenGL 4.3", no DirectX 12, no nested virtualization |
| VMware Fusion | Proprietary, free for all uses since 2024-11-11 (download needs a Broadcom account) | not checked | OpenGL 4.3 for arm64 Linux guests (kernel 5.19 or later, per secondary sources quoting the release notes) | Supported by Fusion 13; 3D for Arm Windows guests not confirmed; not on Microsoft's authorized list |

Windows needs a licence in any of them (Windows 11 Pro or Enterprise; Microsoft names only Parallels and Windows 365 as
authorized on Apple silicon). Disk: the app plus an Arm installer image of a few GB per guest and a virtual disk of
roughly 25 GB for a Linux desktop and at least 64 GB for Windows 11 (Microsoft's published minimum, with 4 GB of
memory); a guest wants 4 to 8 GB of memory, so on a 16 GB machine one guest at a time, not alongside heavy builds.
These sizes are published requirements and estimates, not measured here.

How GPUI would render in each: on Linux, GPUI's adapter sort puts virtual and "other" devices before CPU devices, so
under UTM or Fusion it would likely pick the accelerated OpenGL device (VirGL or the VMware driver) or the Venus Vulkan device; under a guest without
3D it falls back to lavapipe, exactly as on CI. On Windows, GPUI's Direct3D 11 renderer would run on Parallels' virtual
GPU (DirectX 11.1, translated to Metal) or UTM's experimental driver; otherwise on WARP, as on CI. None of this was
tried.

How much of the gap a virtual machine closes: it is the only way, short of hardware, to put hands on the things CI
cannot see: a real desktop (GNOME or KDE on Wayland, Windows' shell), input methods and dead keys, the tray and
notifications, file dialogs and portals, fractional scaling, real fonts, and the screen readers (Orca, Narrator,
NVDA). It does not close the hardware gap: the GPU is virtual (translated to Metal), not an Intel, AMD or NVIDIA
driver; the CPU architecture is Arm, not the x64 Tier 2 targets; performance says nothing about real machines.

**Recommendation: approve UTM for a Linux guest now; defer Windows.** UTM is free and Apache-2.0, needs about 300 MB
plus a guest disk, and an Ubuntu or Fedora arm64 desktop guest covers the Linux desktop items above with an accelerated
OpenGL device, which GPUI's wgpu path can use. Use it at milestone gates for the hands-on Linux items of section 8, as
a check on the aarch64 build, labelled as such. For Windows, decide at M4, when the tray, dialogs and input screens
exist: Parallels is the only route Microsoft authorizes and has a stable DirectX 11.1 driver but costs a yearly
licence plus Windows 11 Pro; UTM's DirectX driver is a month old and experimental. CI already shows the Windows window
rendering on a real desktop session, so a Windows guest adds interaction checks, not rendering evidence. What the
maintainer would approve: installing UTM 5.0.8 (or newer), downloading an arm64 Linux desktop image, about 30 GB of
disk, and running one guest at a time during gate checks.

## Decision

CI is the rendering bench for Tier 2. GPUI windows render on the hosted Linux runner (X11 on Xvfb with Mesa's software
Vulkan and OpenGL, and Wayland under headless sway) and on the hosted Windows runner (a real desktop session, Direct3D
11 on WARP), frames can be captured and checked, and an automation socket of the shape above drives the app on both.
"CI-verified" in section 8 should mean what the table above lists, and no more. The real test harness keeps the
patterns below. Virtual machines are recommended as an optional hands-on step for Linux (UTM), pending the
maintainer's approval; a Windows guest is deferred to M4.

## Who decided

A design call from the M0 trial (Task 13), within design sections 8 and 10.3. Installing virtual machine software, and
whether virtual machines become a gate step, are the maintainer's decisions.

## Consequences

- Section 8 can state per-platform coverage in the words of the table above, and section 10.3's end-to-end tests are
  feasible on all three CI platforms as designed.
- The automation socket needs `render_to_image`, which is behind GPUI's `test-support` feature. Test builds therefore
  compile GPUI with `test-support` (it adds crates such as proptest and criterion to their graph, all within the licence
  policy), and release builds must not. That doubles GPUI's compile in any job that checks both; the check belongs on
  nightly artifacts (P37), not on every pull request.
- Linux in-app capture uses X11 GetImage; on Wayland the harness must capture from outside (grim) or skip frames. If
  GPUI's Linux windows gain `render_to_image`, the harness should switch to it.
- GPUI's AccessKit tree is usable as the harness's element tree for roles, labels, values and actions, but bounds and
  ids must come from the app (or from a GPUI change that adds bounds to its debug JSON).
- `GpuSpecs::is_software_emulated` is false for WARP on Windows; software rendering must be detected by adapter name
  or vendor id there.
- Patterns worth keeping for the real harness: the scene-plus-check approach and `check_frame`
  (`spikes/render-ci/src/lib.rs`); the in-app capture split (`spikes/render-ci/src/bin/render-ci/capture.rs`); the
  socket's endpoint code, peer checks, verb list and line limits, and the per-platform accessibility activation
  (`spikes/render-ci/src/bin/render-ci/automation.rs`); the driver that starts the app and reads the endpoint
  (`spikes/render-ci/src/bin/render-ci-client.rs`); the CI orchestration (`spikes/render-ci/ci/scenario.sh`,
  `automation.sh`, `wayland.sh`, `capture-window.ps1`); and the release-build marker check in
  `.github/workflows/spike-render-ci.yml`.

## Sources and provenance

Read as reference, all permissively licensed: the pinned GPUI crates in the cargo registry (`gpui-pre`,
`gpui-pre-platform`, `gpui-pre-wgpu`, `gpui-pre-linux`, `gpui-pre-windows`, `gpui-pre-macos`, `gpui-pre-apple`, all
0.3.12, Apache-2.0: renderers, adapter selection, `render_to_image`, the accessibility module and its debug JSON,
backend selection, the headless window), `accesskit_macos` 0.26.3 and `accesskit_unix` 0.22.1 (activation),
`windows-sys` 0.61.2 and `x11rb` 0.13.2 (signatures). No code was copied; the trial's code was written for it. No
crate of the editor that is not permissively licensed was read.

External facts, checked on 2026-11-17: Microsoft's "Options for using Windows 11 with Mac computers with Apple M1, M2
and M3 chips" (authorized products and editions); Microsoft Learn's WARP guide (WARP and the Basic Render Adapter,
feature levels) and "How emulation works on Arm" (Prism, x64 emulation, user-mode only); Parallels KB 129497
(limitations, DirectX 11.1 and OpenGL 4.3); UTM's display settings documentation and its GitHub releases (licence,
5.0.8 and 5.0.9 notes, `.dmg` size); the VMware blog post of 2024-11-11 (Fusion free for all users); GitHub's Actions
billing documentation (free on public repositories, per-minute rates); `gh api` for the upload-artifact tag. Parallels
prices come from launch coverage, and VMware Fusion's Linux 3D support from secondary sources quoting its release notes;
both are marked as such above.

## What remains unverified

- The flake rate beyond five runs, and behaviour on other runner image versions (Mesa, Xvfb and Windows images change).
- Which fallback font GPUI used on the Linux runner.
- Whether the automation socket's Windows peer handling holds against a second local user (only the DACL was relied
  on; not tested with another account).
- Input through the operating system (XTEST on Xvfb, `SendInput` on Windows) rather than `dispatch_event`; not tried.
- Scale factors other than 1 on CI (Xvfb and the Windows runner both ran at 1).
- Everything in the virtual machine section that says "likely", "not checked" or "not confirmed": no virtual machine
  was run.
- Whether `test-support` changes GPUI's runtime behaviour on the real platforms beyond adding APIs (the runs here
  showed nothing, but it was not audited).

## Revisit when

The GPUI snapshot is upgraded (re-run the workflow; check whether Linux windows gained `render_to_image` and whether
the debug tree gained bounds); the runner images change their display or Mesa versions in a way that breaks a run; the
maintainer approves (or declines) UTM, or Windows interaction screens arrive in M4; GitHub offers hosted runners with
GPUs for public repositories; or a tester runs a Tier 2 build on real hardware.
