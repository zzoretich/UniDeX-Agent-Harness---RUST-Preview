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

Evidence: CI run 44131744609 (workflow `spike-packaging`, the comparison; deleted at the milestone gate) and run
44131744691 (`nightly`), both on commit c69532c, plus a second round recorded in the Evidence section. Tool facts come
from each project's repository, documentation and crates.io page, read on 2026-11-17; "unverified" is said where a claim
rests on a document or source reading and no run exercised it.

## Options considered

| Option | macOS | Linux | Windows | Updates | Verdict |
|---|---|---|---|---|---|
| A. cargo-packager 0.11.8 (Apache-2.0 OR MIT; released 2025-11-27) | `.app` and `.dmg`: produced (run 44131744609, macOS job). Nested apps (`embeddedApps`), frameworks, signing and notarization are in the config and source (unverified by a run). | `.deb`: produced and installed on the runner. No maintainer scripts; a setuid file is installed as 0755 (measured). AppImage: failed without an icon (round 2 retries with one). `.tar.gz` plus PKGBUILD for pacman: produced; no plain tarball format. | NSIS `.exe` and WiX `.msi`: both produced. | Separate crate `cargo-packager-updater` 0.2.3, optional: minisign-signed, HTTPS endpoints, formats app, appimage, nsis, wix (no deb), no deltas. | Chosen. |
| B. cargo-deb 3.8.0 (MIT; 2026-09-01), Linux `.deb` only | none | `.deb` with explicit modes and maintainer scripts: produced, 4755 kept, postinst ran on install (measured). | none | none (apt is the update channel) | Kept as the fallback for the Linux `.deb` when the browser's setuid helper arrives. |
| C. Velopack vpk 1.2.161 (MIT; 2026-09-29) | `.app`, `.pkg`, portable zip; first run failed (the macOS image has no .NET 10 runtime); round 2 retries with it installed | AppImage only, 17.1 MB (measured). No `.deb`, no tarball. | `Setup.exe` per user, with its own launcher stub and `Update.exe` (documented; run in round 2) | Own system with delta packages and its own feed files. | Rejected: cannot make the required Linux formats, and the browser cannot ship in an AppImage (design 5.8). |
| D. cargo-dist 0.34.0 (MIT OR Apache-2.0; 2026-11-02) | archives and shell/npm/homebrew installers; `.dmg` and `.app` are listed in its docs as future work | archives only | archives and an MSI through cargo-wix | its own receipt-based updater for the shell installers | Rejected: no `.dmg`, `.app` or `.deb`; packages Cargo binary targets only (the first round found "nothing to release" until the probe crate was marked `dist = true`). Round 2 records what it makes. |
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

Sizes are bytes as printed by the runs (`diag.sh` and `ls -l`); MB here is 10^6 bytes. Every package is unsigned.

| Platform | What | Size | Run and job |
|---|---|---|---|
| macOS arm64 | staged program (Mach-O) | 8,838,832 | 44131744609, `spike packaging (macos)` 134210392294 |
| macOS arm64 | `UniDeX Rust Preview_0.0.1_aarch64.dmg` (cargo-packager) | 3,221,292 | same job |
| macOS arm64 | the `.app` inside: `Info.plist` 1,145 and the program only | 8,839,977 (sum of the two files) | same job |
| Linux x64 | staged program (ELF, unstripped, default release profile) | 38,614,040 | 44131744609, 134210392783 |
| Linux x64 | `.deb` (cargo-packager, gzip) | 12,169,866 | same job; also 44131744691, 134210450945 |
| Linux x64 | `.deb` (cargo-deb) | 8,183,212 | 134210392783 |
| Linux x64 | `.tar.gz` of program and LICENSE | 12,208,783 | 44131744691, 134210450945 |
| Linux x64 | pacman `.tar.gz` (cargo-packager) | 12,169,242 | 134210392783 |
| Linux x64 | AppImage (Velopack) and its `nupkg` | 17,107,448 and 16,580,740 | 134210392783 |
| Windows x64 | staged program (PE, console subsystem) | 10,398,208 | 44131744609, 134210393055 |
| Windows x64 | NSIS `unidex-rust-preview_0.0.1_x64-setup.exe` | 4,292,397 | same job; nightly 4,291,144 (134210450827) |
| Windows x64 | WiX `.msi` | 4,452,352 | 134210393055 |

Run artifacts: `nightly` uploaded `unidex-rust-preview-nightly-unsigned-linux` (24,264,247 bytes, deb and tarball
zipped) and `...-windows` (4,273,295 bytes); the macOS job failed after packaging (see "What went wrong").

What the sizes show and do not show: every package is far under 40 MB, but the program is a one-window example with no
application code, no assets and no icons, built with the default release profile (no stripping or link-time
optimization), so the numbers measure the packaging format's overhead and a baseline, not the product against the
target. The download target is to be re-measured with the real app and a tuned profile. The browser component is not
included in any of them.

### What went wrong in round 1 and what was learnt

- The macOS nightly package built (3.2 MB `.dmg`) but the inspection step failed: the `.dmg` carried a click-through
  licence agreement (cargo-packager adds one when `licenseFile` is set, "Adding EULA resources"), and
  `hdiutil attach` answered "attach canceled". The licence is now shipped as a resource instead and the step answers
  the prompt.
- The Windows trial hung for two hours: after packaging, the silent NSIS install (`setup.exe /S`, started from bash)
  never returned; the log's last line names the orphaned installer process. Cause not yet established. Every
  third-party tool and installer step now has its own `timeout-minutes`, and the install step waits with a limit and
  reports the process list.
- cargo-dist found nothing to release (the probe crate is `publish = false`); Velopack's `vpk` needs a .NET 10 runtime
  that the macOS image lacks (it was present on Linux). Both are retried in round 2.
- cargo-packager's AppImage failed ("Could not find a square icon"); retried with an icon.
- `ldd` of the Linux program lists only libxcb, libxkbcommon and libxkbcommon-x11 beyond libc and libgcc; fontconfig,
  freetype, Wayland and Vulkan are not linked at load time. The `.deb`'s `Depends` line (libfontconfig1,
  libfreetype6, libwayland-client0, libvulkan1 besides those) installed and ran `ldd` with no missing library on the
  runner, which shows the list is sufficient for loading, not that it is minimal or that the window opens.

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
- Linux formats: `.deb` and a tarball now; the `.deb` moves to cargo-deb when the setuid helper arrives (measured to
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

Round 2 fills or corrects this list.

- Opening any package on real hardware; the macOS package opened by hand (the maintainer's step, below); Gatekeeper
  behaviour of a downloaded copy.
- Signing, notarization and the secret names above; the updater.
- Nested macOS helper bundles, a Windows bootstrap arrangement, and the browser component's size in a package.
- Velopack on macOS and Windows and cargo-dist's output, until round 2.
- The silent NSIS install (the round-1 hang) and the WiX install.

## Maintainer's step: the macOS package by hand

After a green `nightly` run: on the run's page download the artifact `unidex-rust-preview-nightly-unsigned-macos`,
unzip it, and open the `.dmg`; drag the app to a scratch folder (not Applications). The package is unsigned, and a
browser download is quarantined, so macOS will say the app cannot be opened because Apple cannot check it for malicious
software; on current macOS, open System Settings, Privacy & Security, and choose "Open Anyway" for it (a right-click Open
no longer works), or clear the flag with `xattr -dr com.apple.quarantine <the app>` in Terminal. A window titled
"UniDeX cookbook" with a Count button should open. Afterwards drag the app to the Trash, eject the disk image, and delete
the download. The app writes nothing outside its own window.

## Revisit when

- The first signed or notarized release is prepared (identity confirmed; secrets supplied); the browser is packaged
  (re-trial nested bundles, the setuid `.deb` and the Windows bootstrap); cargo-packager has no release by
  mid-2027 or breaks on a pinned toolchain; or a measured download size exceeds the 40 MB target.
