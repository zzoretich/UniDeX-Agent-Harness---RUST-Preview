# 0014: Packaging tool and application identity

Status: proposed, awaiting the maintainer's confirmation of the identifier and name (the tool choice is a design call from the M0 trial and stands unless the maintainer reopens it)
Date: 2026-11-17

## Context

The design (section 8) wants a signed, notarized `.dmg` on macOS; a `.deb` and a tarball on Linux (an AppImage only
for a build without the browser); an installer on Windows, signed if a certificate is supplied. Signing identities are
supplied by the maintainer as CI secrets; without them builds are unsigned and say so. Section 9 sets a download size
of under 40 MB for the app without the browser; the security section asks for signed updates and pinned CI actions.
The maintainer has one Mac and no Linux or Windows machine, so every package is built on CI runners.

Decision 0012 adds requirements the tool must meet later: on macOS the browser's helper app bundles sit inside the
main bundle; on Linux the browser can ship only in a `.deb`, which must install a sandbox helper setuid root (a tarball
cannot); on Windows CEF's bootstrap executable loads the application as a DLL; the component is large (about 300 MB on
disk on macOS) and may be a separate download. So the tool must allow extra binaries, nested bundles, file modes such as
4755 and post-install steps. Nothing of that is packaged now: the program is the repository's cookbook window (the
example target `hello_window` of `udx-cookbook`).

Evidence: round 1, commit c69532c: runs 44131744609 (`spike-packaging`, the comparison; deleted at the milestone gate)
and 44131744691 (`nightly`). Round 2, commit bf7f040: runs 44154813495 (`spike-packaging`) and 44154813579 (`nightly`).
Job ids are in the Evidence section. Tool facts come
from each project's repository, documentation and crates.io page, read on 2026-11-17; "unverified" is said where a claim
rests on a document or source reading and no run exercised it.

## Options considered

| Option | macOS | Linux | Windows | Updates | Verdict |
|---|---|---|---|---|---|
| A. cargo-packager 0.11.8 (Apache-2.0 OR MIT; released 2025-11-27) | `.app` and `.dmg`: produced (run 44131744609, macOS job). Nested apps (`embeddedApps`), frameworks, signing and notarization are in the config and source (unverified by a run). | `.deb`: produced and installed on the runner. No maintainer scripts; a setuid file is installed as 0755 (measured). AppImage: failed without an icon, produced with one (12,163,576 bytes). `.tar.gz` plus PKGBUILD for pacman: produced; no plain tarball format. | NSIS `.exe` and WiX `.msi`: both produced. | Separate crate `cargo-packager-updater` 0.2.3, optional: minisign-signed, HTTPS endpoints, formats app, appimage, nsis, wix (no deb), no deltas. | Chosen. |
| B. cargo-deb 3.8.0 (MIT; 2026-09-01), Linux `.deb` only | none | `.deb` with explicit modes and maintainer scripts: produced, 4755 kept, postinst ran on install (measured). | none | none (apt is the update channel) | Kept as the fallback for the Linux `.deb` when the browser's setuid helper arrives. |
| C. Velopack vpk 1.2.161 (MIT; 2026-09-29) | `.app` in a portable zip, a `.pkg` installer and an update `nupkg`: produced unsigned (6.19 MB `.pkg`); needs a .NET 10 runtime that the macOS image lacks | AppImage only, 17.1 MB (measured). No `.deb`, no tarball. | `Setup.exe` (13.7 MB), portable zip and `nupkg` produced; installing it was not run | Own system with delta packages and its own feed files. | Rejected: cannot make the required Linux formats, and the browser cannot ship in an AppImage (design 5.8). |
| D. cargo-dist 0.34.0 (MIT OR Apache-2.0; 2026-11-02) | archives and shell/npm/homebrew installers; `.dmg` and `.app` are listed in its docs as future work | archives only | archives and an MSI through cargo-wix | its own receipt-based updater for the shell installers | Rejected on its documentation: no `.dmg`, `.app` or `.deb`; packages Cargo binary targets only. The trial never produced an archive or MSI: after the probe crate was marked `dist = true` it stopped on the MSI installer's missing `authors` field on all three OSes (my probe's fault, not a tool limit), so its output is unverified. |
| E. cargo-bundle 0.12.0, cargo-wix 0.3.9, cargo-generate-rpm 0.21.1 | `.app` only | `.deb` only (cargo-bundle) | MSI only | none | Not trialled: each covers one platform, which cargo-packager and cargo-deb already cover. |

### What each candidate answers for the browser requirements (decision 0012)

| Requirement | cargo-packager | cargo-deb | Velopack | cargo-dist |
|---|---|---|---|---|
| Extra binaries | `binaries` list, `externalBinaries`, `resources` | `assets` with any source and mode | any folder is the pack directory | extra bin targets of the package |
| Nested helper `.app` bundles on macOS | `macos.embeddedApps` copies them in and signs inside-out (source, `package/app/mod.rs`; no run yet) | not applicable | a pre-made `.app` is accepted as the pack directory, so the bundle is ours to build and sign | no |
| Linux setuid helper and post-install step | not possible in the tool: the `.deb` has only `control` and `md5sums`, and file modes are normalized (measured: a 4755 source became 0755). Needs a repack or cargo-deb. | yes (measured: `-rwsr-xr-x root:root` after install, postinst ran) | no `.deb` at all | no |
| Windows bootstrap executable that loads the app as a DLL | NSIS: the template can be replaced (`nsis.template`), and extra files are resources (unverified) | not applicable | its launcher stub starts `current\<mainExe>`, so the bootstrap could be `mainExe` (unverified) | no |
| Component too large for the installer | nothing in the tool stops the installer from omitting it (design: fetched on first use) | the `.deb` is the exception: the helper must be in it | n/a | n/a |

## Decision

Package with cargo-packager (A), configured in `packaging/Packager.toml`, for the macOS `.dmg` and the Windows NSIS
installer, and for the Linux `.deb` while no setuid file is needed. The Linux tarball is a plain `tar` of the staged
program. When the browser's sandbox helper is added, the Linux `.deb` is built with cargo-deb (B) from the same staged
files, because only B installs a setuid file and runs a maintainer script; the nightly keeps one workflow either way.
Velopack and cargo-dist are not used.

## Who decided

A design call from the M0 trial, from the runs listed here. The application identity (identifier and display name) is
the maintainer's to confirm.

## Evidence

Sizes are bytes as printed by the runs (`diag.sh`, `ls -l`); MB is 10^6 bytes. Every package is unsigned. Jobs: round 1
spike macOS 134210392294, Linux 134210392783, Windows 134210393055 (cancelled at the two-hour limit by a hung installer
step), nightly Linux 134210450945, Windows 134210450827, macOS 134210450914 (failed in my inspection step). Round 2
spike Linux 134292574002, Windows 134292574199, macOS 134308615307 (re-run, attempt 2); nightly decide 134292574357,
Linux 134292740520, Windows 134292740333, macOS 134304382864 (attempt 2).

| Platform | What | Size | Job |
|---|---|---|---|
| macOS arm64 | staged program (Mach-O) | 8,838,832 | 134308615307 |
| macOS arm64 | `.dmg` (cargo-packager), nightly build | 3,217,473 | 134304382864 |
| macOS arm64 | the `.app` inside: program, `Info.plist` 1,145, LICENSE 1,076 | 8,841,053 (sum) | 134308615307 |
| macOS arm64 | Velopack `.pkg`, portable zip, `nupkg` | 6,194,092; 6,186,848; 6,396,786 | 134308615307 |
| Linux x64 | staged program (ELF, unstripped, default release profile) | 38,614,040 | 134292574002 |
| Linux x64 | `.deb` (cargo-packager, gzip), nightly build | 12,170,818 | 134292740520 |
| Linux x64 | `.tar.gz` of program and LICENSE, nightly build | 12,208,803 | 134292740520 |
| Linux x64 | `.deb` (cargo-deb, round 1) | 8,183,212 | 134210392783 |
| Linux x64 | AppImage (cargo-packager, with icon) | 12,163,576 | 134292574002 |
| Linux x64 | AppImage (Velopack) and its `nupkg` (round 1) | 17,107,448; 16,580,740 | 134210392783 |
| Windows x64 | staged program (PE, console subsystem) | 10,398,208 | 134292574199 |
| Windows x64 | NSIS `-setup.exe`, nightly build | 4,290,350 | 134292740333 |
| Windows x64 | WiX `.msi` | 4,452,352 | 134292574199 |
| Windows x64 | Velopack `Setup.exe`, portable zip, `nupkg` | 13,707,366; 6,192,234; 6,193,254 | 134292574199 |

The three artifacts the nightly run 44154813579 attached (each kept 7 days, to 2026-11-24; zip sizes as GitHub reports
them): `unidex-rust-preview-nightly-unsigned-macos`, 3,195,296 bytes, holding
`unidex-rust-preview-0.0.1-nightly-unsigned-macos-arm64.dmg` (SHA-256 `3b6445698c07...bb57a`);
`unidex-rust-preview-nightly-unsigned-linux`, 24,265,559 bytes, holding `...-linux-x86_64.deb` (SHA-256
`fc4179f3a54f...ceb72b`) and `...-linux-x86_64.tar.gz` (SHA-256 `c56b3703c64e...a3cd`); and
`unidex-rust-preview-nightly-unsigned-windows`, 4,272,957 bytes, holding `...-windows-x86_64-setup.exe` (SHA-256
`ce76af09e53f...121e`). The `decide` job ran only because the event was a pull request; the schedule rule (skip when no
commit in 25 hours) was not exercised.

What each trial showed, read from the tool's own output:

- cargo-packager, macOS: `.app` and `.dmg` built (the Finder-layout script ran on the runner). The mounted `.dmg`
  holds the app and an `Applications` link. `codesign -dvvv` of the app: `Signature=adhoc`, flags
  `adhoc,linker-signed`, `Info.plist=not bound`, `Sealed Resources=none`; `codesign --verify` and `spctl --assess`
  both answer "code has no resources but signature indicates they must be present". So the unsigned app carries only
  the linker's signature on its executable and its bundle fails strict verification; real signing replaces this.
  `Info.plist` has the identifier, name, version 0.0.1 and `LSMinimumSystemVersion` 13.0; no icon.
- cargo-packager, Linux: `.deb` installed with `apt-get install ./x.deb` and removed; `ldd` showed no missing
  libraries; the LICENSE resource lands in `/usr/lib/unidex-rust-preview/`. The `deb.files` setuid probe: the 4755
  source became `-rwxr-xr-x` in the `.deb`. No maintainer scripts exist in the format.
- cargo-deb 3.8.0: `.deb` with `-rwsr-xr-x` kept, a `postinst` member, installed as `-rwsr-xr-x root:root`, and the
  postinst printed its marker on install.
- cargo-packager, Windows: NSIS and WiX built. The silent NSIS install (started with PowerShell `Start-Process`, with a
  120-second limit) returned, installed `unidex-rust-preview.exe`, `LICENSE` and `uninstall.exe` under
  `%LOCALAPPDATA%\UniDeX Rust Preview` (per user, no elevation), wrote an uninstall registry entry (name, version, publisher
  "UniDeX contributors", install location) and the silent uninstall removed the folder. The same installer started
  from a bash step in round 1 hung for two hours. The cause is not established (the PowerShell step in round 2 did not hang);
  the step printed a failing exit only because my script mixed output into its return value, not because the
  installer failed. The MSI lists the program and LICENSE in `app.cab`; it was not installed.
- Velopack: macOS (after installing .NET 10 with `actions/setup-dotnet`) produced the `.app`, `.pkg`, portable zip and
  `nupkg`, unsigned, with the warning "Package will not be signed or notarized"; the `.app` also carries its
  `UpdateMac` binary (7.7 MB) and a version file. Windows produced `Setup.exe`, portable zip, `nupkg`, unsigned. Linux:
  an AppImage only. Its `--version` flag does not exist (the "install" steps reported failure only because of that).
  None of its installers was run.
- cargo-dist: see the options table. It never reached its archives.

The nightly Windows installer's listing shows the same files. The Windows program is built as a console-subsystem
executable (the cookbook example), so a double-click would also open a console window; that is the example's property,
not the packaging's.

What the sizes show and do not show: every package is far under 40 MB, but the program is a one-window example with no
application code, assets or icons, built with the default release profile (no stripping or link-time optimization), so
the numbers measure the packaging formats' overhead and a baseline, not the product against the target (design section
9 says under 40 MB, without the browser). Re-measure with the real app and a tuned profile. The browser component is
included in none of them.

### What went wrong along the way

- Round 1 macOS nightly: my inspection step failed because the `.dmg` carried a click-through licence agreement
  (cargo-packager adds one when `licenseFile` is set). Now the licence is a resource and the attach step answers a
  prompt; round 2 is green.
- Round 1 Windows: the hang described above; every installer and tool step now has its own `timeout-minutes`.
- cargo-packager's AppImage needed an icon; its tooling (linuxdeploy, a "continuous" plugin release) is downloaded
  with no hash check.
- `ldd` of the Linux program lists only libxcb, libxkbcommon and libxkbcommon-x11 beyond libc and libgcc; fontconfig,
  freetype, Wayland and Vulkan are not linked at load time. The `.deb`'s `Depends` line (those plus libfontconfig1,
  libfreetype6, libwayland-client0, libvulkan1) installed with no missing library, which shows it sufficient for loading,
  not minimal and not that the window opens.

## Consequences

- Updates: the packaging choice does not force an update mechanism. `cargo-packager-updater` is a library the app may
  use (minisign signature checked against a public key compiled into the app, HTTPS endpoint returning a JSON file
  that can be a static file on any host); there are no delta updates, and `.deb` installs are not covered (apt or a
  manual download updates those). The design's "signed updates" can be met with it; whether to use it is a decision for
  the M4 auto-update work. Velopack's updater would have been a stronger feature set (deltas) but is tied to its own
  installers and launcher.
- Signing and notarization: none exists now, and no workflow references a secret. The maintainer would supply, as
  repository secrets: for macOS a Developer ID Application certificate as a base64 `.p12` (`APPLE_CERTIFICATE`) and its
  password (`APPLE_CERTIFICATE_PASSWORD`), the signing identity string (set in `packaging/Packager.toml`), and notarization
  credentials, either an Apple ID, an app-specific password and a team id (`APPLE_ID`, `APPLE_PASSWORD`,
  `APPLE_TEAM_ID`) or an App Store Connect API key (`APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_PATH`); for
  Windows a code-signing certificate usable on the runner (thumbprint) or a custom sign command in the config
  (`windows.signCommand`), for example for a cloud signing service; for updates a minisign key pair
  (`CARGO_PACKAGER_SIGN_PRIVATE_KEY` and `..._PASSWORD`). The names are from cargo-packager's source and have not been
  exercised. Without them every build stays unsigned and is named so.
- Linux formats: `.deb` and a plain tarball now; the `.deb` moves to cargo-deb when the setuid helper arrives (measured to
  work). cargo-packager's AppImage is unnecessary for the browser-less build until needed.
- Future browser component: the macOS nested bundles and the Windows bootstrap arrangement are not exercised by any
  run here; cargo-packager's `embeddedApps` and replaceable NSIS template make them plausible, and Velopack's
  stub launcher is a risk for the bootstrap arrangement. Re-trial both when the browser is packaged.
- Download size: see Evidence. The target is a later measurement.
- Supply chain: cargo-packager, cargo-deb and cargo-dist are installed with `cargo install --locked --version X`
  (cargo checks each crate against the registry checksum; no separate hash of the tool itself is published).
  cargo-packager also downloads NSIS and WiX at run time and verifies those archives against hashes compiled into it
  (NSIS 3.09 zip SHA-1, `nsis_tauri_utils.dll` SHA-1, WiX 3.11 SHA-256); its AppImage tooling (linuxdeploy and a
  "continuous" plugin release) is downloaded with no hash check, which is one more reason the AppImage is not used.
  Actions are pinned to commit SHAs; `actions/setup-dotnet` (trial only) at `0f4a354dd5547a28c70b7853dbd95d5b67886e73`
  (v6.1.0, from `gh api repos/actions/setup-dotnet/git/ref/tags/v6.1.0`).
- Tool risk: cargo-packager's last release is 2025-11-27, almost a year old, although its repository has commits in
  November 2026 and about 267,000 downloads. Its `.deb` lacks maintainer scripts and its `0.x` version means breaking
  changes; the config is small and isolated in `packaging/Packager.toml`, so replacing it is bounded.

## Proposed identity (for the maintainer to confirm)

Placeholders in use today (`packaging/Packager.toml`): identifier `example.unidex.rust-preview.trial`, name
"UniDeX Rust Preview", publisher, authors and Debian `Maintainer` "UniDeX contributors", copyright "UniDeX
contributors". Nothing carries a personal name, account handle or personal domain. A Debian `Maintainer` normally has
an address; `dpkg` accepted this one without (round 1), and the maintainer decides whether to add a project address.

Constraints:

- macOS `CFBundleIdentifier`: reverse-DNS, letters, digits, hyphen and period, compared case-insensitively; it keys
  the app's preferences, Keychain items and privacy permissions, so changing it later makes macOS treat the app as new.
- Windows: the identifier names the uninstall registry entry (and would be the AppUserModelID for taskbar and
  notifications: no spaces, 128 characters at most); changing it after release leaves the old install beside the new.
- Debian package name: lowercase letters, digits, `+`, `-` and `.`, at least two characters, starting with a letter or
  digit, so `unidex-rust-preview`; it cannot be the reverse-DNS string. The Linux desktop file and Flatpak style use
  the reverse-DNS id as well.

Options for the identifier:

1. `example.unidex.rust-preview` (no domain claimed; `.example` is reserved and can never be registered). Safe for
   trials and nightlies. Not suitable for a release the maintainer wants to keep: it must be replaced before the first
   signed preview, which resets any prefs and permissions.
2. `dev.unidex.rustpreview` (or another top-level domain) if the project registers the matching domain. Conventional
   and under the project's control, but this record has not checked that any such domain is available and the project
   owns none today. Using a name under a domain someone else owns is not blocked by Apple, but that owner could ship an
   app with the same identifier, so this option should wait for the domain.
3. `io.github.<project-organization>.unidex-rust-preview`, if the code moves to a GitHub organization named for the
   project (the current repository sits under a personal account, whose handle must not appear, so this needs the
   organization first). Collision-safe by construction and the usual convention for projects with no domain.

Recommendation: keep option 1 until a domain or an organization exists, and decide between 2 and 3 before the first
signed preview. For the display name: "UniDeX Rust Preview" (recommended: matches the edition's name and differs from
the Electron app's "UniDeX"), "UniDeX Preview", or "UniDeX (Rust Preview)" (parentheses are awkward in file names and
paths). Debian package and program file name: `unidex-rust-preview`.

## What was not verified

- Opening any package on real hardware, including the macOS package by hand (the maintainer's step, below) and
  Gatekeeper's verdict on a downloaded copy. The app bundle is not validly signed (see Evidence), so macOS may call a
  downloaded copy "damaged" rather than "unidentified"; this was not tried.
- Whether the Linux or Windows program opens a window at all (the runners have no display for it); installs were
  checked by file and library presence only.
- Signing, notarization and the updater, and the secret names (read from cargo-packager's source, never exercised).
- Nested macOS helper bundles, the Windows bootstrap arrangement, the setuid helper inside cargo-packager's `.deb`
  (shown impossible) and the browser component's size in a package.
- cargo-dist's output (its probe never got past the MSI configuration); installing Velopack's packages; installing
  the WiX `.msi`.
- The nightly schedule rule (skip when no commit in 25 hours): only the pull-request path ran.
- The Linux program's performance, size after stripping, or any size against the 40 MB target.

## What the maintainer must confirm or supply

- Confirm or replace the bundle identifier and display name (proposed identity, above), and the publisher and Debian
  `Maintainer` string "UniDeX contributors" (whether to add a project address), and the copyright line.
- Nothing needs supplying today: no secret exists and no workflow references one. For signed builds later: a Developer
  ID certificate and notarization credentials (macOS), a code-signing certificate or cloud signing service (Windows), an
  update signing key if the cargo-packager updater is used (see Consequences).
- Decide when to register a domain or create a project organization (identity options 2 and 3).
- Do the macOS hand check below once.

## Maintainer's step: the macOS package by hand

After a green `nightly` run, open the run page, download the artifact `unidex-rust-preview-nightly-unsigned-macos`
(about 3.2 MB), unzip it and open the `.dmg` inside; drag the app to a scratch folder, not Applications. The package is
unsigned, and a browser download is quarantined, so macOS will refuse to open it, saying it cannot verify the app is free
of malware or (because the bundle's signature is only the linker's) that it is damaged. If it offers a choice, open
System Settings, Privacy & Security and choose "Open Anyway" (on current macOS a right-click Open no longer works). If
it only says "damaged", run in Terminal on the copy: `xattr -dr com.apple.quarantine "<the app>"`, and if it still
refuses, `codesign --force --deep --sign - "<the app>"` (a local ad hoc signature on your own copy). A window titled
"UniDeX cookbook" with a Count button should open. Afterwards quit it, drag the app to the Trash, eject the disk image,
delete the download, and tell me what macOS said: that is the one result the runners cannot give.

## Revisit when

- The first signed or notarized release is prepared (identity confirmed; secrets supplied); the browser is packaged
  (re-trial nested bundles, the setuid `.deb` and the Windows bootstrap); cargo-packager has no release by
  mid-2027 or breaks on a pinned toolchain; or a measured download size exceeds the 40 MB target.
