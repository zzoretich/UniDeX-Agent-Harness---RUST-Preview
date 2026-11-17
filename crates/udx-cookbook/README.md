# udx-cookbook

Small, compiled examples of GPUI, the UI framework UniDeX draws with, on the exact snapshot this repository pins. Agents
and people write UI code from these examples and from the pinned source, not from memory of the framework. CI builds
every example on macOS, Linux and Windows, so an example that stops compiling fails the build.

The crate publishes nothing and its library is empty. Each example is one file in `examples/`. Later tasks add examples
here when they find a pattern worth keeping.

| Example | What it shows |
|---|---|
| `hello_window` | A window titled "UniDeX cookbook" with one label and one button that counts clicks |

Why this source and these pins: [decision 0007](../../docs/decisions/0007-gpui-source-and-pins.md).

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

Building on Linux needs a few system packages (the list is in decision 0007 and in `.github/workflows/ci.yml`). Before a
full build, `cargo check -p udx-cookbook --examples` is the cheaper way to see whether an example compiles.

## Share compiled dependencies between worktrees (sccache)

Every worktree has its own `target` directory, so without a shared cache each one compiles GPUI from scratch. `sccache`
keeps compiled crates in one cache that every worktree reads. It is optional and is not configured in the repository, so
machines and CI runners without it keep working. To turn it on for one shell session:

```sh
export RUSTC_WRAPPER=sccache
```

Then build as usual. `sccache --show-stats` shows the hit rate. Turn it off with `unset RUSTC_WRAPPER`. The cache lives
in sccache's default folder, limited to 10 GB unless `SCCACHE_CACHE_SIZE` says otherwise. Measured build times are in
decision 0007.

## Upgrade GPUI

An upgrade is its own change, on its own branch, never mixed with feature work.

1. Pick the new snapshot: the `gpui-pre` version that the newest `gpui-kit` release requires (read it from that
   release's dependencies on crates.io), unless decision 0007 has been superseded. Note the upstream commit it was cut
   from, which is in the crate's description and in `[package.metadata.gpui-pre]` of its `Cargo.toml`.
2. Change the version in both lines of `[workspace.dependencies]` in the root `Cargo.toml`, keeping the exact `=x.y.z`
   form, then run `cargo update -p gpui-pre -p gpui-pre-platform` so `Cargo.lock` follows (every other `gpui-pre-*`
   crate moves with them). Read the lockfile diff.
3. Check the snapshot against upstream: check out upstream at the recorded commit and `diff -r` each whole upstream
   crate folder (sources, `build.rs`, `vendor/`) against its `gpui-pre-*` copy in the cargo registry, and read every
   "Modified for gpui-pre" notice. Then compare each manifest's dependency tables (names, versions, features, and any
   swap from a git source to a registry release) with upstream's manifest and workspace `Cargo.toml`, including
   `[patch.crates-io]`. Any change that has no notice, or that decision 0007 does not document, stops the upgrade until
   it is understood and recorded.
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
permissively licensed. Reviewers check where code came from, and `cargo deny` checks every dependency's licence,
including GPUI's, which the cookbook takes as a dev-dependency (`include-dev = true` in `deny.toml`).
