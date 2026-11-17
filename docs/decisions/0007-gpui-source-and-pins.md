# 0007: GPUI source and pins

Status: accepted
Date: 2026-11-17

## Context

Decision 0002 put the whole UI on GPUI. GPUI is pre-1.0 and changes often, so the design (sections 3 and 4.2) requires
it to be consumed as a snapshot of one specific upstream commit, with the exact source, the pinned versions and the
upgrade procedure written down in M0, and every later upgrade made as its own change. Section 5.10 asks for "GPUI with
the pinned widget kit", and section 3 notes that the kit, published as `gpui-kit`, pins such a snapshot. Section 11.3
asks that agents work from the pinned source and a cookbook crate that CI builds, with a shared compiler cache so many
worktrees on a 16 GB machine stay affordable. Section 11.4 sets the provenance rule: GPUI is permissively licensed, but
much of the editor it comes from is not.

Constraints that decide between candidates: the licence policy in `deny.toml` (no GPL, AGPL or LGPL-only crate, no
licence outside the allow list, crates.io only unless a git source is allowed by name), Rust 1.100.0 as pinned, builds
on macOS, Linux and Windows, and a build cost a 16 GB Apple silicon Mac can carry for several worktrees.

Method: every fact below was checked on 2026-11-17 against crates.io's API, the projects' repositories (read with the
GitHub API) and their documentation sites, and each serious candidate was built here (Apple M4, 10 cores, 16 GB,
Xcode 27.1) as a minimal window program in a scratch project, then run through `cargo deny --locked check` with this
repository's `deny.toml`. Leads from an earlier look were treated as unverified; how each turned out is noted.

## Options considered

| Option | Trade-off |
|---|---|
| A. `gpui` on crates.io, `=0.2.2` | Zed's own crate name, but its newest release is 13 months old; fails the licence policy (NCSA) and carries 11 unmaintained advisories. Rejected. |
| B. `gpui-kit = "=0.10.0"` (the widget kit, which brings its own pinned snapshot) | One line brings GPUI and the styled kit; its styled layer fails the licence policy (0BSD), and the kit is a larger, separate choice. Not adopted now. |
| C. The kit's snapshot crates directly: `gpui-pre` and `gpui-pre-platform`, `=0.3.12` (chosen) | crates.io only, checksummed, the exact snapshot `gpui-kit 0.10.0` requires, weekly upgrades by changing two version strings; published by the kit's maintainers, not Zed, with a few files adapted. |
| D. A git dependency on `zed-industries/zed` at a full commit hash | Byte-for-byte upstream, but seven git sources to allow, a 672 MB fetch for every fresh cargo home or runner, Zed's own patches not applied, and no way to add the kit later. Rejected. |

Also seen and not compared in depth: `gpui-ce` (a community fork, crates.io 0.3.0 of 2026-10-20, github.com/gpui-ce/gpui-ce)
and GPUI Fast (a reworked fork by the kit's maintainers, github.com/longbridge/gpui-fast, an opt-in backend of the kit).
Both are forks with their own changes; the design names the kit's snapshot of upstream GPUI.

### A. The `gpui` crate on crates.io

- What and who: Zed's GPUI crate, published from github.com/zed-industries/zed by Zed Industries
  (https://crates.io/crates/gpui). Newest version 0.2.2, published 2025-10-22, from upstream commit
  `69e2130295c2649963eb639fc70b4f2ee8ea1624` (tag `gpui-v0.2.2`, 2025-10-22). Nothing newer has been published; upstream
  `main` still says 0.2.2 in the manifest while the code has moved on. The earlier lead ("0.2.x") holds.
- Licence: Apache-2.0. `cargo deny`: licences FAILED, because `image` 0.25 with default features pulls an AVIF encoder
  (`ravif`, `rav1e`) and with it `libfuzzer-sys` 0.4.13, licensed `(MIT OR Apache-2.0) AND NCSA`. Passing would need NCSA
  added to the allow list, which this decision may not do. Advisories: 11 distinct unmaintained notices
  (RUSTSEC-2024-0384, 2024-0436, 2025-0052, 2025-0134, 2026-0173, 2026-0192, 2026-0206, 2026-0370, 2026-0371,
  2026-0372, 2026-0441). Bans and sources ok.
- macOS: the default build failed: its build script runs the Metal shader compiler, and this Mac's Xcode has no Metal
  Toolchain component ("cannot execute tool 'metal' due to missing Metal Toolchain"). With the `runtime_shaders` feature
  it built. Rust also warned that two of its dependencies (`block` 0.1.6, `proc-macro-error2` 2.0.1) contain code a
  future version will reject.
- Linux and Windows: not established.
- Upgrades: only when Zed publishes, which it has not done for 13 months.
- Widget kit: incompatible. The kit is built on the `gpui-pre` crates; a second copy of GPUI shares no types with it.

### B. `gpui-kit`, the widget kit

- What and who: GPUI Kit by Longbridge (https://crates.io/crates/gpui-kit, github.com/longbridge/gpui-kit), an umbrella
  crate over `gpui-base` (unstyled behaviour) and `gpui-component` (styled components), which re-exports GPUI. Version
  0.10.0, published 2026-11-16; releases are weekly (0.7.1 on 2026-10-05 to 0.10.0). It requires `gpui-pre =0.3.12`,
  `gpui-pre-macros =0.3.12` and `gpui-pre-platform =0.3.12` exactly. The earlier leads hold: the umbrella exists as
  described, and it pins a `gpui-pre` snapshot. The lead that the kit's docs require git dependencies is out of date:
  the Getting Started page (gpui-kit.com/docs/getting-started, read 2026-11-17) shows only crates.io lines, including
  the two `gpui-pre` lines chosen below for applications that use GPUI directly.
- Licence: the crates say Apache-2.0 (the repository's documentation prose is also offered under CC BY 4.0).
  `cargo deny` with default features: licences FAILED, because `gpui-component` depends on `enum-iterator` 2.3.0 and
  `enum-iterator-derive` 1.5.0, licensed 0BSD, which is not on the allow list. Advisories: 8 distinct unmaintained
  notices (the five listed under the decision, plus RUSTSEC-2024-0384 `instant`, 2024-0436 `paste`, 2025-0134
  `rustls-pemfile`). With `default-features = false` (only `gpui-base`): licences ok, 6 advisories (the five plus
  `instant`).
- macOS: built with default features, cold, in 61 s; a window with a kit `Label` and `Button` compiled.
- Linux and Windows: the kit's own CI on its v0.10.0 commit (`f697c3133d55c64fa8a5b67926147fb3dadd276d`) passed its
  Test jobs on macos-latest, ubuntu-latest and windows-latest, built against `gpui-pre 0.3.12`.
- Upgrades: a new kit release names its snapshot; the kit's CI fails when a pin is not exact.

### C. The kit's snapshot crates, `gpui-pre` (chosen)

- What and who: snapshots of Zed's GPUI crates, published to crates.io under `gpui-pre-*` names by the GPUI Kit
  maintainers with the kit's `script/bump-gpui.ts` (https://crates.io/crates/gpui-pre). Zed owns the `gpui` names on
  crates.io and publishes rarely, which is why the snapshots exist. Each crate keeps its upstream library name, so code
  says `use gpui::*`. Version 0.3.12, published 2026-11-09, is a snapshot of upstream commit
  `d2a4fb6ded7c470be25d1541281da3882495b65b` (committed 2026-11-08), recorded in each crate's description and in
  `[package.metadata.gpui-pre]`. Snapshots are published weekly (0.3.6 on 2026-09-21 to 0.3.13 on 2026-11-16).
- What changed from upstream, checked here: the sources of all 20 `gpui-pre-*` crates in the lockfile were compared with
  a checkout of upstream at that commit. They are identical except six modified files and one added file, each starting
  with a "Modified for gpui-pre" notice: `gpui/src/action.rs` and `gpui_macros/src/gpui_macros.rs` (plus the added
  `gpui_pre_facade_paths.rs`) make macro paths work under the new package names; `gpui_wgpu/src/wgpu_atlas.rs`,
  `wgpu_context.rs` and `wgpu_renderer.rs` adapt the Linux and web renderer to the crates.io release of `wgpu` 30.0.1
  instead of the unreleased `wgpu` revision Zed uses; `gpui_apple/build.rs` reads its sources from a vendored path. The
  manifests are rewritten for the new names.
- Licence: Apache-2.0, with Zed's licence file and notices kept. `cargo deny`: licences, bans and sources ok (crates.io
  only, no git source); advisories: the five unmaintained notices listed under the decision.
- macOS: the minimal window built cold in 34 s; the cookbook example below builds and runs.
- Linux and Windows: the kit's CI (above) builds and tests against this exact snapshot on all three platforms. Zed's CI
  on the upstream commit passed its Linux, macOS and Windows bundle jobs and its Linux tests. This repository's own build
  there is proven only by CI.
- Upgrades: change two version strings and the lockfile (procedure below). Rehearsed here: moving to 0.3.13 in a
  scratch copy updated all 20 crates with one `cargo update` and the cookbook still type-checked.
- Widget kit: exactly the snapshot `gpui-kit 0.10.0` requires, so adopting the kit later changes no GPUI line.

### D. A git dependency on a fixed upstream commit

- What and who: `gpui` and `gpui_platform` straight from github.com/zed-industries/zed at a full commit hash. Tried at
  the same commit as option C (`d2a4fb6ded7c470be25d1541281da3882495b65b`).
- Licence: the GPUI crates are Apache-2.0; the repository also holds GPL-3.0-or-later crates (the editor and its
  terminal view among them), none of which reached the build. `cargo deny`: licences ok; advisories: the same five.
- Sources: seven git repositories in `Cargo.lock`, every one of which would have to be allowed in `deny.toml`:
  `zed-industries/zed`, `gfx-rs/wgpu`, `zed-industries/font-kit`, `zed-industries/oo7`, `zed-industries/scap`,
  `zed-industries/wasm_thread` and `zed-industries/xim-rs`. Six are in `cargo deny`'s graph (`wasm_thread` is for
  WebAssembly only); with them allowed, sources pass.
- macOS: resolving took 84 s and fetched 672 MB of git data (the Zed repository's history alone is 412 MB), which every
  fresh cargo home and every CI runner without a cache repeats. The build then took 34 s.
- Other costs: Zed's own `[patch.crates-io]` entries (for example `async-task` and `calloop`) do not apply to a project
  that depends on it, so the result differs from Zed's own build unless they are copied in. Linux and Windows: Zed's CI
  builds that commit on all three, but not as a dependency of another project.
- Upgrades: change the commit hash and re-audit the git sources each time.
- Widget kit: incompatible, as for option A.

## Decision

GPUI comes from crates.io as the `gpui-pre` snapshot that the widget kit pins, version 0.3.12 (upstream commit
`d2a4fb6ded7c470be25d1541281da3882495b65b`), pinned exactly, once, in the workspace `Cargo.toml`:

```toml
[workspace.dependencies]
gpui = { package = "gpui-pre", version = "=0.3.12" }
gpui_platform = { package = "gpui-pre-platform", version = "=0.3.12", features = ["font-kit", "x11", "wayland", "runtime_shaders"] }
```

A crate that draws UI writes `gpui.workspace = true` and `gpui_platform.workspace = true` under `[dependencies]`. The
other 18 `gpui-pre-*` crates follow, because every requirement between them is `=0.3.12`, and `Cargo.lock` (committed)
pins every crate with its checksum. No git source is added to `deny.toml`.

The `gpui_platform` features are the kit's own set, so adding the kit later changes nothing: `font-kit` draws text on
macOS (without it GPUI lays text out but draws no glyphs), `x11` and `wayland` are the Linux window backends, and
`runtime_shaders` compiles the Metal shaders when the app starts. The last one is needed here: without it the build runs
Apple's Metal compiler, which this Mac's Xcode 27.1 lacks (Xcode ships it as a separate download), and which a CI runner
may lack too.

**Widget kit: deferred.** `gpui-kit` is not a dependency yet. Its styled layer (`gpui-component` 0.10.0) needs the 0BSD
licence. The maintainer allowed 0BSD on 2026-11-17 (see "Maintainer decisions" below), so the licence no longer rules
the kit out; the kit's tree carries a few more unmaintained notices than GPUI's own, which under the scope rule below are
warnings, not failures, unless the kit makes one of those crates a direct dependency of this workspace. The choice
between the styled kit, `gpui-base` alone (which passes the policy) and plain GPUI belongs to the trial tasks (Tasks 10
and 11), which build real screens. Whatever they choose, this pin is the snapshot `gpui-kit 0.10.0` requires, so adding
`gpui-kit = { version = "=0.10.0", ... }` later moves no GPUI version.

**Advisories: decided by the maintainer** (see "Maintainer decisions" below; the rest of this paragraph is the
question as it stood). Every candidate's tree carries five informational "unmaintained" notices with
no patched release: RUSTSEC-2026-0192 (`ttf-parser`), RUSTSEC-2026-0206 (`rustybuzz`), RUSTSEC-2026-0370 (`zvariant`),
RUSTSEC-2026-0371 (`zbus_names`) and RUSTSEC-2026-0372 (`zvariant_derive`). None is a vulnerability. `ttf-parser` is a
direct dependency of `gpui-pre`; `rustybuzz` comes through `usvg`; the three `zbus` 5 crates come through the Linux
accessibility, portal and secret-service crates. `cargo deny` treats unmaintained notices as errors, so with this pin
`cargo deny --locked check` fails its advisories check (licences, bans and sources pass), and it would with any other
candidate. Making it pass needs a change to the advisory policy, which is the maintainer's to make, not this
decision's: for example ignoring exactly these five IDs in `deny.toml`, each with its reason, so that any other advisory
still fails. Nothing is ignored here. `cargo audit` reports the five, and `paste` (RUSTSEC-2024-0436, present in
`Cargo.lock` but not in any build), as warnings and passes.

## Maintainer decisions (2026-11-17)

Two questions this record raised were put to the maintainer, who decided both:

- **Unmaintained notices fail the check only for direct dependencies.** The maintainer first accepted the five
  notices by name. The same day the advisory database gained a sixth in GPUI's tree (RUSTSEC-2026-0543, `chrono`,
  which `gpui-pre` uses directly), which showed that naming each one would turn the check red, on `main` and on every
  pull request, whenever any crate in that tree is marked unmaintained. Asked again, the maintainer chose the scope
  rule instead: `deny.toml` sets `unmaintained = "workspace"`, so a notice fails only for a crate this workspace
  depends on directly. Notices deeper in the tree are printed as warnings by `cargo audit` and are reviewed at every
  GPUI upgrade. Vulnerabilities, unsound notices and yanked crates still fail everywhere. The alternatives offered
  were to keep naming each notice (rejected for the reason above) and to leave the check failing. No advisory is
  ignored by name.
- **The 0BSD licence is allowed.** It is added to the licence allow list. It sets no conditions, so it is compatible
  with the project's MIT licence.

## Who decided

A design call, within the maintainer's decision 0002. The advisory scope rule and the 0BSD allowance were decided by
the maintainer.

## Consequences

- Every UI crate shares one GPUI, set in one place. An upgrade is a two-line change plus the lockfile, on its own branch.
- The project trusts the kit's maintainers to republish upstream faithfully. The check above found only documented,
  noticed changes for 0.3.12; the same comparison is part of every upgrade.
- On Linux the renderer is the kit's adaptation to `wgpu` 30.0.1, not exactly what Zed ships.
- Shaders are compiled at app start (`runtime_shaders`). Packaging (Task 14) may prefer shaders compiled at build time,
  which needs Apple's Metal Toolchain on the build machine; that is its own choice.
- `crates/udx-cookbook` holds the examples later tasks copy from, starting with `hello_window`; CI builds them on all
  three platforms. Its README carries the dependency lines, the sccache instructions, the upgrade procedure and the
  provenance rule.

### Upgrade procedure

1. Pick the `gpui-pre` version the newest `gpui-kit` release requires (from its crates.io dependencies), unless a later
   decision changes the source. Note its upstream commit and date from the crate's metadata.
2. Change both version strings in `[workspace.dependencies]`, keeping `=x.y.z`, then run
   `cargo update -p gpui-pre -p gpui-pre-platform` and read the lockfile diff.
3. Compare the new snapshot's sources with upstream at its recorded commit, as done above: check out upstream at that
   commit, `diff -r` each upstream crate's `src` against its `gpui-pre-*` copy in the cargo registry, and read every
   "Modified for gpui-pre" notice. Any change without a notice stops the upgrade.
4. Build and run the cookbook, fix what broke, and run the full local gate: format, clippy with `-D warnings`, tests,
   `cargo deny --locked check`, `cargo audit`. List every advisory the new tree carries in the new record; ignoring one
   needs the maintainer's agreement.
5. Write a decision record that supersedes this one (version, commit and date, packages, licences, advisories, build
   times) and update the cookbook README.
6. Open the pull request alone, with no feature work, and let CI build the cookbook on all three platforms.

### Build cost

Measured in this worktree on the machine above with `sccache` 0.19.0 turned on (`RUSTC_WRAPPER=sccache`, a separate
cache folder that started empty). Crates were already downloaded, so download time is not included. Peak memory is the
summed resident memory of all cargo, rustc, sccache, linker and build-script processes, sampled every 0.2 s, so short
spikes can be missed.

| Build (`cargo build -p udx-cookbook --example hello_window --locked` unless noted) | Time | sccache hits | Peak memory |
|---|---|---|---|
| Cold: empty `target`, empty sccache | 36.9 s | 0 of 299 | 2.2 GB |
| Warm, fresh worktree: a copy of the tree at another path, empty `target`, populated sccache | 24.6 s | 232 of 299 (78 %) | 1.9 GB |
| Warm, same path: `target/debug` deleted, populated sccache | 12.2 s | 299 of 299 | 1.1 GB |
| Incremental: example file touched / one line added | 0.8 s / 0.5 s | | 0.7 / 0.8 GB |
| Whole workspace, as CI runs it: `cargo test --workspace --locked`, fresh copy, no sccache, `CARGO_INCREMENTAL=0` | 38.2 s | | 2.4 GB |

Sizes: `target/debug` was 1.7 GB after the cold build of the example and 2.5 GB after clippy and the workspace tests;
the sccache folder held 375 MB at the end. The figure later task budgets should assume for a new worktree is the
warm, fresh-worktree one: about 25 s on this machine with sccache on. The gap between the two warm rows is the crates
that miss the cache when the worktree sits at another path, most likely because their compiler inputs include the
build directory's path (build-script output); this was not investigated further.

`sccache` is optional and not configured in the repository, so machines and runners without it keep working; the
cookbook README says how to turn it on for a session. `cargo-hakari` (a unified dependency set) is not set up: it is not
installed, and it is deferred until more than one crate depends on GPUI.

### Linux system packages

Building needs `pkg-config`, `libfontconfig-dev` (fontconfig and freetype, found by the build scripts of
`yeslogic-fontconfig-sys` and `freetype-sys`), `libxcb1-dev` (`x11rb` links `libxcb`) and `libxkbcommon-x11-dev` (links
`xkbcommon` and `xkbcommon-x11`). CI installs them with `apt-get`, unpinned. The list comes from the native libraries in
this lockfile's Linux graph, checked against Zed's `script/linux` and the kit's `script/install-linux.sh`. Not needed to
build: `libwayland-dev` (Wayland is loaded at run time), and the kit's extras for its web view, speech and TLS. Running
a window on Linux also needs a Vulkan driver (`libvulkan1` and a GPU or software driver); that belongs to Task 13.
macOS should need only the Xcode command line tools (the kit's installation page says so; not tried here without full
Xcode). Windows needs the MSVC toolchain and Windows SDK, which the runner image has.

### What remains unverified

- That this repository builds on Linux and Windows: only CI can show it, including that the package list is complete.
- That the window renders on Linux and Windows (Task 13), and what it looks like on macOS (the maintainer's check).
- The start-up cost of `runtime_shaders`.
- Screen reader support: the snapshot has AccessKit roles and labels, and the example uses them; Task 11 tests them.
- How large the CI caches grow on each platform, and whether they stay inside the repository's cache quota.

## Revisit when

A newer `gpui-kit` release pins a newer snapshot and an upgrade is planned; the kit's maintainers stop publishing
snapshots, or a snapshot is found to differ from upstream in an undocumented way; Zed starts publishing `gpui` to
crates.io regularly; a trial (Tasks 10 to 13) shows the snapshot failing on a platform; the maintainer decides how to
treat the five advisories or the 0BSD licence; or a second crate depends on GPUI, which brings `cargo-hakari` back into
view.
