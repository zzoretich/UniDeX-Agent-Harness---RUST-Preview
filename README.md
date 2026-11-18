# UniDeX Agent Harness (Rust)

A Rust rebuild of the UniDeX Agent Harness: its own agent engine that works with any hosted or local model, with the
vendors' coding agents as optional add-ons, and a native interface built on GPUI. It plans to target macOS, Linux and
Windows.

**Status: pre-release, and not usable as an application.** Milestone M0, the foundations, is complete and was signed off
by the maintainer on 2026-11-17, who decided the questions due before M1 is planned (questions 1 to 5 of the gate
report); the rest are open. It produced a workspace skeleton, CI on three platforms, a privacy gate, a threat model, an
acceptance checklist, a set of trials with written decisions, and a small cookbook of GPUI examples. The only user-facing
programs are `unidex --version` and the example windows. Nothing here chats, runs an agent or edits anything yet.

## Where to read

- The design: [docs/design/specs/2026-11-16-unidex-rust-design.md](docs/design/specs/2026-11-16-unidex-rust-design.md)
- The M0 gate report, with the open decisions: [docs/build/M0-GATE.md](docs/build/M0-GATE.md)
- The decision records: [docs/decisions/](docs/decisions/README.md)
- The threat model: [docs/security/THREAT-MODEL.md](docs/security/THREAT-MODEL.md), and the security policy:
  [SECURITY.md](SECURITY.md)
- The acceptance checklist: [docs/acceptance/CHECKLIST.md](docs/acceptance/CHECKLIST.md)
- The task ledger: [docs/build/LEDGER.md](docs/build/LEDGER.md)

## Build and check

The Rust toolchain is pinned in `rust-toolchain.toml` (1.100.0); `rustup` selects it on its own. CI runs the tests and the
examples build on macOS, Linux and Windows, and the other checks on Linux only. The list is the part a contributor
runs locally; CI also runs a mutation-testing check (on Linux, on the crates it mutates) that is not listed:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --examples --locked
cargo deny --locked check
cargo audit
cargo xtask privacy --all
```

After cloning, run `cargo xtask hooks install` once. It writes a pre-commit hook and a commit-msg hook that run the
privacy gate on the staged content and on the commit message before each commit. They fail closed: a commit is refused
when the gate finds something or cannot run. CI scans only after a push, and on a public repository a push is already
publication. A contributor may also keep a private list of terms, one per line, in `privacy-terms.local.txt` at the
repository root; git ignores that file, and the gate checks its terms when it is present.

`cargo deny` and `cargo audit` are separate tools (`cargo install --locked cargo-deny cargo-audit`; the pinned versions
are in `.github/workflows/ci.yml`). Building on Linux needs a few system packages for GPUI; the list is in
[decision 0007](docs/decisions/0007-gpui-source-and-pins.md).

## The cookbook

`crates/udx-cookbook` holds small examples of GPUI on the exact snapshot this repository pins. CI builds every example.
For example:

```sh
cargo run -p udx-cookbook --example hello_window
```

The crate's [README](crates/udx-cookbook/README.md) lists the examples.

## Nightly trial packages

A nightly workflow builds unsigned trial packages of the cookbook window for macOS, Linux and Windows and attaches them
to its workflow run for 7 days, where any signed-in user of the hosting service can download them. The workflow
creates no release; the packages are not signed or notarized, and are named so
([decision 0014](docs/decisions/0014-packaging-and-identity.md)).

## Licence

Licensed under the MIT License.

Scratch line, to be discarded.
