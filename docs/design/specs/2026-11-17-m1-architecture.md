# M1 architecture: contracts for the vertical slice

Status: proposed by the architect for review · 2026-11-17 · builds on the approved design
(`docs/design/specs/2026-11-16-unidex-rust-design.md`, "the design"), the M0 gate report (`docs/build/M0-GATE.md`, "the
gate report"), the threat model (`docs/security/THREAT-MODEL.md`, revision 3) and decision records 0001 to 0014, with
the maintainer's answers of 2026-11-17 to gate questions 1 to 5. The task breakdown that builds this is
`docs/design/plans/2026-11-17-m1-vertical-slice.md` ("the task file").

How to read this file:

- Section numbers in parentheses ("design 5.2", "threat model 4.1", "gate question 4", "decision 0013") point to the
  sources. "Pn" is a threat-model proposal; all 31 scheduled for M1 are accepted (maintainer, 2026-11-17).
- **Fact** marks something the repository's records or the pinned sources show; the source is named. **Proposal** marks
  the architect's choice. **Not verified** marks an external API or behaviour this file relies on that no record in the
  repository shows; the owning task verifies it first and stops if it is wrong.
- Rust code blocks are contracts: names, fields, signatures and error types. Bodies are omitted. A builder may add
  private items and derive more traits; changing a public item named here needs the architect's (controller's)
  agreement, recorded in the pull request.
- "The app" is the GPUI application (`udx-app`); "the CLI" is the `unidex` binary (`udx-cli`).

## 1. Scope

### 1.1 What M1 delivers

From design 6, as amended by the maintainer's answers to gate questions 1 to 5:

1. **Two providers**: one hosted, one local, both streaming with tool calls (section 5 names them).
2. **The native engine loop** (design 5.2): turns with parallel tool calls, cancellation, resumable sessions, loop
   guards, retries, the prompt-based tool protocol for models without native tool calling (P6), and a stop when a local
   server silently cut the prompt (P59). Context trimming and summarising are M2 (design 6); in M1 a full window stops
   the turn with a clear reason.
3. **Core tools** (design 5.3, the subset whose dependencies exist in M1): `read_file`, `write_file`, `edit_file`,
   `glob`, `grep`, `shell`, `web_fetch`, `todo`.
4. **The shell sandbox** on all three platforms (design 5.11; gate question 1): macOS seatbelt; Linux user, mount,
   network and PID namespaces with Landlock and seccomp, made possible on default Ubuntu by an AppArmor profile the
   `.deb` installs; Windows AppContainer with a job object (D3, decided by the maintainer on 2026-11-17). Where the
   full sandbox cannot be applied, the shell is off (P7); where it applies but a networked command could not keep P43,
   network grants are off and commands run without network (section 3.6).
5. **Permissions and asks**: rules, sources and precedence (P47), modes, built-in asks (P3, P8, P14), ask cards in the
   app and typed asks in the CLI (P4, P49), rate limits (P58), and the definition of an unattended run (P41).
6. **The file guard, secrets, network and untrusted-content controls** of threat model 4.2 to 4.6 that M1 owns: P8 to
   P10, P48, P13 to P15, P54, P59, P21 to P23, P57, P24, and the cross-cutting P39 and P60.
7. **Spend caps** from M1 (P40): a shared ledger with per-call and per-day caps.
8. **A session log**: one append-only, versioned JSONL file per session (design 5.2), redacted, owner-only.
9. **A real GPUI chat window** and **`unidex chat`**, plus `unidex run`, `unidex config`, `unidex doctor`,
   `unidex trust` and `unidex key` (section 3.12).
10. **The chat's prepared-message cache with eviction** (gate question 4; decision 0008) and **a persistent shaped-text
    cache** (gate question 4), after a measurement that isolates the slow-frame cause.
11. **The automation socket in test builds**, end-to-end UI tests on three CI platforms with Wayland as a hard check,
    and the release check on every nightly artifact (design 10.3; decision 0013; P37).
12. **The first evaluation run** (design 10.2) with a harness and a first task set.
13. **Unsigned nightly builds** of the real app on three platforms (gate question 5: builds stay unsigned through M1;
    signing is decided at M4).
14. **The first task of M1** (gate report, section g): the hardening of the privacy gate and the path guard.

### 1.2 What M1 does not deliver

Each item cites where the design or the maintainer put it.

- Other providers, model profiles beyond one default per provider, context trimming and summarising, sub-agents, skills,
  the MCP client (design 6, M2). The `sub_agent`, `skill` and MCP tools are absent.
- The vault, memory, recall, context injection from memory, `unidex import`, `unidex serve` (design 6, M3). The `memory`
  tool is absent. Until the vault exists, the app keeps its private state under its data folder (D11).
- The app shell (rail, command palette, settings screens), Sessions and History, the terminal, diff pane, tray, themes
  beyond one light and one dark, auto-update, links handling beyond what P54 needs (design 6, M4).
- ACP add-on engines (design 6, M5); routines, duties, notifications and the `notify` tool, the scheduler and `run-due`,
  routine declarations (design 6, M6; P41 beyond its definition of an unattended run).
- The embedded browser (design 6, M7). Nothing in M1 may launch Chromium or CEF, in code, tests or CI.
- Signing, notarization and signed updates (gate question 5). The hash-checked fetches decision 0014 requires "before
  the first signed build, and in any case before any release" therefore stay owed, not built, in M1.
- A file-tool delete. No M1 tool deletes; P3's undo copy covers overwrites and edits (D14).

### 1.3 Exit criteria of the M1 gate (design 10.4)

The gate passes when every box is ticked; a miss blocks it (design 9, 10.4).

- [ ] **CI green on three platforms**: the eight M0 checks, plus the checks M1 adds (`privacy (base)`,
      `sandbox-linux (restricted)`, `sandbox-linux (profile)`, `ui (macos)`, `ui (linux-x11)`, `ui (linux-wayland)`,
      `ui (windows)`, `perf-checks`, `perf-checks (size)`), on the gate's commit; and `governed-paths (flag)`, which
      flags governed changes for review and fails only on its own error (D19).
- [ ] **Performance targets** as defined at the gate (gate question 3, adopted from the baseline's "Comparison"),
      measured on the reference machine, an Apple silicon desktop with 16 GB and a 100 Hz external display (a 120 Hz
      laptop leaves 8.3 ms per frame; the gate report says so beside every frame figure):
  - [ ] Start-up: spawn to the first presented frame showing the home screen's navigation (in M1 the chat window with
        its session list and composer), median of ten warm launches, under 400 ms; the first launch after a build or
        reboot reported separately with no pass mark.
  - [ ] Idle: no periodic wake-ups from our code (an internal counter reads zero over 60 s), and at most 60 wake-ups and
        0.1 CPU-seconds per minute across all the app's processes (placeholders until the floor of an empty native
        window is measured; T34 measures it with the cookbook's `hello_window`, on the development Mac with the
        maintainer's approval).
  - [ ] Idle memory: summed physical footprint of all the app's processes 90 s after the home screen is up (30 s to
        settle, 60 s idle), under 150 MB; and the same after scrolling the whole 5,000-message thread (decision 0008
        found the trial at the limit then).
  - [ ] Scrolling the 5,000-message thread (the baseline's fixture): at 2,000 px/s for 10 s and 10,000 px/s for 5 s,
        from five start points (5, 25, 45, 65 and 85 %), two launches: at most 1 frame interval in 1,000 over 1.5 times
        the refresh interval and none over twice it, from presented frames (gate question 4).
  - [ ] First streamed token: at most 50 ms from the engine reading the provider's bytes to the first presented frame
        containing the text, in a new thread and in the 5,000-message thread, with the first delta 200 ms and 2 s after
        the prompt, at least five turns each, using the scripted mock provider.
  - [ ] Opening the 5,000-message thread: under 100 ms from the action to the first frame showing it (new target).
  - [ ] Download size: the compressed macOS arm64 disk image, without the browser, under 40 MB (40,000,000 bytes, the
        unit decision 0014 uses); checked on every pull request by a proxy (section 3.13) and on the nightly's real
        `.dmg`.
- [ ] **Security review** against the 31 accepted proposals: every row of section 6 has its tests passing in CI on the
      platforms named, and the threat model is updated (threat model 9: each control marked built or tested, each
      proposal moved into design 5.11).
- [ ] **The M1 acceptance rows** (`docs/acceptance/CHECKLIST.md`): C1 and C3, both automated (section 7).
- [ ] **The first evaluation run** (design 10.2): the M1 task set run against one hosted model and one small local
      model, and against the scripted mock provider in CI; results recorded. Prerequisites the maintainer approves at
      the gate, because they spend money or install software: a hosted API key with a spend cap, and a local model
      server with one small model on the development Mac.
- [ ] **Unsigned nightly packages** of the real app built on three platforms from the gate's commit, each passing the
      release check (P37).
- [ ] **The maintainer's hands-on check on macOS** (design 10.4). It cannot be assumed: the gate report lists it as owed
      until the maintainer reports it.

## 2. Crate map

### 2.1 Crates and their single responsibility

Names follow design 5. "New" crates are created by the contracts tasks (T2a writes every manifest with these arrows,
T2b the stubs; task numbers refer to the plan file); the dependency arrows are
the only ones allowed (no cycles; normal dependencies only, dev-dependencies listed separately).

| Crate | M1 status | Responsibility | Workspace dependencies (normal) |
|---|---|---|---|
| `udx-core` | changed | Shared types and the low-level guards every crate uses: paths (the single choke point for user folders), state paths, ids, config, attendance, the shared plain types of section 3.1.0 (model, policy, content and workspace vocabulary), the `Engine` trait and its events, the session-log record types and their error, the `Sandbox` trait, its profile types and the process registry, secrets and redaction, the file guard, address classification, display escaping, the opener | none |
| `udx-providers` | new | The `Provider` trait's implementations, the HTTP transport and its fixtures, endpoint and key binding, model catalog, prices, role routing | `udx-core` |
| `udx-tools` | new | The `Tool` trait and the eight M1 tools; `web_fetch`'s network policy | `udx-core` |
| `udx-sandbox` | new | The per-platform `Sandbox` backends, process groups, limits, the availability probe; the Linux sandbox helper binary | `udx-core` |
| `udx-engine` | new | The native engine: turn loop, permissions, asks, context assembly and labelling, exposure, workspace trust, egress after exposure, spend ledger, session-log writer and reader, prompt-based tool protocol | `udx-core`, `udx-providers`, `udx-tools`, `udx-sandbox` |
| `udx-ui-model` | new | Screen state and actions for the chat window, with no GPUI types: thread view-model, markdown model, ask cards, notices, the prepared-message cache policy | `udx-core` |
| `udx-app` | new | The GPUI application binary `unidex-rust-preview`: window, theme, fonts, the Tokio-to-GPUI bridge, the chat window, message-body rendering, the shaped-text cache, the element registry; the automation socket and perf instrumentation in test and perf builds only | `udx-core`, `udx-ui-model`, `udx-engine`, `udx-providers`, `udx-tools`, `udx-sandbox` |
| `udx-cli` | changed | The `unidex` binary: `chat`, `run`, `config`, `doctor`, `trust`, `key`, `--version` | `udx-core`, `udx-engine`, `udx-providers`, `udx-tools`, `udx-sandbox` |
| `udx-evals` | new | The evaluation harness and the M1 task set (design 10.2) | `udx-core`, `udx-engine`, `udx-providers` (feature `testing`), `udx-tools`, `udx-sandbox` |
| `udx-test-support` | changed | Test helpers that depend on no workspace crate: a test-root folder, a recording HTTP stand-in, a temporary home with planted files, canary scanning | none |
| `udx-cookbook` | unchanged | GPUI examples (decision 0007) | none |
| `xtask` | changed | Privacy gate (hardened), hooks, the release check (P37), fixture conversion, core benchmarks | none |

`udx-store`, `udx-mcp`, `udx-acp`, `udx-term`, `udx-browser`, `udx-scheduler` and `udx-persona` are not created in M1.

Layering check against design 5's order (crates above depend only on crates below): `udx-sandbox` sits above `udx-tools`
in design 5's list, so `udx-tools` must not depend on `udx-sandbox`. That is why the `Sandbox` trait and its types live
in `udx-core` and the shell tool receives an `Arc<dyn Sandbox>` through its context (section 3.4).

**The layering rule (proposal).** A type that appears in a public signature of more than one crate, or in an event or a
session record, lives in the lowest crate every user of it can see, which in M1 is always `udx-core` (section 3.1.0).
The logic that decides with those types stays in the crate that owns it: `PermissionEngine`, `EgressState`,
`TrustStore` and `SpendLedger` in `udx-engine`, `Provider` implementations in `udx-providers`, `ProcessSupervisor` in
`udx-sandbox`. A separate `udx-types` crate was considered and not chosen: it would sit below `udx-core` with the same
dependencies (`serde`, `url`, `http`, `uuid`), add a crate and a second home for the session-log format, and buy no
layering `udx-core` does not already give. The contracts task's done-criterion (T2b) is that the stubs compile with
exactly the arrows of the table above and the drawing below, and no others.

```
udx-app ──┬──> udx-ui-model ──> udx-core
          ├──> udx-engine ──┬──> udx-providers ──> udx-core
udx-cli ──┤                 ├──> udx-tools ──────> udx-core
udx-evals ┘                 └──> udx-sandbox ────> udx-core
```

### 2.2 The test-support cycle and per-crate test helpers

**Fact** (`crates/udx-core/Cargo.toml`, `crates/udx-test-support/Cargo.toml`): `udx-core` dev-depends on
`udx-test-support`, which depends on `udx-core`. The gate report warns that one test-support crate depending on the
crates it helps test is a dev-dependency cycle (gate report, section g, "Architecture notes"). It also means a unit test
inside `udx-core` sees two different compilations of `udx-core`.

**Proposal** (built in T1b):

- `udx-test-support` depends on no workspace crate. It keeps only helpers that need no workspace type (`TestRoot`,
  `HttpStandIn`, `TempHome`, `canary`; section 3.14).
- `TestEnv` (today in `udx-test-support`) moves to `udx_core::paths::testing`, behind `udx-core`'s cargo feature
  `testing`.
- Every crate that has helpers others need exposes them as `pub mod testing` behind its own `testing` feature:
  `udx-core` (test env, memory secret store, fixed clock, fixed ids), `udx-providers` (scripted mock provider, fixture
  transport), `udx-sandbox` (a sandbox that reports itself unavailable, and a fake supervisor that records
  registrations), `udx-tools` (stub resolver), `udx-engine` (a turn harness that collects events). A dependent crate
  enables it only under `[dev-dependencies]`, and a test that needs another crate's real code lives in the crate above
  both: tests that need the engine and the real tools are in `udx-engine` (T23), never in `udx-tools`, so no
  dev-dependency cycle comes back.
- A `testing` feature never adds a capability to a release build. Each one defines a marker string
  (`UNIDEX_TESTING_MARKER_<crate>`) in a `#[used]` static so the release check (P37) fails if any reaches a packaged
  binary (section 3.15 says how the check proves the markers survive a release build).
- `udx-core/testing` also makes `AppPaths::resolve` refuse the operating system's folders entirely, so a test binary
  started outside cargo (which has neither `UNIDEX_TEST_ROOT` nor `CARGO_MANIFEST_DIR`) fails closed instead of reaching
  a real user's folders (gate report, section g, "the test environment helper required in tests").
- Feature unification (decision 0013 warns about it): `udx-evals` depends on `udx-providers/testing` as a normal
  dependency, so a workspace-wide build would carry the mock provider into every binary. Release and nightly builds
  therefore always name packages and binaries
  (`cargo build --release -p udx-app --bin unidex-rust-preview -p udx-cli --bin unidex`), never `--workspace`; the
  release check is the backstop.

### 2.3 External dependencies

Only crates.io; versions are pinned through the committed lockfile, and `[workspace.dependencies]` names each crate once
(T2a adds them all, with every feature and every test-only crate listed below, so later tasks do not edit the root
manifest, a member's dependency tables or the lockfile). Already in the lockfile through GPUI (**fact**,
`cargo metadata` on this commit): `tokio` 1.54.1, `serde` 1.0.229, `serde_json` 1.0.154, `uuid` 1.29.3 (with the `v7`
feature available), `sha2` 0.10.9, `base64` 0.22.1, `idna` 1.1.0, `url` 2.5.8, `icu_normalizer` 2.4.0, `libc` 0.2.191,
`rustix` 1.1.6, `windows-sys` 0.61.2, `objc2-security` 0.3.2, `oo7` 0.6.0, `futures` 0.3.34, `bytes` 1.12.1, `http`
1.5.0, `async-trait` 0.1.92, `regex` 1.13.1, `globset` 0.4.21, `flate2` 1.1.11, `encoding_rs` 0.8.44, `toml_edit`,
`tempfile` 3.27.0, `thiserror` 2.0.21, `rand` 0.9.5, `x11rb` 0.13.2, `time` 0.3.55. Using one directly adds no crate to
the build, but it becomes a direct dependency, which makes its "unmaintained" notices fail `cargo deny` (decision 0007's
scope rule).

New crates proposed, each with what was checked here (crates.io metadata read with `cargo info` on 2026-11-17) and what
was not:

| Crate (version seen) | Used by, for | Licence (crates.io) | Passes `deny.toml`? | Not verified |
|---|---|---|---|---|
| `reqwest` 0.13.5 | `udx-providers`, `udx-tools`: HTTP client | MIT OR Apache-2.0 | yes by licence | Its default TLS is `rustls` with `aws-lc-rs` (metadata: `default-tls = [rustls]`, `rustls = [__rustls-aws-lc-rs, ...]`); M1 uses `default-features = false` with `rustls-no-provider`, `http2`, `stream`, and installs the `ring` provider (D5). That this combination builds and needs no system proxy code is for T2a to show |
| `rustls` 0.23.46 | the `ring` crypto provider (feature `ring`) | Apache-2.0 OR ISC OR MIT | yes | That `CryptoProvider::install_default` is the way to give reqwest the provider at 0.13.5 |
| `ring` 0.17.14 (transitive) | crypto for TLS | Apache-2.0 AND ISC | yes (both allowed) | Build on Windows runners (needs a C compiler; the image has MSVC) |
| `rustls-platform-verifier` 0.7.1 (transitive, via reqwest) | certificate checks against the OS trust store | MIT OR Apache-2.0 | yes | Its Linux path (`rustls-native-certs` 0.8.5, Apache-2.0 OR ISC OR MIT) on the runner |
| `hyper` 1.12.0, `hyper-util` 0.1.21, `hyper-rustls` 0.27.10, `tokio-rustls` 0.26.6, `h2` 0.4.20, `rustls-webpki`, `untrusted` 0.9.0, `security-framework` 3.7.0 (macOS) (transitive) | reqwest's stack | MIT; MIT; Apache-2.0 OR ISC OR MIT; MIT OR Apache-2.0; MIT; ISC; ISC; MIT OR Apache-2.0 | yes | The full transitive list: T2a runs `cargo deny --locked check` and lists every crate added |
| `tokio-util` 0.7.20 | `CancellationToken` (also pulled by reqwest's `stream`) | MIT | yes | |
| `pulldown-cmark` 0.13.6 | `udx-ui-model`: markdown (decision 0008 measured it) | MIT | yes | |
| `rusqlite` 0.43.0 with `bundled` (and `libsqlite3-sys` 0.40.0) | `udx-engine`: the spend ledger (P40 asks for SQLite) | MIT (both) | yes | That the bundled SQLite source (public domain) raises nothing in `cargo deny` (it checks the crate's declared licence, MIT); `rust-version` 1.89 is below the pinned 1.100.0 |
| `ignore` 0.4.34 | `udx-tools`: `glob`/`grep` walking; `udx-engine`: git-ignored paths for P13 | Unlicense OR MIT | yes (MIT; `globset`, already used, has the same expression) | |
| `landlock` 0.4.7 | `udx-sandbox` (Linux) | MIT OR Apache-2.0 | yes | Its ABI handling on the runner's kernel |
| `seccompiler` 0.5.0 | `udx-sandbox` (Linux): the seccomp filter | Apache-2.0 OR BSD-3-Clause | yes | Its maintenance status (no `rust-version` published) |
| `proptest` 1.12.0 (dev only) | property tests (design 10.1), fuzzing P6 | MIT OR Apache-2.0 | yes (`include-dev = true` checks it) | |
| `rcgen` (version not read: no network was used for this revision) | `udx-test-support` (dev side of every crate that uses `HttpStandIn::start_tls`): a test certificate authority and leaf made at run time, since committed key material would trip the deny list and the secret scan; `default-features = false` with its `ring` and `pem` features | MIT OR Apache-2.0 (as published; not read here) | expected yes | Its current version, licence and that its `ring` feature builds with the `ring` version the TLS stack resolves: T2a reads `cargo info` first |
| `tokio-rustls` 0.26.6 (transitive via reqwest; direct in `udx-test-support`) | the TLS side of `HttpStandIn` | MIT OR Apache-2.0 | yes | That its server side needs no feature beyond what reqwest already turns on |

Features and test-only crates T2a turns on (each is a stop for a later task if missing):

- `rustix` 1.1.6: `fs`, `process`, `pty` and `termios` (fact: all four are features of 1.1.6 in the registry source),
  for `openat2`, process groups, the pseudo-terminal tests of T3, T11 and T24, and `Tty::read_secret`'s echo-off.
- `windows-sys` 0.61.2: the Win32 features for the console (`Win32_System_Console`), credentials
  (`Win32_Security_Credentials`), security and access lists (`Win32_Security`, `Win32_Security_Authorization`),
  AppContainers (`Win32_Security_Isolation`), job objects (`Win32_System_JobObjects`), processes and threads
  (`Win32_System_Threading`), named pipes (`Win32_System_Pipes`), files (`Win32_Storage_FileSystem`), error reporting
  (`Win32_System_ErrorReporting`) and the native file API (`Wdk_Storage_FileSystem`); T2a confirms each name against the
  crate's feature list.
- `uuid` 1.29.3: `v7` and `serde` (fact: `v7` is a feature of 1.29.3 in the registry source). `serde`: `derive`.
  `tokio`: `rt-multi-thread`, `macros`, `io-util`, `net`, `process`, `sync`, `time`, `fs`. `reqwest`: as D5.
  `rusqlite`: `bundled`. `landlock` and `seccompiler`: Linux targets only; `objc2-security`: macOS only; `oo7`: Linux
  only, with its Tokio feature.
- Test-only: `assert_cmd` 2.2.2 and `predicates` 3.1.4 (fact: both already in the lockfile, `assert_cmd` already a
  dev-dependency of `udx-cli` and `xtask`), `proptest`, `tempfile`, `rcgen` and `tokio-rustls` through
  `udx-test-support`.

Deliberately not added:

- **`chrono`**: not a direct dependency (gate report, section g; decision 0007: it carries RUSTSEC-2026-0543, which
  would fail `cargo deny` as soon as the workspace used it directly). Timestamps are integer milliseconds since the Unix
  epoch, UTC (`u64`), produced from `std::time::SystemTime`; the spend ledger's "day" is the UTC day (D9). No date
  formatting is needed in M1's core; the app formats for display with `time` 0.3.55 if needed.
- **`keyring`** 4.2.0 (MIT OR Apache-2.0): the OS secret stores are reached through crates already in the tree
  (`objc2-security` on macOS, `windows-sys` Credential Manager on Windows, `oo7` on Linux), so no new crate (D6). Not
  verified: that `objc2-security` 0.3.2's generated `SecItem` bindings cover add, copy-matching and delete for generic
  passwords; T6 verifies and falls back to `security-framework` 3.7.0 (already transitive through the TLS verifier) if
  not.
- **`tracing-subscriber`**: no global subscriber or logger is installed for third-party crates, so their trace output is
  never captured (P57; D13). Our own diagnostics go through `udx_core::diag` (section 3.1.11).
- **`aws-lc-rs`/`aws-lc-sys`**: `aws-lc-sys` 0.47.0's licence expression is satisfiable from the allow list, but it adds
  a large C build; `ring` is used instead (D5).
- **`unicode-normalization`**, **`unicode-script`**, **`trash`**, **`nix`**, **`shell-words`**, **`html5ever`**: NFC
  comes from `icu_normalizer` (already present); hosts with any non-ASCII character are always shown in their ASCII form
  beside the Unicode form, which covers "punycode when scripts mix" without script data (D15); no M1 tool deletes (D14);
  `rustix` and `libc` cover the Unix calls; command parsing and HTML-to-text are small, our own and tested.

## 3. Interfaces

Signatures are contracts (see "How to read"). Common conventions: errors are `thiserror` enums; every public async trait
uses `#[async_trait::async_trait]` so it stays usable as `dyn`; timestamps are `u64` milliseconds since the Unix epoch;
byte sizes are `u64`; money is `u64` micro-dollars (`MicroUsd`).

### 3.1 `udx-core`

#### 3.1.0 Shared plain types (T2a, fully implemented: they are data)

These are the types of the layering rule (section 2.1). Each has one home, named here; other crates use them through
that path (a crate may re-export one for convenience, never redefine it). Section 4.1 gives their serde shape.

```rust
// udx_core::model: provider-neutral names, money and tool descriptions
pub struct ProviderId(pub String);
pub struct ModelId(pub String);
pub type MicroUsd = u64;                        // integer micro-dollars (D9); never floating point
pub struct ToolSpec { pub name: String, pub description: String, pub input_schema: serde_json::Value }
pub enum ToolCalling { Native, Prompt, None }   // Prompt: the engine's protocol (P6); None: chat only. The status bar
                                                // shows which one the current model uses (design 5.2)

// udx_core::policy: the vocabulary of permissions, trust, egress and spend. The deciding logic is in udx-engine.
pub enum PermissionMode { ReadOnly, Ask, Auto }
pub enum Decision { Allow, Ask, Deny }
pub enum DecidedBy { Rule(RuleRef), User, UnattendedDefault, Mode(PermissionMode), BuiltinDeny }
pub struct RuleRef { pub id: String, pub source: RuleSource }
pub enum RuleSource { BuiltinDeny, BuiltinAsk(BuiltinAsk), UserSettings, WorkspaceNarrowing, SessionAnswer }
pub enum BuiltinAsk {
    GitPush, GitHistoryRewrite, RecursiveDelete, PackagePublish, Sudo, GhWrite, CloudCliWrite,   // P3
    GitAliasOrConfig,                                                                           // P3 (section 3.7)
    ProtectedPath,                                                                              // P8
    EgressAfterExposure, VerbatimLinkCap,                                                       // P14
    ExposedImageLoad,                                                                           // P15
    OutsideRead,                                                                                // P60
    InstructionFilesChanged,                                                                    // P13
}
/// What the engine approved, handed to Tool::run (section 3.4).
pub struct DecisionRef { pub turn: TurnId, pub call: CallId, pub decision: Decision, pub by: DecidedBy }
pub enum TrustState { Untrusted, Trusted { instruction_hash: Option<String>, since_ms: u64 } }
pub enum CapScope { Turn, Session }
pub enum SpendRefusal { PerCall { worst_case: MicroUsd, cap: MicroUsd }, PerDay { remaining: MicroUsd, worst_case: MicroUsd },
                        NoPrice { model: ModelId }, UnrecognisedLoopback { endpoint: String } }
pub enum EgressRequest {
    Fetch { url: url::Url, method: http::Method, has_body: bool, typed_by_user: bool },
    Redirect { from: url::Url, to: url::Url },
    ShellNetwork { command: String },
    ImageLoad { url: url::Url, from_exposed_turn: bool },   // P15: a user's "load image" action (section 3.13)
}
pub enum EgressVerdict {
    Allow,
    Ask { reason: EgressAskReason, composed: Vec<std::ops::Range<usize>> },  // byte ranges of the URL's display form
    StopTurn(CapScope),                                                      // that the model composed (P4; section 3.9)
    Deny(EgressDenyReason),
}
pub enum EgressAskReason { ComposedSinceExposure, HasBody, NewOrigin, PerOriginCap, ShellNetworkWhileExposed, ExposedImageLoad }
pub enum EgressDenyReason { UnattendedOutsideDeclaredPrefix /* M1: unattended runs declare no prefix */ }

// udx_core::content: where content came from, and what tools emit
pub enum Origin {
    User, App,
    File { path: PathBuf, outside_workspace: bool, third_party: bool, untrusted_workspace: bool },
    InstructionFile { path: PathBuf, hash: String },
    Web { url: url::Url },
    Shell { command: String, networked: bool, untrusted_workspace: bool },
}
pub enum OutStream { Stdout, Stderr }
pub struct TodoItem { pub id: u32, pub text: String, pub status: TodoStatus }
pub enum TodoStatus { Pending, InProgress, Done }
pub enum TodoChange { Add { text: String }, SetStatus { id: u32, status: TodoStatus }, List }

// udx_core::workspace: the workspace as every crate sees it
pub struct WorkspaceInfo {
    pub root: PathBuf,                       // canonical
    pub key: WorkspaceKey,
    pub trust: TrustState,
    pub ignore: Arc<dyn IgnoreMatcher>,      // git-ignored paths (P13)
    pub submodule_roots: Vec<PathBuf>,
    pub nested_repo_roots: Vec<PathBuf>,
}
/// udx-engine implements this with the `ignore` crate, so udx-core does not depend on it.
pub trait IgnoreMatcher: Send + Sync { fn is_ignored(&self, path: &Path, is_dir: bool) -> bool; }
pub struct WorkspaceRef { pub key: WorkspaceKey, pub path: PathBuf }
/// sha-256 over the canonical path, the device and the inode (file id on Windows), first 16 bytes as hex (D24).
pub fn workspace_key(root: &Path) -> Result<WorkspaceKey, WorkspaceError>;
```

Other public types the signatures below name, with their homes (each defined in its section): `udx_core::config::KeyPath`
(3.1.3), `udx_core::diag::Level` (3.1.11), `udx_core::fsguard::{DenyEntry, UndoRef}` (3.1.8), `udx_core::net::HostPort`
(3.1.9), `udx_core::sandbox::{Platform, KillHandle, ProcessRegistry}` (3.1.5), `udx_core::events::{LogError, *Summary}`
(4.2, 4.3), `udx_providers::{ProviderSet, RouteError}` and `testing::RequestMatcher` (3.2),
`udx_engine::protocol::{ParsedCall, ProtocolError}` (3.11), `udx_engine::context::ContextError`,
`udx_engine::trust::TrustError` (3.9), `udx_engine::spend::SpendError` (3.10), `udx_evals::{SuiteReport, EvalError}`
(3.15), `udx_test_support::{StandInRoute, RecordedRequest, TestCert}` (3.14), `udx_ui_model::{WorkspaceVm,
SessionSummary, ComposerVm, Banner, ToolRowVm}` (3.13), `udx_app::{Theme, ShapeKey, ElementRole}` (3.13).

#### 3.1.1 Paths, state paths and the test-root guard (T1b, T3)

`AppPaths` keeps its M0 shape (`crates/udx-core/src/paths.rs`). `Env` gains one provided method, `var_os` (default:
`var` converted), so a non-UTF-8 `PATH` survives. T1b adds the guard functions and the user-folder accessors; T3 adds
`StatePaths`.

```rust
// udx_core::paths (existing items unchanged unless listed)
pub trait Env { fn var(&self, key: &str) -> Option<String>; fn var_os(&self, key: &str) -> Option<OsString> { /* default */ } }
pub enum PathError {
    NoHome,
    OutsideTestRoot { dir: PathBuf, root: PathBuf },
    CargoWithoutTestRoot,
    // added by T1b
    NotAbsolute { var: &'static str, value: PathBuf },   // replaces the NoHome misuse at paths.rs:73, :77, :96
    Blank { var: &'static str },                          // a value that is only whitespace
    TestBuildWithoutTestRoot,                             // `testing` feature on and no test root: never OS folders
}

/// Every user-folder path in the workspace comes from here (gate report, section g). The choke-point test
/// (crates/udx-core/tests/choke_point.rs, T1b) scans every product crate's sources, that is everything under `crates/`
/// except `crates/udx-core/src/paths.rs` and `udx-cookbook`, and fails on `directories::`, `dirs::`, `home_dir`,
/// `temp_dir()`, and a string literal naming HOME, USERPROFILE, APPDATA, LOCALAPPDATA, TMPDIR, TEMP, TMP, USER, LOGNAME,
/// SHELL or XDG_* passed to any environment read. `xtask/` is excluded by name, with the reason in the test: it is the
/// build tool, never shipped, and its own tests move onto the test root in T1a. The test holds a named allow list of
/// (file, pattern) pairs; it is empty when T1b merges and grows only with the controller's agreement.
impl AppPaths {
    pub fn resolve(env: &dyn Env) -> Result<AppPaths, PathError>;
}
/// The user's home folder: the OS's answer normally; `<test root>/home` when a test root is set (created by TempHome or
/// TestEnv, refused by check_test_path if it would lie outside the root). GuardConfig::home and P48 come from here.
pub fn home_dir(env: &dyn Env) -> Result<PathBuf, PathError>;
/// The few user variables other modules need, read only here: the child environment's allow list (section 3.1.6), the
/// login shell, and the Linux helper's runtime folder to hide.
pub enum UserVar { Home, User, Logname, Shell, Lang, Tz, Term, XdgRuntimeDir, UserProfile, SystemRoot, Windir, ComSpec, PathExt, Path }
pub fn user_var(env: &dyn Env, var: UserVar) -> Option<OsString>;

/// The test root of this process, if any (UNIDEX_TEST_ROOT, resolved). Not cached: every call reads `env`, so each
/// TestEnv's own root is honoured (a process-wide cache would make a later test's paths look outside the root).
pub fn test_root(env: &dyn Env) -> Result<Option<PathBuf>, PathError>;

/// Fails when a test root is set and `path` (resolved as `resolve_path` does) lies outside it. Called by the file
/// guard on every open and create, by `StatePaths`, and by every module that writes (design 10.1: "a guard fails any
/// test that resolves a path outside its temp folder").
pub fn check_test_path(env: &dyn Env, path: &Path) -> Result<(), PathError>;

/// Fails when a test root is set, unless the opt-in variable names this kind. Called before any change to OS-level state
/// outside the app's folders. The one opt-in variable is UNIDEX_TEST_OS_SIDE_EFFECTS: a comma-separated list of kinds in
/// snake case, or `all`. Only CI runners set it (T6 adds it to the CI test jobs); the development Mac never does, and no
/// test, probe or check run there may change OS state outside its temporary folder (section 3.6, "Where tests run").
pub fn os_side_effects_allowed(env: &dyn Env, what: OsSideEffect) -> Result<(), PathError>;
pub enum OsSideEffect {
    SecretStore,          // a write to the OS secret store, or planting an item a test then looks for
    WakeUp,               // M6 scheduler
    WindowsAcl,           // an access entry on a file or folder for a sandbox SID
    AppContainerProfile,  // creating or deleting an AppContainer profile
    TrustStoreCert,       // adding a test certificate authority to an OS trust store (macOS CI only; section 3.6)
    SandboxEscapeMatrix,  // running escape payloads that, if the sandbox failed, would open windows or register launch items
    DiskImage,            // creating and attaching a disk image (macOS P51 test volume)
    PackageInstall,       // installing a built package (the `.deb` on the Linux runner)
}

#[cfg(feature = "testing")]
pub mod testing {
    /// Moved from udx-test-support. A fresh folder inside the process's test root, removed on drop.
    pub struct TestEnv { /* private */ }
    impl TestEnv { pub fn new() -> TestEnv; pub fn root(&self) -> &Path; pub fn with(self, key: &str, value: &str) -> TestEnv; }
    impl super::Env for TestEnv {}
}
```

```rust
// udx_core::state (T3). Where the app keeps its private state in M1, before the vault exists (D11).
pub struct StatePaths {
    pub config_file: PathBuf,        // <config_dir>/config.toml
    pub trust_file: PathBuf,         // <config_dir>/trust.toml
    pub sessions_dir: PathBuf,       // <data_dir>/sessions/<workspace-key>/
    pub undo_dir: PathBuf,           // <data_dir>/undo/<session-id>/
    pub spend_db: PathBuf,           // <data_dir>/spend.sqlite
    pub logs_dir: PathBuf,           // <data_dir>/logs/
    pub session_tmp_root: PathBuf,   // <cache_dir>/sessions/   (per-session temp folders for the sandbox)
    pub sandbox_dir: PathBuf,        // <data_dir>/sandbox/     (Windows access-entry records, section 3.6)
}
impl StatePaths {
    pub fn new(app: &AppPaths) -> StatePaths;
    pub fn sessions_for(&self, ws: &WorkspaceKey) -> PathBuf;
    pub fn session_log(&self, ws: &WorkspaceKey, id: &SessionId) -> PathBuf;   // <sessions_for>/<id>.jsonl
}
```

Every folder under `StatePaths` is created owner-only (0700, or an owner-only ACL on Windows) and every file holding
private state owner-only (0600), created without following links (P39), through `fsguard::private` (section 3.1.8).

#### 3.1.2 Ids, clock and attendance (T3)

```rust
// udx_core::ids
pub struct SessionId(Uuid);  pub struct TurnId(Uuid);  pub struct MessageId(Uuid);
pub struct CallId(String);   // the provider's tool-call id, kept verbatim; engine-made ids are "udx-<uuid>"
pub struct AskId(Uuid);      pub struct ReservationId(Uuid);  pub struct WorkspaceKey(String); // sha-256 hex, 16 bytes
pub trait IdGen: Send + Sync { fn uuid(&self) -> Uuid; }   // uuid v7 in production; sequential in `testing`
pub trait Clock: Send + Sync { fn now_ms(&self) -> u64; fn monotonic(&self) -> std::time::Instant; }

// udx_core::attendance (P41's definition of an unattended run; threat model 5.4)
pub enum Attendance {
    Attended(Surface),
    Unattended,
}
pub enum Surface { App, Terminal }
/// Attended(Terminal) only when standard input is a terminal AND the controlling terminal device can be opened
/// (/dev/tty on Unix, CONIN$ on Windows). Anything else, including `unidex run` with piped input, is Unattended.
/// The app is Attended(App). M6 adds routines, duties and `run-due`, all Unattended.
pub fn detect_cli_attendance() -> Attendance;
```

#### 3.1.3 Config (T3; the CLI verbs in T25)

The configuration is TOML, read with `toml_edit` so writes keep comments and order. In M1 the user file is
`StatePaths::config_file`; it moves into the vault's `.unidex/config.toml` in M3 (D11).

```rust
// udx_core::config
pub struct Config { /* typed view; built from layers */ }
pub enum Source {
    Default,
    UserFile { path: PathBuf, line: u32 },
    Env { var: String },                       // only UNIDEX_CONFIG_* overrides listed in config::ENV_KEYS; no
                                               // InteractiveOnly key is ever in that list (P22, P47, P24)
    Flag { name: String },                     // a command-line flag for this run only; a flag that widens (`--mode`)
                                               // needs Attended(Terminal) and a typed confirmation (section 3.12)
    WorkspaceNarrowing { path: PathBuf, line: u32 }, // <workspace>/.unidex/permissions.toml, narrowing only (P47)
}
pub struct KeyPath(Vec<String>);              // "providers.local.base_url" parsed into segments; quoted segments allowed
impl KeyPath { pub fn parse(s: &str) -> Result<KeyPath, ConfigError>; }
pub struct Row { pub key: KeyPath, pub value: toml_edit::Value, pub source: Source, pub sensitivity: Sensitivity }
pub enum Sensitivity { Plain, InteractiveOnly(InteractiveReason) }
pub enum InteractiveReason { ProviderHost /* P22 */, PermissionWidening /* P47 */, PrivateAddressAllow /* P24 */ }
pub struct UnknownKey { pub key: KeyPath, pub path: PathBuf, pub line: u32 }

impl Config {
    pub fn load(paths: &StatePaths, env: &dyn Env, flags: &[(KeyPath, toml_edit::Value)]) -> Result<Config, ConfigError>;
    pub fn rows(&self) -> Vec<Row>;                      // every section, every row, with its source (C1)
    pub fn unknown_keys(&self) -> &[UnknownKey];          // reported by doctor (C3); never an error at load
    pub fn plan_set(&self, key: &KeyPath, value: &str) -> Result<ConfigEdit, ConfigError>;
    pub fn plan_unset(&self, key: &KeyPath) -> Result<ConfigEdit, ConfigError>;
}
pub struct ConfigEdit { pub path: PathBuf /* the file the edit targets: StatePaths::config_file in M1 */, pub key: KeyPath,
                        pub before: String, pub after: String, pub sensitivity: Sensitivity, pub side_effects: Vec<SideEffect> }
pub enum SideEffect { ClearStoredKey { provider: ProviderId } /* P22: a host change clears the key */ }
/// Applies an edit atomically (temp file and rename, owner-only, through check_test_path on `edit.path`). `proof` exists
/// only when the edit's sensitivity is InteractiveOnly; it is minted by the CLI's typed confirmation (section 3.12) or the
/// app's settings action, never by a tool (no tool holds a ConfigWriter) and never in an unattended run.
pub fn apply(edit: ConfigEdit, proof: Option<InteractiveProof>, secrets: &dyn SecretStore, env: &dyn Env) -> Result<(), ConfigError>;
pub struct InteractiveProof { /* private; constructed only by udx_core::config::interactive::confirm */ }
pub enum ConfigError { Parse { path: PathBuf, line: u32, message: String }, UnknownKey(KeyPath), BadValue { key: KeyPath, reason: String },
                       NeedsInteractive(InteractiveReason), Io(std::io::Error), Guard(GuardError) }
```

Sections in M1 (each key's default, type and sensitivity are a table in `config.rs`, and the doctor's unknown-key check
reads that table):

| Section | Keys (examples) | Notes |
|---|---|---|
| `[providers.<id>]` | `kind` (`anthropic` or `ollama`), `base_url`, `key_source` (`keychain` or `env:<VAR>`), `free` (`recognised`, `marked`, `no`) | `base_url` is InteractiveOnly(ProviderHost) (P22); non-loopback must be `https` (P24) |
| `[providers.<id>.models.<model>]` | `price_input_usd_per_mtok`, `price_output_usd_per_mtok`, `price_cache_read_usd_per_mtok`, `price_cache_write_usd_per_mtok`, `context_window`, `tool_calling` (`native`, `prompt`, `none`) | decimal strings, parsed to `MicroUsd` |
| `[roles.main]` | `provider`, `model`, `fallback` (list of `{provider, model}`) | design 5.1; a fallback happens only between turns |
| `[spend]` | `per_call_usd`, `per_day_usd` | P40; defaults 0.50 and 5.00 (D22) |
| `[permissions]` | `mode` (`read_only`, `ask`, `auto`), `[[permissions.rules]]` | allow rules, and any `mode` change that widens (`read_only` to `ask` or `auto`, `ask` to `auto`), are InteractiveOnly(PermissionWidening) (P47; design 5 as qualified at gate question 2(d)); a narrowing change is Plain. The configured mode applies to trusted workspaces only; an untrusted workspace's sessions start in `ask` whatever it says (P60; D29) |
| `[egress]` | `verbatim_per_origin_per_turn`, `verbatim_per_turn`, `verbatim_per_session` | P14; numbers set from the measurement in T28 |
| `[net]` | `private_allow` (list of `host:port`) | InteractiveOnly(PrivateAddressAllow); never applied unattended (P24) |
| `[limits]` | `command_wall_s`, `command_memory_mb`, `command_processes_margin`, `output_cap_bytes`, `read_cap_bytes`, `free_space_floor_mb` | P51, P40 |

#### 3.1.4 The `Engine` trait and its events (T2a for the types; T19 and T23 implement the native engine)

Design 5.1 puts `Engine` beside `Provider` as a product-defining trait, and design 5 puts events in `udx-core`. The
trait lives here so M5's ACP engine can implement it without depending on the native engine. Event types are in section
4.

```rust
// udx_core::engine
#[async_trait::async_trait]
pub trait Engine: Send + Sync {
    fn kind(&self) -> EngineKind;
    async fn start_session(&self, opts: SessionOptions) -> Result<SessionHandle, EngineError>;
    async fn resume_session(&self, ws: &WorkspaceKey, id: &SessionId, opts: ResumeOptions) -> Result<SessionHandle, EngineError>;
}
pub enum EngineKind { Native, Acp { agent: String } }
pub struct SessionOptions {
    pub workspace: PathBuf, pub attendance: Attendance, pub role: Role,
    pub mode: Option<PermissionMode>,                      // policy types: section 3.1.0
    pub model_override: Option<(ProviderId, ModelId)>,
}
pub struct ResumeOptions { pub attendance: Attendance }
pub struct SessionHandle {
    pub id: SessionId,
    pub events: EventStream,             // lossless and ordered; consecutive text deltas may be coalesced
    pub commands: CommandSender,
}
pub struct EventStream { /* impl futures::Stream<Item = EventEnvelope> */ }
pub struct CommandSender { /* cloneable */ }
impl CommandSender { pub async fn send(&self, cmd: SessionCommand) -> Result<(), EngineError>; }
pub enum SessionCommand {
    UserMessage { text: String },
    Cancel,
    AnswerAsk { ask: AskId, answer: AskAnswer },
    /// Narrowing is always accepted. Widening beyond the session's starting mode is refused (EngineError::Refused)
    /// unless the session is Attended; the CLI asks for a typed confirmation before sending it (P47).
    SetMode(PermissionMode),
    Grant(SessionGrant),                 // per session, attended only; ends with the session (P50)
    Revoke(SessionGrant),                // asks about processes holding the grant (P50)
    /// P47: "from the app's own screens". Accepted only when the session's attendance is Attended(App); refused for
    /// Attended(Terminal) and Unattended. The CLI never sends it.
    TurnOffBuiltinAsk { name: BuiltinAsk },
    /// P15: the user's action on a remote image placeholder. Runs through the egress gate (EgressRequest::ImageLoad, which
    /// asks when the image came from an exposed turn) and then the same fetch policy as web_fetch (section 3.5), with an
    /// image content-type allow list and a 5 MiB cap. Attended(App) only.
    LoadImage { message: MessageId, image: u32, url: String },
    Close,
}
pub enum SessionGrant {
    UseMyCredentials { vars: Vec<String> },   // P23
    ShellNetwork,                            // P1, per session; per-command network is a rule or an ask
    UnsandboxedShell,                        // P7: attended only, after a warning; refused when Unattended
}
pub enum Role { Main, Background, Reviewer, Embeddings }   // design 5.1; M1 uses Main only
/// LogError is udx_core::events::LogError (section 4.3), so this enum needs nothing from udx-engine.
pub enum EngineError { Workspace(WorkspaceError), Config(ConfigError), Provider(String), Log(LogError), Closed, Refused(String) }
```

#### 3.1.5 The `Sandbox` trait and profile types (T2a for the types; T10 to T12 implement)

```rust
// udx_core::sandbox
pub trait Sandbox: Send + Sync {
    /// Decided by the probe at start (and again on demand); never assumed. Returned owned, so a reprobe is visible.
    fn availability(&self) -> Availability;
    fn reprobe(&self) -> Availability;
    fn spawn(&self, cmd: CommandSpec, profile: &SandboxProfile) -> Result<Box<dyn SandboxedChild>, SandboxError>;
}
pub enum Availability {
    Full { backend: Backend, details: BackendDetails, network_grants: NetworkGrants },
    Unavailable { backend: Backend, reason: UnavailableReason, remedy: String },
}
/// Whether this backend can run a networked command with P43 intact (section 3.6). When Unavailable, a session network
/// grant, a rule's network and a networked ask are all refused with the reason; commands still run without network.
/// This is fail closed for the network grant only (P7, P43), and the app says so in the status bar and in doctor.
pub enum NetworkGrants { Available, Unavailable { reason: NetworkUnavailableReason, remedy: String } }
pub enum NetworkUnavailableReason { LandlockScopeMissing { abi: u32 } /* Linux below ABI 6 */, ProbeCheckFailed { check: ProbeCheck } }
pub enum Backend { MacosSeatbelt, LinuxNamespaces, WindowsAppContainer }
pub enum Platform { Macos, Linux, Windows }
impl Platform { pub fn current() -> Platform; }
pub struct BackendDetails { pub landlock_abi: Option<u32>, pub process_limit: ProcessLimitKind, pub memory_limit: bool,
                            pub reports_denied_writes: bool, pub probe: ProbeReport, pub notes: Vec<String> }
pub enum ProcessLimitKind { CgroupPids, RlimitUserCountPlusMargin, JobObject }
pub enum UnavailableReason {
    UserNamespacesRestricted,      // Linux: AppArmor restricts unprivileged user namespaces and no profile covers the helper
    LandlockMissing, SeccompFailed, HelperMissing, SeatbeltFailed, AppContainerFailed, JobObjectFailed,
    ProbeCheckFailed { check: ProbeCheck },  // a probe check that should have been denied was allowed
    ForcedByTestBuild,             // only with the `testing` feature (P7's test)
}
pub enum ProbeCheck { WriteOutsideWorkspace, LoopbackConnect, NameLookup, IpcEscape, IpcEscapeNetworked, SecretStoreReach,
                      ReadDeniedFile, ReadInTreeDenied, OtherProcessEnv }
/// What the probe saw for one check. `Denied` names the specific denial (the sandbox's error code or the missing
/// service), never just "failed". `NotApplicable` (the resource does not exist outside the sandbox on this machine, so
/// a denial cannot be shown) is reported as such and is never counted as a pass or a failure.
pub enum ProbeOutcome { Denied { how: String }, Allowed, NotApplicable { why: String }, Error(String) }
pub struct ProbeReport { pub checks: Vec<(ProbeCheck, ProbeOutcome)> }

pub struct SandboxProfile {
    pub workspace: PathBuf,             // canonical; read and write
    pub session_tmp: PathBuf,           // read and write; TMPDIR points here; a sized tmpfs on Linux (P51)
    pub read_allow: Vec<PathBuf>,       // P10: system folders and the reviewed toolchain list; never <home> whole
    pub read_deny: Vec<DenyEntry>,      // the deny list (threat model 4.2) as patterns, applied inside every granted
                                        // tree; per-backend mechanism in section 3.6
    pub write_deny: Vec<PathBuf>,       // P8 protected sub-paths and P48 persistence locations, resolved, whether or
                                        // not they exist; per-backend mechanism and the absent-path gap in section 3.6
    pub network: NetworkGrant,          // P1; Full only when Availability says NetworkGrants::Available
    pub credentials: Vec<String>,       // P23: variables restored by a "use my credentials" grant
    pub credential_sockets: Vec<PathBuf>, // P23: the agent sockets that grant restored (the path SSH_AUTH_SOCK names);
                                        // the profile allows connecting to exactly these, nothing else
    pub limits: Limits,                 // P51
}
pub enum NetworkGrant { None, Full }    // no host filtering (threat model 7)
pub struct Limits { pub wall: Duration, pub cpu: Duration, pub memory_bytes: u64 /* not enforced on macOS: residual */,
                    pub processes_margin: u32, pub output_cap: u64 }

pub struct CommandSpec {
    pub program: CommandProgram,
    pub cwd: PathBuf,                   // inside the workspace (checked by the shell tool through the file guard)
    pub env: ChildEnv,                  // built by udx_core::process_env (P23)
    pub stdin: Option<Vec<u8>>,         // a secret reaches a child only here or in env, never in argv (P23)
}
pub enum CommandProgram {
    /// P2: a rule-allowed command, no shell, no wildcard expansion. argv[0] is resolved against the child's PATH by the
    /// shell tool before spawn; relative and empty PATH entries are skipped, so a planted `./git` is never chosen.
    Direct { argv: Vec<OsString> },
    FixedShell { script: String },      // P2: asked commands only; /bin/sh with no startup files; on Windows cmd.exe /d /s /c
}
pub trait SandboxedChild: Send {
    fn pid(&self) -> u32;
    fn stdout(&mut self) -> Option<Box<dyn tokio::io::AsyncRead + Send + Unpin>>;
    fn stderr(&mut self) -> Option<Box<dyn tokio::io::AsyncRead + Send + Unpin>>;
    /// Waits on this child by its own id (pidfd, kqueue or the process handle), never waitpid(-1): the terminal's
    /// SIGCHLD handler arrives in M4 (design 5.7; gate report, section g).
    fn wait(&mut self) -> futures::future::BoxFuture<'_, Result<ExitInfo, SandboxError>>;
    /// A handle that kills the whole session-owned group (P50) and lists its live processes. It is separate from the
    /// child so cancellation and session end can use it while another task awaits `wait`.
    fn kill_handle(&self) -> KillHandle;
}
/// Implemented by each backend: PID namespace (Linux), job object (Windows), process group plus the marker sweep (macOS, D2).
pub trait GroupKiller: Send + Sync {
    fn kill_group(&self) -> Result<(), SandboxError>;
    fn live_processes(&self) -> Vec<ProcessInfo>;   // for the UI's list of live processes (P50)
}
#[derive(Clone)]
pub struct KillHandle(Arc<dyn GroupKiller>);
impl KillHandle {
    pub fn new(killer: Arc<dyn GroupKiller>) -> KillHandle;
    pub fn kill_group(&self) -> Result<(), SandboxError>;
    pub fn live_processes(&self) -> Vec<ProcessInfo>;
}
/// Every sandboxed group is registered here at spawn (P50). udx-sandbox's ProcessSupervisor implements it; the shell
/// tool reaches it through ToolContext::processes (section 3.4), so udx-tools needs nothing from udx-sandbox.
pub trait ProcessRegistry: Send + Sync {
    fn register(&self, session: &SessionId, call: &CallId, handle: KillHandle, info: ProcessInfo);
    fn unregister(&self, session: &SessionId, pid: u32);
}
pub struct ExitInfo {
    pub code: Option<i32>, pub signal: Option<i32>, pub timed_out: bool, pub limits_hit: Vec<LimitKind>,
    /// Protected writes the backend reported denying (P8). Only a backend with BackendDetails::reports_denied_writes
    /// fills it; the re-run offer exists only for an entry here (section 3.6).
    pub denied_writes: Vec<PathBuf>,
    /// Protected paths (write_deny entries) that did not exist before the command and exist after it, found by a
    /// before-and-after check on backends that cannot deny an absent path (Linux, Windows; section 3.6). Reported to
    /// the user on a card naming each file; never removed silently and never offered as a re-run.
    pub protected_created: Vec<PathBuf>,
}
pub enum LimitKind { Wall, Cpu, Memory, Processes, Output, FreeSpace }
pub struct ProcessInfo { pub pid: u32, pub command: String, pub started_ms: u64, pub has_network: bool, pub has_credentials: bool }
pub enum SandboxError { Unavailable(UnavailableReason), NetworkUnavailable(NetworkUnavailableReason), Spawn(std::io::Error),
                        Profile(String), Wait(std::io::Error), Kill(std::io::Error) }
```

What **"full sandbox applies"** means for P7, per backend. The probe runs each check inside a real sandboxed child: our
own binary (the app or the CLI) started with the hidden argument `--udx-sandbox-probe <check>`, so the probe's code is
ours and needs nothing installed. Any check `Allowed`, or any setup step failing, makes the backend `Unavailable`; a
networked check `Allowed` makes only `NetworkGrants` unavailable. The production probe plants nothing (no item in a
secret store, no file outside the app's folders); CI tests plant known items under the opt-in variable (section 3.1.1)
and assert they are not returned.

| Check | macOS seatbelt | Linux namespaces | Windows AppContainer |
|---|---|---|---|
| Profile applied | `sandbox-exec` with our profile returns and the child runs | user, mount, PID and IPC namespaces created, and a network namespace when the command has no network; `no_new_privs`; seccomp filter loaded; Landlock enforced at ABI 1 or higher, and at ABI 6 or higher with the abstract-socket and signal scopes for a networked command | AppContainer token created; job object assigned with kill-on-close, UI limits and no breakaway |
| Write outside the workspace denied | yes | yes | yes |
| Loopback connect and name lookup denied, no network (P1) | yes | yes (empty network namespace) | yes (no network capabilities; loopback blocked for AppContainers) |
| IPC escape denied, no network (P43, `IpcEscape`) | a `mach-lookup` of a denied service fails with the sandbox's denial, not "unknown service"; `lsopen` denied | the session bus, `$XDG_RUNTIME_DIR` and `/tmp/.X11-unix` are absent from the new root; an abstract socket the probe's parent binds in its own namespace is unreachable | COM activation of a test class fails with access denied |
| IPC escape denied, networked profile (P43, `IpcEscapeNetworked`) | a stand-in Unix socket the parent binds outside the workspace refuses the connect; `mach-lookup` of a denied service still fails | an abstract socket the parent binds in the shared host namespace refuses the connect (Landlock scope); a stand-in pathname socket outside the workspace is absent from the new root | a stand-in named pipe outside the container's access refuses the open; COM activation still fails |
| Secret store unreachable (P23) | `mach-lookup` of the security server's service fails with the sandbox's denial | no session bus path in the root and the abstract bus refused; `NotApplicable` when no bus exists outside either | `CredReadW` fails with access denied rather than not-found (not verified that the two differ; T12 checks first) |
| Planted file under a temp home unreadable (P10) | yes | yes | yes |
| Deny-listed file inside the workspace unreadable (`ReadInTreeDenied`) | yes (profile rule) | yes (masked) | yes (deny entry) |
| Another process's environment unreadable (P23, `OtherProcessEnv`) | `sysctl` `kern.procargs2` on a process started outside the sandbox fails | the PID namespace's own `/proc` shows no host process | `OpenProcess` on a process started outside the container fails |

How fail-closed is reported: `Availability::Unavailable` carries a reason and a remedy in plain words (for example
"Linux restricts user namespaces here. Install the `.deb`, which adds an AppArmor profile, or use a system that allows
them."). The shell tool is then absent from the model's tool list; the app's status bar and `unidex doctor` show the
reason and remedy; the session log records an `availability` record at session start (section 4.3). `NetworkGrants::
Unavailable` is shown the same way ("Commands run without network here: this kernel's Landlock (ABI 5) cannot keep a
networked command away from other programs' sockets."). A test-only switch forces `Unavailable(ForcedByTestBuild)`; it
exists only under the `testing` feature (P7's test; P37 checks it is gone).

#### 3.1.6 Process environment (T6)

```rust
// udx_core::process_env (P23; L9)
pub struct ChildEnv { vars: BTreeMap<OsString, OsString> }
/// Built from an allow list, never by removing names from the parent's environment: PATH (the login shell's, resolved
/// once at start, design 5.4), HOME, USER, LOGNAME, LANG, LC_*, TERM, TZ, SHELL=/bin/sh, TMPDIR (the session's temp
/// folder), and on Windows SystemRoot, windir, ComSpec, PATHEXT, TEMP, TMP. Never: agent sockets (SSH_AUTH_SOCK,
/// GPG_AGENT_INFO), *_TOKEN, *_KEY, *_SECRET, cloud CLI variables, loader variables (LD_*, DYLD_*), markers a parent
/// agent session left (CLAUDECODE, CODEX_*), UNIDEX_* internals, and every provider key variable.
/// `restore` names credential variables a "use my credentials" grant restores (P23); each comes from BaseEnv's sealed
/// credential map, never from the allow list, and a name not captured there is ignored.
pub fn scrubbed(base: &BaseEnv, session_tmp: &Path, restore: &[String]) -> ChildEnv;
/// Captured once at start, through paths::user_var and Env only (the choke point): the allow-listed values, the login
/// PATH (filled in later, see below), and a sealed map of the credential variables (agent sockets, *_TOKEN, cloud CLI
/// variables) that only `scrubbed(.., restore)` can read. Debug prints names, never values.
pub struct BaseEnv { /* allow-listed values; login PATH; sealed credential map */ }
pub fn capture_base_env(env: &dyn Env) -> BaseEnv;
/// Runs $SHELL -l -c once, scrubbed env, no tty, with a timeout. Never on the first-frame path (the 400 ms start-up
/// target): the app and the CLI start it on a background thread after start-up; the shell tool's first command waits
/// for it up to the timeout and then uses the inherited PATH minus relative and empty entries, saying so in the result.
#[cfg(unix)] pub fn resolve_login_path(timeout: Duration) -> Option<OsString>;
```

#### 3.1.7 Secrets and redaction (T6)

```rust
// udx_core::secret
pub struct Secret(/* zeroized on drop; Debug and Display print "[redacted]" */);
impl Secret { pub fn expose(&self) -> &str; pub fn from_string(s: String) -> Secret; }
pub struct SecretName { pub service: &'static str /* "unidex-rust-preview" */, pub account: String /* "provider:<id>" */ }

#[async_trait::async_trait]
pub trait SecretStore: Send + Sync {
    fn backend(&self) -> SecretBackend;
    async fn get(&self, name: &SecretName) -> Result<Option<Secret>, SecretError>;
    async fn set(&self, name: &SecretName, value: Secret) -> Result<(), SecretError>;
    async fn delete(&self, name: &SecretName) -> Result<(), SecretError>;
}
pub enum SecretBackend { MacosKeychain, LinuxSecretService, WindowsCredentialManager, Environment, Memory /* testing only */ }
/// Picks the OS store; falls back to the environment only where no OS store is reachable (design 5.10), and never
/// writes to the OS store when a test root is set unless CI set UNIDEX_TEST_OS_SECRET_STORE=1 (os_side_effects_allowed).
pub fn open_secret_store(env: &dyn Env) -> Box<dyn SecretStore>;
pub enum SecretError { Unavailable(String), Locked, NotFound, Backend(String) }

/// Redacts every stored secret, its base64 (standard and URL-safe, padded and not), percent-encoded and hex forms, and
/// token-shaped strings (provider key prefixes, `Authorization:` header values, bearer tokens, PEM private key blocks).
pub struct Redactor { /* built from the store's values at session start and when a key changes */ }
impl Redactor {
    pub fn new(secrets: &[&Secret]) -> Redactor;
    pub fn redact(&self, text: &str) -> Cow<'_, str>;      // replaces with "[redacted:<kind>]"
    pub fn redact_json(&self, value: &mut serde_json::Value);
    pub fn contains_secret(&self, text: &str) -> bool;      // P21's backstop scan of outbound arguments
}
/// P21 for streamed text (TextDelta and ToolOutputDelta on --json and the automation socket): holds back the tail that
/// could be the start of a secret in any of its encodings (the longest encoded form, less one byte) until more text
/// arrives or the stream ends, so a secret split across chunks is still redacted.
pub struct StreamRedactor { /* ... */ }
impl StreamRedactor { pub fn new(r: Arc<RwLock<Redactor>>) -> StreamRedactor; pub fn push(&mut self, chunk: &str) -> String; pub fn finish(&mut self) -> String; }
/// P57: called first in every binary's main. Linux: prctl(PR_SET_DUMPABLE, 0). All Unix: RLIMIT_CORE = 0. Windows:
/// no WER dump for this process (the mechanism is T6's to verify). Installs a panic hook that writes a redacted message.
pub fn harden_process();
```

#### 3.1.8 The file guard (T5)

The guard is the only way the file tools, the session log, the undo store, the config writer and the trust store touch
files. It implements design 5.11's "paths resolved before checks", threat model 4.2's deny list, P8, P9, P10's lists
(used by the sandbox profiles), P39 and P48.

```rust
// udx_core::fsguard
pub struct FileGuard { /* workspace root (canonical, plus device and inode), deny list, protected paths, persistence list, outside rules */ }
pub struct GuardConfig {
    pub workspace: PathBuf,
    pub outside_write_rules: Vec<PathBuf>,  // folders a user rule names (threat model 4.2: writes outside need one)
    pub home: PathBuf,                      // from udx_core::paths::home_dir (the choke point, section 3.1.1)
    pub app_state: Vec<PathBuf>,            // StatePaths roots and the config file: persistence, write-denied (P48)
}
pub enum Access { Read, Write, Create }
pub enum Verdict {
    Inside(GuardedPath),
    Outside(GuardedPath),                 // read: labelled and asks (P60); write/create: allowed only under an outside rule
    Protected(GuardedPath, Protected),    // P8: asks in every mode, denied unattended
    Denied(DenyReason),
}
pub enum Protected { GitDir, UnidexDir, InstructionFile, AgentConfig, McpJson, EditorTasks, Envrc, Husky, CiWorkflows,
                     HooksPath, WorktreeCommonDir, ModuleDir }
// InstructionFile covers, for writes, every agent instruction file a tool the user runs outside the sandbox may load:
// AGENTS.md, CLAUDE.md, .github/copilot-instructions.md, .cursorrules and the .cursor/rules folder, and similar files, at any
// depth; the table is in fsguard/protected.rs with a reason per entry. Only AGENTS.md is loaded by M1 (D16).
// AgentConfig covers agent project configuration (P44): .claude/, .codex/, .cursor/, and similar folders.
pub enum DenyReason {
    DenyList { entry: &'static str },     // credential and key files, matched as asked and as resolved
    Persistence { entry: &'static str },  // P48's fixed list, write-denied everywhere
    WindowsForm(WindowsForm),             // UNC, \\?\, \\.\, device names, streams, trailing dots and spaces (P9);
                                          // a short (8.3) component is expanded with GetLongPathNameW before any
                                          // comparison, and refused if it cannot be expanded
    LinkInPath, TooManyHops, NotUtf8OnWindows, OutsideNoRule, Race, TestRoot(PathError),
}
pub struct GuardedPath { /* canonical path, the components as asked, device and inode where they exist */ }

impl FileGuard {
    pub fn new(cfg: GuardConfig, env: &dyn Env) -> Result<FileGuard, GuardError>;
    /// Pure decision on a spelling: normalises (NFC, case-folded comparison for the deny list), resolves links once,
    /// checks both the spelling as asked and as resolved, and denied folders also by device and inode.
    pub fn check(&self, requested: &Path, access: Access) -> Verdict;
    /// P9: opens each component relative to its parent without following links (Linux openat2 with RESOLVE_BENEATH
    /// and RESOLVE_NO_SYMLINKS; macOS O_NOFOLLOW_ANY; Windows NtCreateFile relative to the parent handle with
    /// FILE_OPEN_REPARSE_POINT), then reads the final path back from the handle and checks it again.
    pub fn open_read(&self, path: &GuardedPath, cap: u64) -> Result<GuardedRead, GuardError>;
    /// Temp file and atomic rename keeping mode and extended attributes; a symlink's target is written (once it
    /// passes the guard), never the link replaced; a file with more than one hard link is replaced by rename and the
    /// result says the other names keep the old content; never creates a link or sets an executable bit (P8). Takes an
    /// undo copy first when the file exists (P3).
    pub fn write_atomic(&self, path: &GuardedPath, bytes: &[u8], undo: &UndoStore) -> Result<WriteOutcome, GuardError>;
    /// The resolved P8 and P48 paths for sandbox profiles (write_deny) and P10's reviewed read list (read_allow).
    pub fn sandbox_lists(&self) -> SandboxLists;
}
pub struct GuardedRead { pub bytes: Vec<u8>, pub truncated: bool, pub final_path: PathBuf }
pub struct WriteOutcome { pub created: bool, pub undo: Option<UndoRef>, pub other_hard_links: u32, pub wrote_through_symlink: bool }
pub struct SandboxLists { pub read_allow: Vec<PathBuf>, pub read_deny: Vec<DenyEntry>, pub write_deny: Vec<PathBuf> }
/// One deny-list entry (threat model 4.2), matched after NFC and lower-casing.
pub struct DenyEntry { pub pattern: &'static str, pub kind: DenyKind }
pub enum DenyKind { Folder /* ".ssh/" */, File /* ".netrc" */, Glob /* "*.pem", ".env.*", "id_*" */ }
pub struct UndoRef { pub id: u64, pub original: PathBuf, pub stored_ms: u64 }
pub enum GuardError { Denied(DenyReason), Io(std::io::Error), FreeSpaceFloor { free: u64, floor: u64 }, TooLarge { cap: u64 } }

/// P48: <home>, its parents and the root are refused as a workspace unless the user typed a confirmation; then the
/// untrusted rules apply (P13).
pub fn check_workspace_root(path: &Path, home: &Path, typed_confirmation: bool) -> Result<PathBuf, WorkspaceError>;
pub enum WorkspaceError { IsHome, IsParentOfHome, IsRoot, NotADirectory, Guard(GuardError) }

// udx_core::fsguard::undo (P3)
pub struct UndoStore { /* StatePaths::undo_dir for one session */ }
impl UndoStore { pub fn restore(&self, r: &UndoRef) -> Result<(), GuardError>; pub fn list(&self) -> Vec<UndoRef>; }

// udx_core::fsguard::private (P39)
// Both call check_test_path(env, path) first (section 3.1.1).
pub fn create_private_dir(path: &Path, env: &dyn Env) -> Result<(), GuardError>;            // 0700 / owner-only ACL, no link followed
pub fn create_private_file(path: &Path, env: &dyn Env) -> Result<std::fs::File, GuardError>; // 0600 / owner-only ACL, O_EXCL|O_NOFOLLOW

// udx_core::fsguard::space (P51)
pub fn free_space(path: &Path) -> std::io::Result<u64>;
```

Notes:

- The deny list is threat model 4.2's, as a table in `fsguard/deny.rs`, matched after NFC normalisation and lower-casing
  (`str::to_lowercase`; full Unicode case folding, which threat model 4.2 names, differs for a few characters such as
  the German sharp s; D15 accepts this as a residual for the threat model update, mitigated by the handle readback of
  P9, which compares the opened file's identity with denied folders by device and inode). It includes
  `.unidex/` (the workspace's narrowing file is read by the config loader through `open_read` with an explicit internal
  allowance, never by a tool) and the app's own state folders.
- P10's reviewed read list is a table in `fsguard/read_allow.rs`, one entry per line with a reason: system folders per
  platform and named toolchain folders and files (for example `~/.cargo/bin`, `~/.rustup`, `~/.gitconfig`). `<home>` is
  never granted whole. M1 measures which common tools fail without more entries (threat model 7) in T28's evaluation
  tasks and lists them in the gate report.
- The free-space floor: `write_atomic`, the session log and the undo store refuse a write that would leave less than
  `limits.free_space_floor_mb` free (P51).

#### 3.1.9 Network address classification (T4 builds; used by T7 and T22)

```rust
// udx_core::net (threat model 4.6)
pub enum AddrClass { Public, Loopback, LinkLocal, Private, SharedCgnat, Unspecified, Multicast, Broadcast, Documentation, Reserved, MetadataService }
pub fn classify(ip: std::net::IpAddr) -> AddrClass;   // IPv4-mapped and IPv4-compatible IPv6 are classified as their IPv4
/// Every spelling (integer, octal, hex, short forms, bracketed IPv6, trailing dot) normalised before any check.
pub fn parse_host(host: &str) -> Result<HostSpelling, NetError>;
pub enum HostSpelling { Ip(std::net::IpAddr), Name(String /* lower-case, IDNA ASCII form */) }
pub struct HostPort { pub host: HostSpelling, pub port: u16 }   // a `[net] private_allow` entry (P24)
pub enum NetError { BadHost(String), BadScheme(String), CredentialsInUrl }
/// P24 for provider endpoints: loopback may be http; anything else must be https; no URL with credentials.
pub fn check_provider_endpoint(url: &url::Url) -> Result<EndpointClass, NetError>;
pub enum EndpointClass { Loopback, Remote }
```

#### 3.1.10 Display escaping and the opener (T4)

```rust
// udx_core::display (P4, P49, P16's chat clause)
pub struct DisplayText(String);   // safe to show: control (C0, C1, DEL), bidirectional controls (U+202A..U+202E,
                                  // U+2066..U+2069, U+200E, U+200F, U+061C) and zero-width characters escaped as \u{..}
pub fn escape(text: &str) -> DisplayText;
pub fn display_host(host: &str) -> DisplayHost;   // ASCII (punycode) form always; Unicode form beside it when non-ASCII (D15)
pub struct DisplayHost { pub ascii: String, pub unicode: Option<String> }

// udx_core::opener (P54, P15): the one opener for the whole app
pub enum OpenTarget { Url(url::Url), RevealFile(PathBuf) }
pub enum OpenSource { User, Untrusted { origin: String } }
pub struct OpenRequest { pub target: OpenTarget, pub source: OpenSource, pub confirmed_full_url: bool }
/// http and https only, no credentials in the address, never through a shell (argv to `open`, `xdg-open` or
/// ShellExecuteW with the URL as one argument); a file that would execute, or HTML a model wrote, is revealed, not
/// opened; an untrusted source needs `confirmed_full_url`.
pub fn open(req: &OpenRequest) -> Result<(), OpenError>;
pub enum OpenError { SchemeRefused(String), CredentialsInUrl, NeedsConfirmation, Launch(std::io::Error) }
```

#### 3.1.11 Diagnostics (T6)

```rust
// udx_core::diag (P57)
/// Our own log, written redacted to StatePaths::logs_dir, owner-only. No `log` or `tracing` subscriber is installed for
/// third-party crates, so RUST_LOG has no effect and no HTTP header from them can reach a file (D13).
pub fn init(paths: &StatePaths, redactor: Arc<RwLock<Redactor>>) -> Result<(), std::io::Error>;
pub fn event(level: Level, target: &'static str, message: &str);
pub enum Level { Error, Warn, Info, Debug }
```

### 3.2 `udx-providers` (T7 to T9)

```rust
// udx_providers
pub use udx_core::model::{ProviderId, ModelId, MicroUsd, ToolSpec, ToolCalling};   // homes: section 3.1.0

#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &ProviderId;
    fn kind(&self) -> ProviderKind;
    fn endpoint(&self) -> &BoundEndpoint;
    /// The model catalog with capabilities (design 5.1): tool calling, vision, context size.
    async fn catalog(&self) -> Result<Vec<ModelInfo>, ProviderError>;
    /// A streaming chat call. Dropping the stream cancels it. Events are provider-neutral (StreamEvent).
    fn stream(&self, req: ChatRequest) -> futures::stream::BoxStream<'static, Result<StreamEvent, ProviderError>>;
    /// Input tokens for the worst-case spend estimate (P40): exact where the provider can count, otherwise an upper
    /// bound (UTF-8 bytes plus a per-message overhead), marked as such.
    async fn count_input_tokens(&self, req: &ChatRequest) -> Result<TokenCount, ProviderError>;
    /// What the endpoint is: for P40, whether a loopback endpoint is a recognised local model server.
    async fn probe(&self) -> Result<ProbeInfo, ProviderError>;
    /// P59: whether the provider reported a prompt shorter than what was sent.
    fn prompt_integrity(&self, sent: &PromptMeasure, usage: &Usage) -> PromptIntegrity;
}
pub enum ProviderKind { Anthropic, Ollama, Mock }
pub struct ModelInfo { pub id: ModelId, pub context_window: Option<u64>, pub max_output: Option<u64>,
                       pub tool_calling: ToolCalling, pub vision: bool, pub reasoning: bool, pub price: Option<Price> }
pub struct Price { pub input_per_mtok: MicroUsd, pub output_per_mtok: MicroUsd, pub cache_read_per_mtok: MicroUsd,
                   pub cache_write_per_mtok: MicroUsd, pub source: PriceSource }
pub enum PriceSource { BuiltIn { read_on: String /* date the builder read the vendor's page */ }, User, Free(FreeReason) }
pub enum FreeReason { RecognisedLocalServer, MarkedFreeByUser }
pub struct TokenCount { pub tokens: u64, pub exact: bool }
pub struct ProbeInfo { pub recognised_local_server: Option<String /* e.g. "ollama 0.x" */>, pub loopback: bool }

pub struct ChatRequest {
    pub model: ModelId,
    pub system: Vec<SystemBlock>,       // ordered so injected context does not break a prompt cache (design 5.2)
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,           // already sanitised for this provider (design 5.2) by `sanitize_tools`
    pub max_output_tokens: u32,
    pub context_window: Option<u64>,    // local servers: the window the engine sets (Ollama num_ctx)
    pub temperature: Option<f32>,
}
pub struct SystemBlock { pub text: String, pub cache_hint: bool }
pub struct Message { pub role: MessageRole, pub content: Vec<ContentBlock> }
pub enum MessageRole { User, Assistant }
pub enum ContentBlock {
    Text { text: String },
    Reasoning(OpaqueReasoning),          // kept and sent back unchanged (design 5.2)
    ToolUse { id: CallId, name: String, input: serde_json::Value },
    ToolResult { id: CallId, content: String, is_error: bool },   // content already labelled and redacted by the engine
}
pub struct OpaqueReasoning { pub provider: ProviderKind, pub data: serde_json::Value }  // stored verbatim, never edited
pub fn sanitize_tools(kind: ProviderKind, tools: &[ToolSpec]) -> Vec<ToolSpec>;

pub enum StreamEvent {
    MessageStart { provider_message_id: Option<String> },
    TextDelta { index: u32, text: String },
    ReasoningDelta { index: u32, text: String },
    ReasoningBlock { index: u32, block: OpaqueReasoning },
    ToolCallStart { index: u32, id: CallId, name: String },
    ToolCallArgsDelta { index: u32, fragment: String },
    ToolCallEnd { index: u32 },
    Usage(Usage),
    MessageEnd { stop: ProviderStop },
}
pub struct Usage { pub input_tokens: Option<u64>, pub output_tokens: Option<u64>, pub cache_read_tokens: Option<u64>,
                   pub cache_write_tokens: Option<u64>, pub prompt_eval_tokens: Option<u64> }
pub enum ProviderStop { EndTurn, ToolUse, MaxTokens, StopSequence, Refusal, Other(String) }
pub struct PromptMeasure { pub upper_bound_tokens: u64, pub lower_bound_tokens: u64, pub context_window_set: Option<u64> }
pub enum PromptIntegrity { Intact, Unknown, Cut { reported: u64, expected_at_least: u64 } }

pub enum ProviderError {
    KeyMissing, Auth, RateLimited { retry_after: Option<Duration> }, Overloaded, Transient(String),
    BadRequest(String), ContextTooLong, Http { status: u16 }, Protocol(String), Timeout, Cancelled,
    EndpointRefused(NetError),           // P24
    HostChangedKeyCleared,               // P22
}
impl ProviderError { pub fn retryable(&self) -> bool; }

// Endpoint and key binding (P22, P24)
pub struct BoundEndpoint { /* base URL checked by check_provider_endpoint; the key's SecretName is derived from the provider id and host */ }
impl BoundEndpoint {
    pub fn new(base_url: &url::Url) -> Result<BoundEndpoint, NetError>;
    pub fn key_name(&self, provider: &ProviderId) -> SecretName;   // account "provider:<id>@<host>": a host change finds no key
}

// Transport: every provider call goes through it, so fixtures replace the network in tests
#[async_trait::async_trait]
pub trait HttpTransport: Send + Sync {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, TransportError>;
}
pub struct HttpRequest { pub method: http::Method, pub url: url::Url, pub headers: Vec<(String, SensitiveHeader)>, pub body: Vec<u8>, pub timeout: Duration }
pub enum SensitiveHeader { Plain(String), Secret(Secret) }   // secret header values never reach Debug, logs or fixtures
pub struct HttpResponse { pub status: u16, pub headers: Vec<(String, String)>, pub body: futures::stream::BoxStream<'static, Result<bytes::Bytes, TransportError>> }
pub struct ReqwestTransport { /* rustls with ring, platform verifier, no proxy from the environment, no automatic redirects */ }

// Role routing (design 5.1)
pub struct Router { /* roles -> (provider, model) with an ordered fallback list */ }
impl Router {
    pub fn from_config(cfg: &Config, providers: &ProviderSet) -> Result<Router, RouteError>;
    pub fn resolve(&self, role: Role) -> Result<Route, RouteError>;           // first healthy entry; between turns only
    pub fn next_fallback(&self, role: Role, failed: &Route) -> Option<Route>;
}
pub struct Route { pub provider: ProviderId, pub model: ModelId, pub reason: String }
pub enum RouteError { NoRoute(Role), UnknownProvider(ProviderId), UnknownModel { provider: ProviderId, model: ModelId } }
/// The configured providers, built once from Config; the engine and the router look providers up here.
pub struct ProviderSet { /* ProviderId -> Arc<dyn Provider> */ }
impl ProviderSet {
    pub fn from_config(cfg: &Config, secrets: &dyn SecretStore, transport: Arc<dyn HttpTransport>) -> Result<ProviderSet, ProviderError>;
    pub fn get(&self, id: &ProviderId) -> Option<Arc<dyn Provider>>;
    pub fn ids(&self) -> Vec<ProviderId>;
}

#[cfg(feature = "testing")]
pub mod testing {
    /// The scripted mock provider of design 10.1: replays a script of StreamEvents per call, records every request
    /// (for P21's and P56's style of assertions), can delay the first delta (for the first-token target) and can report
    /// any Usage (for P59 and P40).
    pub struct MockProvider { /* ... */ }
    pub struct Script { pub calls: Vec<ScriptedCall> }
    pub struct ScriptedCall { pub events: Vec<(Duration, StreamEvent)>, pub expect: Option<RequestMatcher> }
    /// What a scripted call expects of the request; a mismatch fails the call with ProviderError::Protocol.
    pub struct RequestMatcher { pub model: Option<ModelId>, pub last_user_contains: Option<String>, pub tool_names: Option<Vec<String>> }
    impl MockProvider { pub fn new(script: Script) -> MockProvider; pub fn requests(&self) -> Vec<ChatRequest>; }
    /// Replays recorded HTTP exchanges from crates/udx-providers/tests/fixtures/<provider>/<case>.json.
    pub struct FixtureTransport { /* ... */ }
}
```

Fixtures: one JSON file per exchange, holding the request's method, path and body (with no header values at all) and the
response's status and body as the chunks the server sent, with their timing. Recording is opt-in
(`UNIDEX_RECORD_FIXTURES=1` and a key in the environment, never on CI or forks) and writes through the redactor; the
recorded body is reviewed before commit. The conformance suite (`tests/conformance.rs`) runs every provider against its
fixtures: text streaming, a tool call with streamed arguments, two parallel tool calls, usage, a rate limit with
`retry-after`, an overloaded error, a malformed chunk, a stream cut mid-event, an empty response, a refusal.

### 3.3 The two M1 providers (T8, T9)

See section 5 for the choice. Their public surface is the `Provider` trait plus a constructor.

```rust
// udx_providers::anthropic
pub struct AnthropicProvider { /* ... */ }
impl AnthropicProvider { pub fn new(id: ProviderId, endpoint: BoundEndpoint, key: Secret, transport: Arc<dyn HttpTransport>, models: Vec<ModelInfo>) -> Self; }

// udx_providers::ollama
pub struct OllamaProvider { /* ... */ }
impl OllamaProvider { pub fn new(id: ProviderId, endpoint: BoundEndpoint, transport: Arc<dyn HttpTransport>, models: Vec<ModelInfo>) -> Self; }
```

Not verified (each task verifies against the vendor's published API reference and records the pages and dates in its
pull request; fixtures are recorded from a live call only by the maintainer's opt-in):

- Anthropic: the Messages API's request shape, the server-sent event names and their order, the header carrying the key
  and the version header, the error body shape and `retry-after`, how thinking blocks and their signatures are returned
  and must be sent back, and the token-counting endpoint.
- Ollama: `/api/chat` streaming as newline-delimited JSON, its `tools` and `tool_calls` shape, `options.num_ctx`,
  `prompt_eval_count` and `eval_count`, `/api/show` (context length and a "tools" capability), `/api/tags`, and
  `/api/version` as the recognition probe for P40. T9 must establish whether `prompt_eval_count` counts only tokens not
  served from the server's prompt cache; the P59 formula (`prompt_integrity`) is settled from that measurement, and if
  the count cannot tell a cut prompt from a cached one, the engine guarantees no cut instead: it sets `num_ctx` to at
  least the upper-bound token count plus the output budget and refuses to send when that exceeds the model's window.

### 3.4 `udx-tools` (T20 to T22)

```rust
// udx_tools (uses udx_core::{model::ToolSpec, policy::DecisionRef, content::{Origin, OutStream, TodoChange},
// workspace::WorkspaceInfo, sandbox::{Sandbox, ProcessRegistry, NetworkGrant, Platform}}: section 3.1.0)
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn spec(&self) -> ToolSpec;
    /// Pure: what the call would touch, for permissions and the ask card. Never performs I/O beyond the file guard's
    /// path checks. Invalid arguments are an error the engine counts toward the invalid-call guard.
    fn classify(&self, args: &serde_json::Value, ctx: &ToolContext) -> Result<ToolIntent, ToolError>;
    /// Runs an intent the engine approved. Only the engine calls this.
    async fn run(&self, approved: Approved, ctx: &ToolContext, out: &dyn ToolOutputSink) -> Result<ToolOutput, ToolError>;
}
pub struct Approved { pub call: CallId, pub intent: ToolIntent, pub decision: DecisionRef, pub network: NetworkGrant }
pub struct ToolContext {
    pub session: SessionId,
    pub guard: Arc<FileGuard>,
    pub sandbox: Arc<dyn Sandbox>,
    pub processes: Arc<dyn ProcessRegistry>,     // the shell tool registers each group's KillHandle here (P50)
    pub workspace: WorkspaceInfo,                // root, trust state, git-ignore matcher, submodule roots
    pub env: Arc<BaseEnv>,
    pub session_tmp: PathBuf,
    pub undo: Arc<UndoStore>,
    pub limits: Limits,
    pub redactor: Arc<RwLock<Redactor>>,
    pub egress: Arc<dyn EgressGate>,             // implemented by udx-engine (P14 for redirects and links)
    pub resolver: Arc<dyn Resolver>,             // name lookups for web_fetch, after policy (threat model 4.6)
    pub net: NetPolicy,
    pub attendance: Attendance,
    pub cancel: tokio_util::sync::CancellationToken,
}
pub enum ToolIntent {
    Read { path: GuardedPath, verdict_kind: VerdictKind },
    Write { path: GuardedPath, verdict_kind: VerdictKind, bytes: u64, creates: bool },
    Edit { path: GuardedPath, verdict_kind: VerdictKind, replacements: u32 },
    Glob { root: GuardedPath, pattern: String },
    Grep { root: GuardedPath, pattern: String, glob: Option<String> },
    Command { shape: CommandShape, cwd: GuardedPath, wants_network: bool },
    Fetch { url: url::Url, method: http::Method, has_body: bool },
    Todo(TodoChange),
}
pub enum VerdictKind { Inside, Outside, Protected(Protected) }
/// P2: the parse decides how a command may run.
pub enum CommandShape {
    Simple { argv: Vec<String> },        // no shell syntax at all: may run under a rule, as argv, with no shell
    NeedsShell { script: String, why: ShellReason },   // never under a rule: always asks, then runs in FixedShell
}
pub enum ShellReason { Pipe, Sequence, AndOr, Substitution, Redirection, Wildcard, Assignment, Quoting, Other }
pub struct ToolOutput { pub content: String, pub origin: Origin, pub truncated: bool, pub meta: ToolMeta }
pub enum ToolMeta { None, Command(ExitInfo), Fetch(FetchMeta), Write(WriteOutcome) }
pub trait ToolOutputSink: Send + Sync { fn chunk(&self, stream: OutStream, text: &str); }  // shell output as it arrives
pub enum ToolError { InvalidArgs(String), Guard(GuardError), Sandbox(SandboxError), Net(FetchError), Refused(String), Cancelled, Io(std::io::Error) }

/// In `src/registry.rs`, written whole by T2b: wiring only. It constructs each tool's struct (`files::ReadFile`, ...,
/// `shell::Shell`, `web_fetch::WebFetch`, `todo::Todo`), whose trait methods are the owning tasks' stubs until they land.
pub fn m1_tools() -> Vec<Arc<dyn Tool>>;   // read_file, write_file, edit_file, glob, grep, shell, web_fetch, todo
/// P2: a conservative parser. Anything it is not sure is Simple is NeedsShell. Windows always yields NeedsShell (no
/// parser for cmd.exe yet, so nothing is auto-allowed there; design 8 exception, maintainer decision of 2026-11-17).
pub fn parse_command(command: &str, platform: Platform) -> CommandShape;
```

Tool rules (each is a test in the owning task):

- `read_file`: through `FileGuard::open_read` with `limits.read_cap_bytes` (P40's single-file cap); outside the
  workspace it asks (P60) and its result exposes the turn; `Origin::File` carries `outside_workspace` and `third_party`.
- `write_file`, `edit_file`: through `FileGuard::write_atomic`; protected paths ask (P8); outside the workspace only
  under a rule naming the folder; never create links or executable files; undo copy first (P3). `edit_file` takes exact
  `old` and `new` strings and fails when `old` is absent or ambiguous.
- `glob`, `grep`: inside the workspace only (outside roots ask like reads), through `ignore`'s walker with no link
  following; results list paths and line excerpts capped by `limits.output_cap`; deny-listed files are skipped silently
  and counted.
- `shell`: absent when `Sandbox::availability()` is `Unavailable` unless the session holds `UnsandboxedShell` (P7);
  `Simple` runs `Direct`, `NeedsShell` runs `FixedShell`; network only with a grant or an approved ask (P1); output is
  streamed to the sink and capped with a truncation marker (P51); the turn sees `ExitInfo`; a denied write to a
  protected path that the backend reported (`ExitInfo::denied_writes`) lets the engine offer one re-run with a wider
  profile on an approved card (P8); a backend that cannot report never offers one; `ExitInfo::protected_created`
  raises a card naming each created file (section 3.6). The child's PATH skips relative and empty entries, and
  `Direct`'s program is resolved against it before spawn (P2).
- `web_fetch`: section 3.5.
- `todo`: a list kept in the session (add, update status, list); written to the session log; touches no file.

### 3.5 `web_fetch` and the network policy (T22)

```rust
// udx_tools::net
pub struct NetPolicy { pub private_allow: Vec<HostPort> /* attended only */, pub caps: FetchCaps }
pub struct FetchCaps { pub max_body: u64 /* 10 MiB */, pub max_decompressed: u64 /* 20 MiB */, pub timeout: Duration /* 30 s */, pub max_redirects: u8 /* 5 */ }
#[async_trait::async_trait]
pub trait Resolver: Send + Sync { async fn lookup(&self, host: &str) -> Result<Vec<std::net::IpAddr>, FetchError>; }
#[async_trait::async_trait]
pub trait EgressGate: Send + Sync {
    /// Asked before the first request and before following every redirect hop. May raise an ask and wait for it.
    async fn check(&self, req: &EgressRequest) -> EgressVerdict;
    /// Records links found verbatim in fetched content (for P14's verbatim rule), with their origin.
    fn record_links(&self, origin: &url::Url, links: &[url::Url]);
}
// EgressRequest and EgressVerdict are udx_core::policy types (section 3.1.0); udx-engine implements EgressGate.
pub struct FetchMeta { pub final_url: url::Url, pub status: u16, pub content_type: Option<String>, pub bytes: u64, pub redirects: Vec<url::Url> }
pub enum FetchError { Scheme, Credentials, Address(AddrClass), Lookup(String), Redirects, BodyTooLarge, DecompressedTooLarge,
                      Timeout, Egress(EgressVerdict), SecretInUrl /* P21 backstop */, Http(u16), Transport(String),
                      ContentType(String) /* image loads: not an allowed image type */ }
/// The policy path below, for a caller that wants bytes rather than text: the engine's P15 image load
/// (SessionCommand::LoadImage). Same steps 1 to 6, with `EgressRequest::ImageLoad` at step 2, an allow list of image
/// content types (png, jpeg, gif, webp), a 5 MiB cap, and no HTML step.
pub async fn fetch_bytes(url: &url::Url, req: EgressRequest, ctx: &ToolContext, max: u64) -> Result<(FetchMeta, Vec<u8>), FetchError>;
```

Order of operations for every request and every redirect hop (threat model 4.6, P14, P21, P24):

1. Scheme `http` or `https`; no credentials in the URL; the redactor's backstop scan refuses a URL holding a stored
   secret before any connection (P21's test).
2. `EgressGate::check` (P14) runs before any name lookup, so a link to an origin new to the session asks before it is
   resolved.
3. `parse_host`; an IP spelling is classified directly; a name is resolved once through `Resolver`.
4. Every resolved address is classified; any non-public address refuses the request unless `private_allow` names that
   host and port and the run is attended (P24).
5. The connection goes to the checked address (a resolver override per request; no second lookup), with no proxy from
   the environment.
6. The body is read up to `max_body`; `Accept-Encoding: identity` is sent, and a compressed body (gzip or deflate) is
   decoded with `flate2` up to `max_decompressed`; other encodings are refused.
7. HTML becomes text with its links extracted; text and JSON pass as they are; other types return metadata only. The
   result's origin is the final URL; it exposes the turn.

### 3.6 `udx-sandbox` backends (T10 to T12)

```rust
// udx_sandbox
pub fn platform_sandbox(paths: &StatePaths, env: &dyn Env) -> Arc<dyn Sandbox>;  // probes once and caches; reprobe on demand
/// Reached from the app's and the CLI's `main` through the hidden argument `--udx-sandbox-probe <check>`: runs one
/// ProbeCheck from inside the sandbox and prints its ProbeOutcome as one JSON line (section 3.1.5).
pub fn probe_main(check: &str) -> std::process::ExitCode;
pub struct ProcessSupervisor { /* every live sandboxed group per session, held by its KillHandle */ }
impl udx_core::sandbox::ProcessRegistry for ProcessSupervisor {}
impl ProcessSupervisor {
    pub fn new() -> Arc<ProcessSupervisor>;
    pub fn live(&self, session: &SessionId) -> Vec<ProcessInfo>;
    pub fn end_session(&self, session: &SessionId) -> Vec<ProcessInfo>;   // kills every group; returns what was killed
    pub fn end_all(&self) -> Vec<ProcessInfo>;                            // app quit
    pub fn kill(&self, session: &SessionId, pid: u32) -> Result<(), SandboxError>;
    pub fn holding_network_or_credentials(&self, session: &SessionId) -> Vec<ProcessInfo>;  // P50: asked on exposure or revoke
    /// P51: while an unattended run has a live group, reads free space on `volume` once a second (never while idle, so
    /// the idle target holds) and kills that run's groups when it falls below `floor`. Dropping the watch stops it.
    pub fn watch_free_space(&self, session: &SessionId, volume: PathBuf, floor: u64) -> FreeSpaceWatch;
}
pub struct FreeSpaceWatch { /* ... */ }
// Windows access entries (D3; the other platforms return an empty list and do nothing)
pub struct AccessEntryRecord { pub workspace: Option<WorkspaceKey>, pub path: PathBuf, pub sid: String, pub kind: EntryKind, pub added_ms: u64 }
pub enum EntryKind { AllowWorkspace, AllowSessionTmp, DenyProtected, DenyListed, AllowReadList }
pub enum CleanScope { Workspace(PathBuf), All }
pub struct CleanReport { pub removed: u32, pub failed: Vec<(PathBuf, String)> }
pub fn access_entries(paths: &StatePaths) -> Result<Vec<AccessEntryRecord>, SandboxError>;
pub fn clean_access_entries(paths: &StatePaths, env: &dyn Env, which: CleanScope) -> Result<CleanReport, SandboxError>;
#[cfg(feature = "testing")] pub mod testing {
    pub struct UnavailableSandbox;   // reports ForcedByTestBuild
    pub struct FakeSupervisor;       // a recording ProcessRegistry, so engine tests need no real backend
}
```

**Process lifetime, the same on every platform (P50).** A group lives until the supervisor ends it (session end, app
quit, the end of an unattended run, or the user's kill from the list of live processes) or until no process is left in
it. On Linux the helper stays as the PID namespace's first process and reaps orphans, rather than replacing itself with
the command, so a background process outlives its command there exactly as it does in a macOS process group or a
Windows job; `p50_live_processes_listed` asserts the same behaviour on all three. P50 names "a cgroup on Linux"; the
PID namespace is what makes the group impossible to leave with `setsid`, and a cgroup is used only for limits where a
delegated one is writable. The threat model update (T40) records that wording.

Per platform (D1, D2, D3, D28; each backend task writes a decision record):

- **macOS** (`macos/`): `/usr/bin/sandbox-exec -p <profile> -D WORKSPACE=<path> -D TMP=<path> ... -- <program>`. Paths
  travel as `-D` parameters, never interpolated into the profile text. The profile starts `(deny default)`, allows
  process fork and exec, reads of the read list, writes of the workspace and session temp minus `write_deny`, denies
  the deny list inside granted trees (table below), denies `lsopen` and `appleevent-send`, and allows a short reviewed
  `mach-lookup` list, each entry with a reason in the profile's comments. P23: `process-info*` and `signal` only for
  the sandbox's own processes, and `sysctl-read` only for a named list (each with a reason), never the process-table
  and argument sysctls (`kern.procargs2`, `kern.proc.*`), so a command cannot read another process's arguments or
  environment (not verified: the exact operation and filter names that govern `kern.procargs2`; T10's first step).
  Process group: the child starts in a new process group; killing signals the group and then runs the marker sweep
  (D2). P51 by `setrlimit` in the child before exec: process count (the user's current count plus the margin) and CPU;
  wall time by the supervisor. No memory limit is applied on macOS (residual P51-a, listed for T40).
- **Linux** (`linux/` and the binary `unidex-sandbox-helper`): the app runs the helper with its arguments ended by
  `--` before the command. The helper creates user, mount, PID and IPC namespaces, and a network namespace when the
  command has no network; maps the user; builds a new root from bind mounts (read list read-only, workspace and session
  temp read-write, a sized tmpfs for the temp folder) with its own `/proc` for the PID namespace, so no host process's
  `/proc/<pid>/environ` is visible (P23); hides `$XDG_RUNTIME_DIR`, the session bus and the X11 and Wayland socket
  folders; sets `no_new_privs`; applies Landlock; loads a seccomp filter refusing the `TIOCSTI` and `TIOCLINUX` ioctls,
  `keyctl`, `add_key`, `request_key`, `ptrace`, `process_vm_*`, `bpf`, `perf_event_open`, `userfaultfd`,
  `io_uring_setup`, `io_uring_enter`, `io_uring_register`, `unshare` and `setns`, `clone` with namespace flags, and
  `clone3` with `ENOSYS` (its flags sit behind a pointer the filter cannot read, so the C library falls back to `clone`,
  whose flags the filter checks); closes every inherited descriptor except standard input, output and error
  (`close_range`); calls `setsid` with no controlling terminal; and starts the command as its child. P50: killing the
  PID namespace ends every descendant, including ones that called `setsid`. P51: cgroup v2 `pids.max` and `memory.max`
  when a delegated cgroup is writable, otherwise `RLIMIT_NPROC` at the user's current count plus the margin (reported
  in `BackendDetails`). On default Ubuntu, AppArmor lets only a profiled binary create user namespaces; the `.deb`
  installs `/etc/apparmor.d/unidex-rust-preview-sandbox` granting `userns` to
  `/usr/lib/unidex-rust-preview/unidex-sandbox-helper` and loads it in its post-install script (D1, D12, both decided
  by the maintainer). Availability is decided by the probe, not by the package (D1): wherever the full sandbox applies,
  the tarball included, the shell is on; on default Ubuntu without the profile it is off. Not verified: the exact
  profile syntax on the runner's AppArmor version; T11 verifies it on CI.
- **Windows** (`windows/`): an AppContainer per workspace with a job object (D3, decided by the maintainer on
  2026-11-17); no capabilities without a network grant (no network, loopback blocked); a job object with kill-on-close,
  an active-process limit, a memory limit, UI restrictions (desktop, clipboard, global atoms, handles, system
  parameters) and no breakaway; `cmd.exe /d /s /c` for `FixedShell` (D21). An AppContainer process cannot open a
  process of the user outside it (P23; tested). Every Windows command asks (P2). The access-list footprint is below.
  Not verified: which ordinary tools work inside the AppContainer (threat model 7 expects many not to); T12 lists the
  results. A less privileged AppContainer (no `ALL APPLICATION PACKAGES` membership) would cut broker access further;
  T12 tries it first and falls back if ordinary tools fail, recording which (not verified).

**Networked profiles (P1, P43; D28).** A command with `NetworkGrant::Full` keeps every other part of its profile. What
changes, per platform:

- **macOS:** the profile adds network over IP only, outbound to remote IP addresses and binding and accepting on local
  IP addresses (`remote ip` and `local ip` filters). It never adds a `unix-socket` filter or an unfiltered `network*`
  allow, so no Unix-domain socket outside the session temp folder can be reached (a container engine's socket, an
  agent's socket, the app's own); Unix sockets inside the session temp folder stay allowed, and `credential_sockets`
  by exact path. It adds the `mach-lookup` entries the certificate trust service and name resolution need, each named
  and reviewed. If name resolution needs the resolver's Unix socket, exactly that one literal path is allowed, with a
  reason in the profile, and the escape matrix's stand-in socket outside the workspace must still be refused (not
  verified which the resolver uses on the runner's macOS; T10's first step).
- **Linux:** a networked command cannot have an empty network namespace, and a user-mode network stack would need an
  install, so it shares the host's network namespace. Abstract Unix sockets are scoped by network namespace, so the
  helper then requires Landlock ABI 6 or higher and applies its abstract-socket and signal scopes (`Scope::
  AbstractUnixSocket`, `Scope::Signal`; fact: present at ABI 6 in `landlock` 0.4.7, `src/scope.rs`), which refuse a
  connection to an abstract socket outside the command's Landlock domain, the X server's `@/tmp/.X11-unix/X0` and an
  abstract session bus included. Pathname sockets are kept out by the mount namespace alone, because Landlock does not
  restrict connecting to an existing pathname socket (threat model 7): the new root is built only from the read list,
  the workspace and the fresh session tmpfs, and never contains the host's `/run`, `/var/run`, `/tmp`, `/dev/shm`,
  `$XDG_RUNTIME_DIR` or the user's home; a read-list entry that is or contains one of those folders is refused when the
  profile is built. Below ABI 6, or where Landlock is absent, `NetworkGrants` is `Unavailable(LandlockScopeMissing)`:
  network grants and networked asks are refused with that reason and commands run without network (fail closed, P7).
  Not verified: the Landlock ABI of the `ubuntu-26.04` runner's kernel and of a default Ubuntu install; T11's first
  step finds out on CI which case applies there, and the networked tests assert the matching behaviour (the escape
  matrix where network grants are available, the refusal where they are not). Residuals for T40: a networked command
  reaches the host's loopback services (no host filtering, threat model 7), and a pathname socket that a process outside
  the sandbox creates inside the workspace is reachable (P43-a).
- **Windows:** the AppContainer gets the `internetClient` capability only, never `internetClientServer`,
  `privateNetworkClientServer` or a loopback exemption, so loopback and the local network stay unreachable; named
  pipes, COM and WMI stay governed by the container's access as without network.

**Protected and denied paths inside granted trees (P8, P10, P48), per backend.** Landlock is allow-only and cannot
subtract a sub-path, and bind mounts and Windows deny entries apply only to paths that exist when the profile is built,
so the mechanism differs:

| | macOS | Linux | Windows |
|---|---|---|---|
| `write_deny`, existing path | `(deny file-write* ...)` on the literal path or sub-path | a read-only bind mount over the path | an explicit deny entry (write, append, delete) for the workspace's container SID |
| `write_deny`, absent path (a configured but missing `core.hooksPath` folder, `.github/workflows/x.yml`, `.claude/settings.json`, `.mcp.json`, `.envrc`) | the same rule: seatbelt rules are path-based, so an absent path is denied | cannot be denied without creating it, and nothing is created: the backend checks each listed path before and after the command and reports any that appeared in `ExitInfo::protected_created`, which raises a card naming the file (residual P8-a, listed for T40) | the same as Linux (residual P8-a) |
| `read_deny` inside granted trees (`.env`, `*.pem`, `*.key` in the workspace) | `(deny file-read* (regex ...))` compiled from each `DenyEntry`, so a file created later is covered | before each command the helper walks the granted trees (no link following, git-ignored files included) and bind-mounts an empty read-only file or folder over each match | the same walk, with an explicit deny entry (read) for the container SID on each match, recorded as below |
| Walk limits (Linux, Windows) | none needed | a file created during a command is not masked until the next command (residual P10-a); past `limits.deny_walk_entries` (default 500,000 entries) the scan stops, and every command in that workspace then asks in every mode, its card saying the scan was incomplete | the same |
| Reports denied writes (P8's re-run offer) | yes, from the unified log's sandbox violation records for the child's processes (not verified that an unprivileged process can read them on the runner's macOS; T10's first step; if not, `reports_denied_writes` is false) | no: Landlock and read-only mounts return an error to the process only | no |

P8 lets the app offer a wider re-run only after the sandbox reports a denied write to a protected path, so where a
backend cannot report, no re-run is offered (fail closed; residual P8-b for T40: the re-run offer exists on macOS
only, if the log can be read). Tests per platform: `p8_sandbox_write_to_absent_protected_path` (macOS: denied; Linux and
Windows: reported in `protected_created` and raised on a card) and `p10_sandbox_read_of_in_tree_deny_listed_file_fails`.

**The Windows access-list footprint (D3).** Every entry is written to `StatePaths::sandbox_dir/grants.toml` before it
is added (write-ahead), so a crash never leaves an unrecorded entry.

- **Which folders get which entries, and when.** At the first sandboxed command in a workspace, trusted or not, the
  backend creates that workspace's AppContainer profile (`UniDeX.RustPreview.Sandbox.<first 12 hex characters of the
  workspace key>`). One profile per workspace means an entry granted for one workspace gives nothing to a command run
  for another; with one shared SID every granted workspace would be readable by every sandboxed command, breaking P10.
  Then, before each command: the workspace root gets one inheritable allow entry (read, write, execute, delete) for
  that SID, added once (Windows propagates it to every existing file and folder below, which changes each one's
  security descriptor and takes time on a large tree; T12 measures it on a generated 100,000-file tree and records the
  figure in 0016); existing protected paths and in-tree deny-list matches get explicit deny entries for the same SID,
  refreshed before each command; the session temp folder gets an inheritable allow entry and is deleted with its
  entries at session end. P10's read list under `<home>` (for example `.cargo`, `.rustup`, `.gitconfig`) gets one read
  and execute entry per existing listed path for one custom capability SID (`unidexToolchainRead`, derived with
  `DeriveCapabilitySidsFromName`) that every workspace's container token carries, so each home path carries one entry,
  not one per workspace (not verified that a lowbox token's custom capability is honoured by the access check; T12's
  first step; if not, one entry per workspace SID, recorded the same way). System folders need no entry: they already
  grant read access to all application packages.
- **The user is told.** The first command's card in a workspace (every Windows command asks, P2) says that running
  commands there adds an access entry to the folder and names the command that removes it.
- **Removal, for every workspace, never-trusted ones included.** Entries stay until removed: a per-workspace SID grants
  nothing elsewhere, and removing and re-adding per session would propagate across the whole tree every time. Removal
  paths: `unidex sandbox clean DIR` (one workspace's entries, its profile and its records), `unidex sandbox clean
  --all` (every recorded entry, the capability entries, every profile), and `unidex trust DIR --revoke`, which also
  removes that workspace's entries. A removal reads the records, removes each entry (with propagation), deletes the
  profile, then the records; a failure keeps its record and is reported. The NSIS uninstaller does not run it
  (uninstall is gate question 16, M6 and M11), so `unidex doctor` lists the entries and the nightly's release notes say
  to run `unidex sandbox clean --all` before uninstalling.
- **The `unidex doctor` row** "Windows sandbox access entries": the number of workspaces and the files and folders
  carrying entries for UniDeX SIDs and the capability SID (each path with `--json`), whether each recorded path still
  exists, and the removal command. It reads the records only.
- **Tests (T12, Windows runner, opt-in `windows_acl,app_container_profile`):** `acl_entries_recorded_before_added`,
  `acl_clean_removes_workspace_entries_and_profile`, `acl_never_trusted_workspace_cleaned`,
  `acl_home_read_list_entries_via_capability_sid`, `acl_other_workspace_sid_cannot_read`, `acl_revoke_removes_entries`,
  `no_acl_change_without_opt_in`; `doctor_lists_acl_entries` (T25, from planted records).

**The escape matrix** (P43) and the containment tests (P50, P51) run from inside the sandbox on each CI platform. Each
payload only writes a marker file in the test root if it succeeds and registers nothing else; the test asserts no
marker exists. Without network: `open`, `osascript`, `launchctl` (macOS); `systemd-run --user`, `gio open`, an X11
connection to the test Xvfb (pathname and abstract), `TIOCSTI` (Linux); `schtasks`, WMI process creation, a COM
activation (Windows). `p43_escape_matrix_networked`, with a network grant: a stand-in Unix socket the test binds outside
the workspace (macOS, Linux); an X11 connection to the test Xvfb by its abstract and its pathname socket, and an
abstract socket the test binds in the parent namespace (Linux); a stand-in named pipe outside the container's access,
plus `schtasks`, WMI and COM again (Windows).

**The networked https check (P43, P23).** It proves that a networked command reaches an https stand-in
with certificate checks on while the secret store stays out of reach. macOS: the test certificate authority is added
to a temporary keychain on the CI runner only, under the opt-in `trust_store_cert`, so the platform verifier's call to
the certificate trust service goes through the profile's `mach-lookup` allowance. Linux: `SSL_CERT_FILE` points at the
test authority inside the session temp folder (no OS change). Windows: the client trusts the test authority from a
file, so the Windows test does not exercise the OS trust store (said in the test). The secret-store half asserts the
specific denial (section 3.1.5) and, under the opt-in `secret_store`, that an item the test planted is not returned.

**Where tests run (no side effects on the development Mac).** No test, probe or check run on the development Mac may
change OS state outside its temporary folder. These run only on CI runners, behind `UNIDEX_TEST_OS_SIDE_EFFECTS`
naming their kind (section 3.1.1), and are skipped elsewhere with a printed notice: the escape matrices
(`sandbox_escape_matrix`), the macOS daemon sweep test (`sandbox_escape_matrix`), the macOS disk-image test volume
(`disk_image`), the trust-store certificate (`trust_store_cert`), planting secret-store items (`secret_store`), access
entries and AppContainer profiles (`windows_acl`, `app_container_profile`), and installing the `.deb`
(`package_install`). Locally, the macOS backend is exercised only by tests whose effects stay in the test root: a write
outside the workspace aimed at a file inside the test root, a loopback connect to a listener the test opens, planted
files under a `TempHome`, profile parameter quoting, and the probe.

### 3.7 Permissions (T13)

```rust
// udx_engine::permissions (PermissionMode, Decision, DecidedBy, RuleRef, RuleSource, BuiltinAsk: udx_core::policy, 3.1.0)
/// `network`: a rule that allows a command may also grant it network for that command only (P1: "network is a separate
/// permission, per command rule or per session"); the card and the saved rule's text show it.
pub struct Rule { pub id: String, pub source: RuleSource, pub tool: String, pub matcher: Matcher, pub decision: Decision, pub network: bool }
pub enum Matcher {
    Argv(ArgvPattern),                  // exact argv by default (P47)
    PathUnder(PathBuf),                 // a folder; for writes outside the workspace (threat model 4.2)
    Host(String),                       // web_fetch per host (P60)
    Any,
}
pub struct ArgvPattern { pub program: String, pub args: Vec<ArgPattern> }
pub enum ArgPattern { Exact(String), Wildcard { matches_options: bool } }  // a wildcard never matches "-..." unless marked (P2)

pub struct PermissionEngine { /* user rules, workspace narrowing rules, mode, session answers, built-in asks turned off this session */ }
impl PermissionEngine {
    /// `trust` sets the starting mode: an untrusted workspace starts in Ask whatever the configured mode (P60; D29).
    pub fn new(cfg: &Config, workspace_narrowing: Vec<Rule>, attendance: Attendance, trust: &TrustState) -> PermissionEngine;
    /// Precedence (P47): deny (built-in deny, the deny list, P48's persistence locations, user and workspace denies),
    /// then built-in asks (P3, P8, P14, P15, P13, P60) unless turned off by name for this session, then user allows,
    /// then the mode. Workspace rules can only add deny or ask; their allows are ignored. Unattended: Ask becomes Deny
    /// (design 5.2), and no built-in ask is ever off.
    pub fn evaluate(&self, call: &PermissionQuery) -> Evaluation;
    /// "Allow always" saves exactly the rule the card displayed (P47); a user may have edited a wildcard in.
    pub fn rule_for_always(&self, query: &PermissionQuery, edited: Option<ArgvPattern>) -> Rule;
    /// P47: refused with NotFromAppScreen unless `attendance` is Attended(App).
    pub fn turn_off_builtin(&mut self, name: BuiltinAsk, attendance: Attendance) -> Result<(), PermissionError>;
    /// Narrowing always succeeds; widening past the starting mode needs an attended session (P47).
    pub fn set_mode(&mut self, mode: PermissionMode, attendance: Attendance) -> Result<(), PermissionError>;
    pub fn builtins_off(&self) -> Vec<BuiltinAsk>;   // drives the persistent indicator
}
pub struct PermissionQuery { pub tool: String, pub intent: ToolIntent, pub exposed: bool, pub egress: Option<EgressVerdict> }
pub struct Evaluation { pub decision: Decision, pub by: DecidedBy, pub builtin: Option<BuiltinAsk> }

/// P58: asks rate-limited per session; the turn stops after `max_denials` (default 3) denials in a turn.
pub struct AskLimiter { /* ... */ }
impl AskLimiter { pub fn admit(&mut self, now_ms: u64) -> Admit; pub fn record_denial(&mut self) -> DenialState; }
pub enum Admit { Raise, DeferredUntil(u64) }
pub enum DenialState { Continue, StopTurn { denials: u32 } }
pub enum PermissionError { NotAttended, NotFromAppScreen, Unknown(String) }
```

**How built-in asks match a command (P3, "in every mode").** The matcher is conservative: when in doubt, a command
asks. It runs on `CommandShape::Simple` argv (a `NeedsShell` command asks anyway, P2):

- A wrapper program as `argv[0]` makes the command `NeedsShell`, so it always asks: `env`, `nice`, `ionice`, `timeout`,
  `xargs`, `command`, `exec`, `nohup`, `stdbuf`, `time`, `watch`, `chronic`, `flock`, `setsid`, `script`, `busybox`,
  any shell (`sh`, `bash`, `zsh`, `dash`, `fish`), and interpreters given `-c` or `-e`. `sudo`, `doas` and `pkexec` ask
  as `Sudo`.
- `find` with `-exec`, `-execdir`, `-ok`, `-okdir` or `-delete` asks as `RecursiveDelete`.
- The program is matched by file name after stripping any directory (`/usr/bin/git` is `git`) and, on Windows, a
  `.exe`, `.cmd` or `.bat` suffix (every Windows command asks anyway, P2).
- For `git`, global options are parsed before the subcommand (`-C <dir>`, `--git-dir`, `--work-tree`, `-c <k=v>`,
  `--exec-path`, `--namespace`, `-P`, `--no-pager`), so `git -C . push` is `GitPush`. Any `-c` setting an `alias.*`,
  `core.sshCommand`, `core.hooksPath`, `core.fsmonitor`, `credential.*` or a `*.command` key, and any subcommand that
  is not a known built-in git command (it could be an alias), asks as `GitAliasOrConfig`.
- `rm` with `-r`, `-R` or `--recursive` (in any combined short-option spelling), `rmdir` with `-p`, and `git clean`
  ask as `RecursiveDelete`; the package managers' publish verbs (`npm publish`, `cargo publish`, `twine upload`, `gem
  push`, `pnpm publish`, `yarn npm publish`) ask as `PackagePublish`; `gh` verbs other than a fixed read list ask as
  `GhWrite`; the cloud CLIs (`aws`, `gcloud`, `az`, `kubectl`, `terraform`, `pulumi`) ask as `CloudCliWrite` except a
  fixed read list. The tables live in `permissions/builtin.rs`, each entry with a reason.

**Allowing a build or test runner (P2).** A card for a command whose program is a build or test runner (`cargo`,
`npm`, `pnpm`, `yarn`, `make`, `just`, `pytest`, `go`, `gradle`, `mvn`, `dotnet`, and the like, a table in
`permissions/card.rs`) uses the template "Run code from this repository: <command>", and "Allow always" on it shows the
same words; the test `p2_runner_card_says_repository_code` asserts it.

**The permission mode's scope (D29).** The mode belongs to a session. It starts from the configured
`permissions.mode` in a trusted workspace and from `ask` in an untrusted one. `--mode` (CLI) and `SetMode` (app) may
narrow at any time; widening past the starting mode needs an attended session and, in the CLI, a typed confirmation,
and is refused unattended (`p47_mode_widening_refused_headless`). Changing the configured mode to a wider one is
InteractiveOnly (section 3.1.3).

Defaults for a new workspace (P60), which the view-model test asserts:

| Action | ReadOnly | Ask (default) | Auto |
|---|---|---|---|
| Read inside the workspace | allow | allow | allow |
| Read outside the workspace | ask | ask | ask |
| Write inside (not protected) | deny | ask | allow |
| Write to a protected path (P8) | deny | ask | ask |
| Write outside the workspace | deny | needs a rule naming the folder; otherwise deny | same |
| Shell | deny | ask | allow (sandboxed, no network); built-in asks still ask |
| Shell with network | deny | ask | ask |
| `web_fetch` | ask per host | ask per host | allow, subject to P14 after exposure |
| `todo` | allow | allow | allow |

A new workspace starts untrusted (P13) in `Ask` mode. MCP tools and the browser, which P60 also names, do not exist in
M1.

### 3.8 Asks and cards (T13 builds the data; T27 the view-model; T24 the CLI form)

```rust
// udx_core::ask (data shared by the engine, the view-model and the CLI)
pub struct AskCard {
    pub id: AskId,
    pub title: String,                 // app-written words leading with the action (P4), from a fixed template per intent
    pub subject: DisplayText,          // command, path or host, escaped (P4)
    pub hosts: Vec<DisplayHost>,
    pub exposed: bool,                 // the exposure badge (P4)
    pub egress: Option<EgressDetail>,  // P4 for P14 asks
    pub network: bool,
    pub builtin: Option<BuiltinAsk>,
    pub always_rule: Option<String>,   // the exact rule "Allow always" would save, shown as text (P47)
    pub rerun_wider: bool,             // P8's offer after a backend-reported denied protected write
    pub created_protected: Vec<DisplayText>, // protected paths a command created (ExitInfo::protected_created; 3.6)
    pub guard_ms: u32,                 // input ignored for this long after the card appears (P4); 600 ms (D23)
}
/// `composed` comes from EgressVerdict::Ask (section 3.9 says how EgressState computes it), mapped onto `full_url`'s
/// escaped display form by the card builder.
pub struct EgressDetail { pub full_url: DisplayText /* never truncated */, pub composed: Vec<std::ops::Range<usize>>, pub body_summary: Option<String> }
pub enum AskAnswer { AllowOnce, AllowAlways { edited: Option<String> }, Deny, TimedOut /* a deny */ }
```

No choice is the default (P4). In the CLI, the card is printed escaped and the answer is a typed word read from the
terminal device after pending input is discarded (P49): `allow`, `always` or anything else (a deny).

### 3.9 Context, labelling, exposure, trust (T14) and egress (T15)

```rust
// udx_engine::context
pub struct Delimiters { /* per-session random nonce, 128 bits, hex */ }
impl Delimiters { pub fn new(rng: &mut dyn rand::RngCore) -> Delimiters; pub fn wrap(&self, item: &LabelledItem) -> String; }
pub struct LabelledItem { pub origin: Origin, pub body: String }   // body containing a delimiter is escaped first
// Origin is udx_core::content::Origin (section 3.1.0). A resumed session gets a fresh nonce: tool results are stored
// unlabelled (section 4.3) and labelled again with the new delimiters when the context is assembled.
/// Threat model 4.3's exposure table, for the M1 rows: web pages, files outside the workspace, files in an untrusted
/// workspace, third-party paths in a trusted one, and shell output when the command had network or the workspace is
/// untrusted, expose; other files in a trusted workspace, instruction files when trusted and pinned, user messages
/// and the app's own instructions do not.
pub fn exposes(origin: &Origin) -> bool;
pub struct Exposure { pub exposed: bool, pub origins: Vec<Origin> }   // lasts until the context is cleared

pub enum ContextError { Guard(GuardError), ImportOutsideWorkspace(PathBuf), DepthExceeded, TooLarge { bytes: u64 }, Cycle(PathBuf) }

// udx_engine::trust (P13; TrustState is udx_core::policy)
pub struct TrustStore { /* StatePaths::trust_file, owner-only; keyed by WorkspaceKey (canonical path, device and inode) */ }
impl TrustStore {
    pub fn get(&self, ws: &WorkspaceKey) -> TrustState;
    pub fn trust(&self, ws: &WorkspaceKey, instruction_hash: Option<String>, proof: InteractiveProof) -> Result<(), TrustError>;
    pub fn revoke(&self, ws: &WorkspaceKey) -> Result<(), TrustError>;   // the CLI's --revoke also cleans Windows entries (3.6)
}
pub enum TrustError { NeedsInteractive, Io(std::io::Error), Guard(GuardError), Corrupt { line: u32, reason: String } }
/// Instruction files (D16): <workspace>/AGENTS.md and the files it imports with `@path` lines, resolved through the file
/// guard inside the workspace, depth at most 5, 64 KiB in all. Loaded only when the workspace is trusted and the hash
/// of the resolved set equals the pinned one; a changed hash raises InstructionFilesChanged before loading.
pub fn resolve_instruction_set(guard: &FileGuard) -> Result<InstructionSet, ContextError>;
pub struct InstructionSet { pub files: Vec<(PathBuf, String)>, pub hash: String }
/// Third-party paths (P13): node_modules/, vendor/, third_party/, package caches, git-ignored paths, submodules and
/// nested repositories stay labelled and expose even in a trusted workspace.
pub fn is_third_party(ws: &WorkspaceInfo, path: &Path) -> bool;

// udx_engine::egress (P14, P15; EgressRequest, EgressVerdict and their reasons are udx_core::policy, section 3.1.0)
pub struct EgressState { /* typed URLs, URLs requested before exposure, verbatim links with their origins, contacted origins,
                           origins that supplied untrusted content, per-turn and per-session verbatim counts */ }
impl EgressState {
    pub fn evaluate(&self, req: &EgressRequest, caps: &EgressCaps, attendance: Attendance) -> EgressVerdict;
    pub fn record_request(&mut self, url: &url::Url, exposed: bool);
    pub fn record_links(&mut self, origin: &url::Url, links: &[url::Url]);
    pub fn new_turn(&mut self);
    /// P4: the byte ranges of `url`'s display form that carry data composed since exposure (below).
    pub fn composed_ranges(&self, url: &url::Url) -> Vec<std::ops::Range<usize>>;
}
pub struct EgressCaps { pub per_origin_per_turn: u32, pub per_turn: u32, pub per_session: u32 }  // defaults 3, 10, 40 until T28 measures
```

P14's rules as `evaluate` applies them in an exposed turn: a request passes without an ask only when it carries no data
composed since exposure (a GET whose full URL is byte-identical to a URL the user typed, a URL requested before
exposure, or a link present verbatim in content already in context); a verbatim link to an origin not yet contacted in
the session asks, as does every redirect hop to one; links to an origin that supplied untrusted content are capped per
turn (`per_origin_per_turn`), past which they ask; verbatim links in total, approved ones included, are capped per turn
and per session, and reaching either stops the turn; any request with a body asks; a shell command with network asks.
Host history never exempts composed data. Before exposure, normal permissions apply (P60's per-host ask).

P15's image load (`EgressRequest::ImageLoad`): from an image in an exposed turn's output it asks
(`ExposedImageLoad`), even when the current turn is not exposed; otherwise it is the user's own action and passes; unattended it is denied
(only the app can send it). Every load then runs `udx_tools::net::fetch_bytes` (section 3.5), so address
classification, `private_allow` and the caps apply exactly as for `web_fetch`.

How `composed_ranges` marks a URL (P4): the URL's display form is split into components (host labels, each path
segment, each query parameter's name and value, the fragment). Each component that appears identically, at the same
position, in a URL the user typed, a URL requested before exposure or a verbatim link of the same origin is
unmarked; every other component is marked. With no known URL of that origin, the host and everything after it are
marked. The card shows the whole URL with the marked ranges highlighted and never truncates either
(`p4_composed_4kb_url_shown_whole_and_marked` checks it against this rule).

### 3.10 Spend ledger (T17)

```rust
// udx_engine::spend (P40; MicroUsd and SpendRefusal are udx_core types, section 3.1.0)
pub struct SpendLedger { /* rusqlite connection to StatePaths::spend_db, WAL mode, owner-only file */ }
pub struct Caps { pub per_call: MicroUsd, pub per_day: MicroUsd }
pub enum BudgetKey { Day { utc_day: u32 /* days since epoch */ } }   // M6 adds per-routine budgets
impl SpendLedger {
    pub fn open(path: &Path) -> Result<SpendLedger, SpendError>;
    /// Atomic (BEGIN IMMEDIATE): refuses when the worst case passes the per-call cap or the day's remaining budget, so
    /// the app, the CLI and (in M6) run-due draw on one budget.
    pub fn reserve(&self, worst_case: MicroUsd, caps: &Caps, now_ms: u64) -> Result<Reservation, SpendRefusal>;
    /// Settles to the provider-reported usage. A cancelled or retried stream keeps its reservation until usage is
    /// reported; with no report, the reservation is settled at its worst case after `unreported_after` (P40).
    pub fn settle(&self, r: &Reservation, actual: MicroUsd) -> Result<(), SpendError>;
    pub fn today(&self, now_ms: u64) -> Result<DaySummary, SpendError>;
}
pub struct Reservation { pub id: ReservationId, pub amount: MicroUsd, pub utc_day: u32 }
pub struct DaySummary { pub utc_day: u32, pub settled: MicroUsd, pub reserved: MicroUsd, pub cap: MicroUsd }
pub enum SpendError { Sqlite(String), Io(std::io::Error), Guard(GuardError), Busy }
/// Worst case: counted input tokens plus max_output_tokens at the model's price (threat model 5.3). A model with no
/// known price refuses; a loopback endpoint counts as free only when probe() recognised a local model server there or
/// the user marked it free.
pub fn worst_case(price: Option<&Price>, input: TokenCount, max_output: u32) -> Result<MicroUsd, SpendRefusal>;
```

### 3.11 The engine loop (T19, T23), the session log (T16) and the prompt-based protocol (T18)

```rust
// udx_engine
pub struct NativeEngine { /* ... */ }
impl NativeEngine { pub fn new(deps: EngineDeps) -> NativeEngine; }
impl udx_core::engine::Engine for NativeEngine {}
pub struct EngineDeps {
    pub paths: StatePaths, pub config: Arc<Config>, pub providers: ProviderSet, pub router: Router,
    pub tools: Vec<Arc<dyn Tool>>, pub sandbox: Arc<dyn Sandbox>, pub supervisor: Arc<ProcessSupervisor>,
    pub secrets: Arc<dyn SecretStore>, pub ledger: Arc<SpendLedger>, pub trust: Arc<TrustStore>,
    pub clock: Arc<dyn Clock>, pub ids: Arc<dyn IdGen>, pub resolver: Arc<dyn Resolver>,
}
pub struct LoopGuards { pub max_model_calls_per_turn: u32 /* 50 */, pub identical_call_repeats: u32 /* 3 */, pub invalid_call_streak: u32 /* 3 */ }  // D23
pub struct RetryPolicy { pub max_attempts: u32 /* 4 */, pub base: Duration, pub cap: Duration }  // honours retry-after; only before any tool ran for that model call
```

The loop, per turn: route the role (fallback only between turns); assemble context (system blocks, instruction files
when trusted, history with labels); compute the worst case and reserve spend; stream the model call, emitting events; on
tool calls, classify each, evaluate permissions and egress, raise asks one at a time (P58), run approved read-only calls
in parallel and write, edit and shell calls one after another in the model's order; label and redact each result (P21)
and update exposure; settle spend; check prompt integrity (P59); repeat until the model ends its turn or a stop reason
fires. A full context window stops the turn with `ContextFull` (trimming is M2). Cancellation drops the provider stream,
kills running process groups and lets an atomic write finish or not happen.

T19 builds the loop against `testing` fakes (the mock provider, fake tools, `udx_sandbox::testing::FakeSupervisor`). T23
then wires the real tools, sandbox and supervisor into `NativeEngine` and owns the flows that cross those crates: P8's
re-run with a wider profile and the `protected_created` card, P7's unsandboxed-shell grant, P50's asks on exposure and
on a revoked grant, P15's image load, and the plan's review-focus tests.

```rust
// udx_engine::log (T16)
pub struct SessionLogWriter { /* one file, append-only, owner-only, created without following links; every record redacted before it is written */ }
impl SessionLogWriter {
    pub fn create(paths: &StatePaths, ws: &WorkspaceKey, header: SessionHeader, redactor: Arc<RwLock<Redactor>>, env: &dyn Env) -> Result<SessionLogWriter, LogError>;
    pub fn open_append(path: &Path, redactor: Arc<RwLock<Redactor>>, env: &dyn Env) -> Result<SessionLogWriter, LogError>;
    pub fn append(&mut self, record: &SessionRecord) -> Result<(), LogError>;   // one line, fsync per turn end
}
pub struct SessionLogReader { /* ... */ }
impl SessionLogReader {
    pub fn open(path: &Path) -> Result<SessionLogReader, LogError>;
    /// Reads the last `n` message records by scanning backwards from the end of the file, so a 5,000-message thread
    /// shows its end first (the 100 ms open target); the rest is read off the UI thread with `records()`.
    pub fn tail_messages(&mut self, n: usize) -> Result<Vec<SessionRecord>, LogError>;
    pub fn records(&mut self) -> impl Iterator<Item = Result<SessionRecord, LogError>> + '_;
    pub fn header(&self) -> &SessionHeader;
}
// LogError is udx_core::events::LogError (section 4.3).

// udx_engine::protocol (P6), used only for models whose ToolCalling is Prompt
pub struct PromptProtocol { /* delimiters "<<udx-call:{nonce}>>" ... "<<udx-end:{nonce}>>" */ }
impl PromptProtocol {
    pub fn new(nonce: &Delimiters) -> PromptProtocol;
    pub fn system_text(&self, tools: &[ToolSpec]) -> String;
    /// Parses calls only from the assistant's current output.
    pub fn parse(&self, assistant_output: &str) -> Vec<Result<ParsedCall, ProtocolError>>;
    /// Escapes tool-result text so it can never contain a delimiter.
    pub fn escape_result(&self, text: &str) -> String;
}
pub struct ParsedCall { pub id: CallId /* engine-made "udx-<uuid>" */, pub name: String, pub args: serde_json::Value }
pub enum ProtocolError { Unterminated, BadJson(String), UnknownTool(String), NonceMismatch, TooLarge { bytes: u64 } }
```

### 3.12 CLI verbs (T24, T25)

| Verb | What it does | Attendance and threat-model rules |
|---|---|---|
| `unidex chat [--workspace DIR] [--provider ID] [--model M] [--mode read_only\|ask\|auto] [--resume SESSION] [--json]` | Interactive session in the terminal | Requires `Attended(Terminal)`; otherwise refuses with a message pointing to `unidex run`. Output escaped (P49); asks typed from the terminal device (P49); `--json` prints `EventEnvelope`s as NDJSON (section 4), streamed text through `StreamRedactor`, and still asks on the terminal device. A `--mode` wider than the session's starting mode needs a typed confirmation (D29). The CLI never turns off a built-in ask (P47) |
| `unidex run PROMPT [--workspace DIR] [--provider ID] [--model M] [--mode M] [--json]` | One turn, then exit with a status | Attendance detected (section 3.1.2): without a terminal every ask is a deny (P41's test) and a `--mode` wider than the starting mode is refused with exit code 4 (P47); exit code 0 completed, 3 stopped (with the reason), 4 refused, 1 error |
| `unidex config list [--json]` | Every section and row with its source (C1) | |
| `unidex config get KEY` / `set KEY VALUE [--dry-run]` / `unset KEY [--dry-run]` | `--dry-run` prints what the file would become and writes nothing; `unset` restores the default (C1) | `InteractiveOnly` keys need a terminal and a typed confirmation; refused headless and in `--json` without a terminal (P22, P47, P24) |
| `unidex key set PROVIDER [--stdin]` / `key delete PROVIDER` | Stores or removes a provider key in the secret store | The key is read from the terminal without echo, or from standard input with `--stdin`; never from an argument (P23) |
| `unidex trust [DIR] [--revoke] [--list]` | Marks a workspace trusted after showing its instruction files and their hash (P13); `--revoke` also removes the workspace's Windows access entries (section 3.6) | Interactive only, typed confirmation; refusing `<home>`, its parents and the root unless the confirmation word is the longer one P48 asks for |
| `unidex sandbox clean DIR` / `sandbox clean --all` | Removes the Windows sandbox's recorded access entries and AppContainer profiles for one workspace or all (D3; section 3.6); prints what it removed and what failed. On macOS and Linux it reports that there is nothing to clean | Interactive or headless (it only removes access) |
| `unidex doctor [--json] [--online]` | Rows: config (unknown keys warn, C3), paths, secret store, sandbox (with reason and remedy, and whether network grants are available), AppArmor profile (Linux), Windows sandbox access entries (section 3.6), providers (no network unless `--online`), workspace trust | Makes no network call without `--online` (design 2, anti-goals) |
| `unidex --version [--json]` | Unchanged from M0 | |

```rust
// udx_cli::tty (P49)
pub struct Tty { /* the terminal device, opened separately from stdin */ }
impl Tty {
    pub fn open() -> Option<Tty>;
    pub fn discard_pending_input(&mut self) -> std::io::Result<()>;
    pub fn read_word(&mut self, prompt: &DisplayText) -> std::io::Result<String>;
    pub fn read_secret(&mut self, prompt: &str) -> std::io::Result<Secret>;   // echo off through termios or the console mode
}
pub fn print_escaped(out: &mut dyn std::io::Write, text: &str) -> std::io::Result<()>;   // every model and tool text
```

### 3.13 The chat window: view-model boundary, caches and the automation socket (T26, T27, T29 to T32, T34)

The boundary rule (design 5.10): everything a test needs to assert about the chat screen is in `udx-ui-model` and is
tested with no window; `udx-app` only renders that state and turns input into actions.

```rust
// udx_ui_model
pub struct ChatScreen { pub session: Option<SessionVm>, pub sessions: Vec<SessionSummary>, pub composer: ComposerVm,
                        pub cards: CardQueue, pub status: StatusVm, pub notices: NoticeGate, pub banner: Option<Banner> }
impl ChatScreen {
    pub fn new(workspace: WorkspaceVm) -> ChatScreen;
    pub fn apply_event(&mut self, ev: &EventEnvelope) -> Vec<ScreenChange>;
    pub fn apply_action(&mut self, action: ChatAction, now_ms: u64) -> Vec<Effect>;   // effects go to the engine or the opener
    pub fn load_tail(&mut self, records: Vec<SessionRecord>);                       // the 100 ms open target
    pub fn load_rest(&mut self, records: Vec<SessionRecord>);
}
pub enum ChatAction {
    Send, EditComposer(String), Cancel, AnswerAsk { ask: AskId, answer: AskAnswer, at_ms: u64 },
    SetMode(PermissionMode), TrustWorkspace, OpenSession(SessionId), NewSession, OpenWorkspace(PathBuf),
    OpenLink { message: MessageId, link: u32 }, LoadRemoteImage { message: MessageId, image: u32 },
    CopyMessage(MessageId), TurnOffBuiltinAsk(BuiltinAsk), GrantUnsandboxedShell,
}
/// LoadRemoteImage becomes Effect::Engine(SessionCommand::LoadImage { .. }) with the image's full URL (P15; section 3.9);
/// TurnOffBuiltinAsk becomes Effect::Engine(SessionCommand::TurnOffBuiltinAsk { .. }), which the engine accepts only
/// from an Attended(App) session (P47).
pub enum Effect { Engine(SessionCommand), Open(OpenRequest), Clipboard(String), StartSession(SessionOptions), ResumeSession(SessionId) }
pub enum ScreenChange { Inserted(std::ops::Range<usize>), Updated(usize), Removed(std::ops::Range<usize>), CardsChanged, StatusChanged }
/// Types from udx-core only (section 3.1.0). `tool_calling` is how the current model calls tools (design 5.2: "the UI
/// says which mode a model is in").
pub struct StatusVm { pub provider: String, pub model: String, pub tool_calling: ToolCalling, pub mode: PermissionMode,
                      pub sandbox: Availability, pub exposed: bool, pub builtins_off: Vec<BuiltinAsk>, pub spend_today: MicroUsd,
                      pub trust: TrustState }
pub struct WorkspaceVm { pub path: PathBuf, pub key: WorkspaceKey, pub display_name: String, pub trust: TrustState }
pub struct SessionSummary { pub id: SessionId, pub title: String, pub updated_ms: u64, pub messages: u32 }
pub struct ComposerVm { pub text: String, pub can_send: bool, pub disabled_reason: Option<String> }
pub struct Banner { pub kind: BannerKind, pub text: String, pub action: Option<ChatAction> }
pub enum BannerKind { Untrusted, SandboxUnavailable, NetworkGrantsUnavailable, BuiltinAskOff, KeyMissing }
pub struct ToolRowVm { pub call: CallId, pub tool: String, pub summary: DisplayText, pub status: ToolStatus, pub output: Option<String>,
                       pub exposes: bool }
pub struct CardQueue { /* one visible card at a time; P4's guard interval enforced in apply_action from at_ms */ }
pub struct NoticeGate { /* P58: at most one unanswered notice per session; a session sees only its own asks */ }

// udx_ui_model::thread
pub struct SessionVm { pub id: SessionId, pub messages: Vec<MessageVm>, pub streaming: Option<MessageId> }
pub struct MessageVm { pub id: MessageId, pub role: MessageRoleVm, pub body: Arc<MessageBody>, pub tools: Vec<ToolRowVm>, pub origin_badge: Option<String> }
pub enum MessageRoleVm { User, Assistant, Tool, Notice }

// udx_ui_model::markdown (P15; decision 0008's StreamDoc pattern)
pub struct MessageBody { pub blocks: Vec<Block>, pub links: Vec<LinkRef>, pub images: Vec<ImageRef>, pub source_len: usize }
pub enum Block { Paragraph(Vec<Inline>), Heading(u8, Vec<Inline>), Code { lang: Option<String>, text: String }, List { ordered: bool, items: Vec<Vec<Block>> },
                 Quote(Vec<Block>), Table { header: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>> }, Rule, ImagePlaceholder(u32) }
pub enum Inline { Text(String), Code(String), Emphasis(Vec<Inline>), Strong(Vec<Inline>), Link { text: Vec<Inline>, link: u32 } }
pub struct LinkRef { pub url: String, pub display: DisplayText }
pub struct ImageRef { pub url: String, pub display: DisplayText, pub exposed_turn: bool }  // never fetched by the renderer (P15)
pub struct StreamDoc { /* blocks before the last blank line outside a fence parsed once; the open tail parsed again */ }
impl StreamDoc { pub fn push(&mut self, delta: &str) -> BodyDelta; pub fn body(&self) -> Arc<MessageBody>; }
pub enum BodyDelta { TailChanged { from_block: usize }, BlocksAppended(std::ops::Range<usize>) }

// udx_ui_model::cache (the prepared-message cache policy; decision 0008, gate question 4)
pub struct PreparedCache<V> { /* LRU by message id with a byte budget; entries in view or streaming are pinned */ }
impl<V> PreparedCache<V> {
    pub fn new(budget_bytes: u64) -> Self;
    pub fn get(&mut self, id: &MessageId) -> Option<&V>;
    pub fn insert(&mut self, id: MessageId, value: V, bytes: u64);
    pub fn pin(&mut self, ids: &[MessageId]);           // the visible range plus overdraw, and the streaming message
    pub fn evict_to_budget(&mut self) -> Vec<MessageId>;
    pub fn bytes(&self) -> u64;
}
```

```rust
// udx_app (GPUI types appear only here)
/// The swap boundary for gate question 23. Exactly one implementation is compiled in: `PlainRenderer` (our own blocks
/// over GPUI's styled text and the shaped-text cache) by default, `KitRenderer` (the widget kit's markdown view in its
/// cached form) only if the maintainer adopts the kit, behind the cargo feature `kit-renderer`.
pub trait MessageBodyRenderer: 'static {
    type Prepared: 'static;
    fn prepare(&self, body: &MessageBody, width: gpui::Pixels, theme: &Theme, window: &mut gpui::Window, cx: &mut gpui::App) -> Self::Prepared;
    fn apply_delta(&self, prepared: &mut Self::Prepared, body: &MessageBody, delta: &BodyDelta, window: &mut gpui::Window, cx: &mut gpui::App);
    fn element(&self, prepared: &Self::Prepared, message: &MessageId) -> gpui::AnyElement;
    fn approx_bytes(&self, prepared: &Self::Prepared) -> u64;
    fn selected_text(&self, prepared: &Self::Prepared) -> Option<String>;
}

/// The persistent shaped-text cache (gate question 4; D4). Fact (the pinned source): GPUI's LineLayoutCache is
/// crate-private and keeps layouts for the current and previous frame only (gpui-pre 0.3.12,
/// src/text_system/line_layout.rs); WindowTextSystem::shape_text returns owned WrappedLines and WrappedLine::paint is
/// public (src/text_system.rs, src/text_system/line.rs). So the cache is built around GPUI, with no GPUI change.
pub struct ShapedTextCache { /* key: ShapeKey; value: Arc<[gpui::WrappedLine]> */ }
pub struct ShapeKey { pub message: MessageId, pub block: u32, pub width_px: u32 /* device pixels */, pub generation: u32 /* theme and font */ }
impl ShapedTextCache {
    pub fn new(budget_bytes: u64) -> Self;
    pub fn get_or_shape(&mut self, key: ShapeKey, text: &gpui::SharedString, runs: &[gpui::TextRun], font_size: gpui::Pixels, wrap: gpui::Pixels, window: &mut gpui::Window) -> Arc<[gpui::WrappedLine]>;
    pub fn invalidate_width(&mut self, width: gpui::Pixels);
    pub fn invalidate_theme(&mut self);
}

/// App-supplied element ids and bounds (decision 0013: GPUI's accessibility tree has neither in release-mode test builds).
pub struct ElementRegistry { /* id -> (role, label, bounds) recorded in prepaint by a canvas inside each tagged element */ }
pub fn tagged(id: &'static str, role: ElementRole, label: &str, child: impl gpui::IntoElement) -> impl gpui::IntoElement;
pub enum ElementRole { Button, TextInput, List, ListItem, Card, Status, Banner, Link, Image }   // not udx_core::engine::Role
/// Colours, font families with fallbacks and metrics for the light and dark themes (design 5.10).
pub struct Theme { /* ... */ }
impl Theme { pub fn light() -> Theme; pub fn dark() -> Theme; }

/// Software rendering by adapter name or vendor id (decision 0013: GPUI reports WARP as hardware).
pub fn is_software_renderer(adapter_name: &str, vendor_id: Option<u32>) -> bool;
```

Fonts (gate report, section g; decision 0013): the theme names its families with fallbacks; on Linux the list is "DejaVu
Sans", "Noto Sans", "Liberation Sans", then the system's default, and the `.deb` depends on `fonts-dejavu-core`. No font
is bundled in M1 (D17). Not verified: how GPUI's font fallback list behaves on the runner. T29 checks it through GPUI's
text system, not a logger (D13 installs none): `TextSystem::all_font_names` lists the faces present and
`get_font_for_id(resolve_font(..))` names the face chosen for the theme's family (fact: both public in gpui-pre
0.3.12, `src/text_system.rs:290`, `:335`, `:351`), and the test records the result.

Message rows, tool rows, ask cards, the status bar (provider and model, mode switch, sandbox state with its reason, the
exposure badge, the persistent indicator for built-in asks turned off, today's spend, trust state with a Trust button),
the session list and an "Open folder" action (GPUI's path prompt) are the M1 chat window, and the home screen whose
first frame the start-up target times (D25). The status bar also shows how the current model calls tools (native,
prompt protocol, or chat only; design 5.2) and whether commands may have network. Light and dark themes come from the
Electron app's tokens (design 5.10), limited to what this window uses.

**The automation socket** (design 10.3; decision 0013; P37), in `udx_app::automation`, compiled only with the cargo
feature `automation`, which also enables `gpui/test-support` (needed for `render_to_image`):

- Off unless the test build is started with `--automation`; the endpoint's name is written to a file whose path the
  driver passes in `--automation-endpoint-file` (decision 0013's "passed through a file rather than stdout").
- Unix: a socket in a fresh directory created 0700 (creation fails if the name exists), bound with no process-wide umask
  change (the socket is created inside the 0700 directory and then set to 0600), the peer's user id compared on every
  connection (`SO_PEERCRED`, `getpeereid`). Windows: a named pipe with a protected access list for the current user
  only, `FILE_FLAG_FIRST_PIPE_INSTANCE` on every instance, `PIPE_REJECT_REMOTE_CLIENTS`, and the client process's token
  user compared with ours. One client at a time; an idle timeout (30 s) and a per-connection rate limit (100 requests
  per second).
- Line-delimited JSON, one request per line of at most 64 KiB. Verbs: `hello`, `state` (the `ChatScreen` snapshot as
  JSON), `elements` (the registry), `tree` (GPUI's accessibility tree), `a11y_activate`, `click {id}`,
  `type {id, text}`, `key {keystroke}`, `action {name, args}` (a fixed list of `ChatAction`s by name),
  `scroll {id, dy}`, `frame` (base64 PNG; on Wayland the error `no_inapp_readback`), `wait_idle {timeout_ms}`,
  `counters` (our idle wake-up counter and frames drawn), `quit`. Anything else is refused; no verb takes a path or runs
  a program.
- The test build also accepts `--provider-script FILE` (the scripted mock provider) and `--state-root DIR` (a test
  root); release builds refuse both, and `--automation`, with exit code 2. The CLI's test-only flag
  `--test-provider-script FILE` (T24) follows the same rule.
- Screenshot baselines on macOS (design 10.1): the `ui (macos)` job compares the chat window's frame, at registry
  bounds, with committed baseline images (`crates/udx-app/tests/e2e/baselines/macos/*.png`, synthetic content only)
  within a per-pixel tolerance and a mismatch budget; a pull request that changes a screen updates the baseline and
  attaches the frame and the difference (T32). Other platforms run decision 0013's frame check (colours and text
  presence at registry bounds) without a baseline.
- Release builds carry no marker string `UNIDEX_AUTOMATION_SOCKET_v1`; the release check (section 3.15) fails if it is
  present in any packaged binary.

**Perf instrumentation**, compiled only with the cargo feature `perf`: timestamps of start-up milestones, presented
frames and message renders written to the file named by `UNIDEX_PERF_OUT`, and scripted scenarios (`--perf-scenario
scroll|open-thread|first-token|idle`) driven from the next-frame callback (decision 0008's method). The perf build is
exactly `cargo build --release --locked -p udx-app --bin unidex-rust-preview --features perf`, where `perf` enables
`gpui/profiler`, `udx-providers/testing` (the scripted mock provider the first-token scenario needs) and
`udx-core/testing`. Because `udx-core/testing` refuses the OS folders, the perf build runs only with `--state-root DIR`
(a folder the launcher creates and removes). It therefore differs from the shipped binary by those three features, not
one; the gate report says so beside every figure, and the release check refuses all three features' markers. Not
verified: the profiler's present-time API at the pin; T34 verifies.

**The start-up launcher** (T35): `cargo xtask perf-launch <binary> --runs 10 --warmup 1` takes a clock reading
immediately before spawning the perf build with `--perf-scenario idle --state-root <fresh folder>`, reads the first
presented frame's timestamp from the recorder's file (same clock, monotonic), and reports the median and range of
spawn to first presented frame; the first launch after a build is reported separately.

**Download size on every change** (design 9): the `perf-checks (size)` job, on the macOS arm64 runner for every pull
request, builds the release app and CLI binaries (no feature), puts them in a `tar` and compresses it with zlib at the
level the `.dmg` uses (UDZO), and fails at 36,000,000 bytes: the 40,000,000-byte target less a 4,000,000-byte margin
for the disk image's own overhead and the bundle's other files. Not verified: how closely this proxy tracks the real
`.dmg`; T35 calibrates it against the nightly's `.dmg` and records the ratio, and the nightly's own check of the real
`.dmg` (T33) stays as the backstop.

### 3.14 `udx-test-support` (T1b creates it; T2a fills the stand-in)

```rust
pub struct TestRoot { /* a fresh folder inside UNIDEX_TEST_ROOT, removed on drop; panics if the variable is missing */ }
impl TestRoot { pub fn new() -> TestRoot; pub fn path(&self) -> &Path; pub fn child(&self, name: &str) -> PathBuf; }
pub struct TempHome { /* a fake home under a TestRoot with planted files: .bash_history, .ssh/id_ed25519, a token file, .zshrc */ }
impl TempHome { pub fn new(root: &TestRoot) -> TempHome; pub fn plant(&self, rel: &str, contents: &[u8]) -> PathBuf; pub fn path(&self) -> &Path; }
/// A recording HTTP/1.1 stand-in on 127.0.0.1 with scripted responses (redirects, huge or compressed bodies, slow
/// bodies). It records every request it receives, so a test can assert that none was made (P15, design 4.6).
pub struct HttpStandIn { /* ... */ }
impl HttpStandIn {
    pub async fn start(routes: Vec<StandInRoute>) -> HttpStandIn;
    /// The same over TLS with a certificate made at run time (no key material is ever committed: the deny list and the
    /// secret scan would refuse it). Uses `rcgen` and `tokio-rustls` (section 2.3).
    pub async fn start_tls(routes: Vec<StandInRoute>, cert: &TestCert) -> HttpStandIn;
    pub fn url(&self, path: &str) -> String;
    pub fn requests(&self) -> Vec<RecordedRequest>;
}
pub struct StandInRoute { pub path: String, pub response: StandInResponse }
pub enum StandInResponse { Body { status: u16, content_type: String, body: Vec<u8> }, Redirect { status: u16, location: String },
                           Slow { bytes_per_s: u32, body: Vec<u8> }, Gzip { body: Vec<u8> }, Huge { bytes: u64 } }
pub struct RecordedRequest { pub method: String, pub path: String, pub headers: Vec<(String, String)>, pub body: Vec<u8> }
/// A test certificate authority and a leaf for 127.0.0.1 and `localhost`, generated per test; `ca_pem()` is written to
/// a file in the test root for the clients that need it (section 3.6, "The networked https check").
pub struct TestCert { /* ... */ }
impl TestCert { pub fn generate() -> TestCert; pub fn ca_pem(&self) -> String; }
/// Scans every file under a folder for a value and its base64, percent and hex encodings (threat model 4.5's test).
pub fn scan_for_canary(root: &Path, canary: &str) -> Vec<PathBuf>;
```

### 3.15 The release check (T33) and the evaluation harness (T28)

```rust
// xtask release-check <artifact> [--platform macos|linux|windows]
// Extracts the program(s) from a .dmg (hdiutil), .deb (dpkg-deb), .tar.gz or NSIS installer (7z) and fails when:
//  - any marker string UNIDEX_AUTOMATION_SOCKET_v1, UNIDEX_PERF_MARKER_v1 or UNIDEX_TESTING_MARKER_* is present;
//  - the program accepts --automation, --provider-script, --state-root, --perf-scenario or (the CLI)
//    --test-provider-script (exit code must be 2);
//  - macOS: entitlements differ from packaging/macos/entitlements.toml (empty while builds are unsigned).
// Each format is checked only on its own CI platform (hdiutil on macOS, dpkg-deb on Linux, 7z on Windows); attaching a
// disk image happens only on a CI runner, never on the development Mac (section 3.6, "Where tests run").
```

The markers are `#[used]` statics; whether they survive the release profile's linking and section garbage collection
is not verified, so the release check's positive control is a release-profile build with each feature on (not a test
build): the check must find each marker there, or the check itself fails. A marker that does not survive is replaced by
a reference the binary cannot drop (for example a string printed by the feature's own code path) before T33 merges.

```rust
// udx_evals
pub struct Task { pub id: String, pub kind: TaskKind, pub prompt: String, pub workspace: PathBuf /* a template copied into a TestRoot */,
                  pub checks: Vec<Check>, pub max_turn_calls: u32, pub budget: MicroUsd }
pub enum TaskKind { Edit, FixTest, MultiStep, Refusal, InjectionCanary }
pub enum Check { FileEquals { path: String, contents: String }, FileContains { path: String, needle: String },
                 CommandSucceeds { argv: Vec<String> } /* run in the sandbox */, NoCanaryLeft { canary: String }, PathUnchanged(String), AskCount { max: u32 } }
pub struct RunConfig { pub provider: ProviderSel, pub model: String, pub budget: MicroUsd, pub out_dir: PathBuf, pub repeats: u32 }
pub enum ProviderSel { Mock /* replays tasks/<id>/mock-script.json */, Configured(ProviderId) }
pub struct TaskResult { pub id: String, pub passed: bool, pub invalid_tool_calls: u32, pub model_calls: u32, pub turns: u32,
                        pub cost: MicroUsd, pub asks: u32, pub egress_asks: u32, pub stopped: Option<StopReason>, pub log: PathBuf }
pub fn run_suite(suite: &Path, cfg: &RunConfig) -> Result<SuiteReport, EvalError>;  // writes results.json and a markdown table
pub struct SuiteReport { pub results: Vec<TaskResult>, pub passed: u32, pub total: u32, pub skipped: Vec<(String, String /* reason */)>, pub cost: MicroUsd }
pub enum EvalError { Task { id: String, reason: String }, Io(std::io::Error), Engine(EngineError), Budget { spent: MicroUsd } }
```

The M1 task set (12 tasks in `crates/udx-evals/tasks/`): three edits, two failing tests to fix (shell scripts as the
test runner, so no toolchain is needed inside the sandbox), two multi-step changes, two refusals (a request to read a
credential file; a request to push), and three injection canaries (a fetched stand-in page asking to send a canary, a
planted `node_modules` README in a trusted workspace, an untrusted workspace's `AGENTS.md`). The canary tasks double as
the measurement that sets P14's cap numbers (asks per task, T28).

## 4. Event types

Two families, both in `udx_core::events`, both JSON, both versioned, both golden-tested (design 10.1):

- `EventEnvelope` / `EngineEvent`: the live stream a session emits to the app, the CLI (`--json`) and the evaluation
  harness. Not stored as such.
- `SessionRecord`: the lines of the session log. Every durable fact of a session is a record; streaming deltas are not
  (the completed message is).

### 4.1 Serialization and versioning rules

- One JSON object per line (NDJSON), UTF-8. Every envelope and every record carries `"v": <u32>`. M1 writes `v = 1`
  for both families.
- **Representation per enum family.** serde's internal tagging cannot hold a newtype variant around a primitive or a
  unit enum, and two nested internally tagged enums would both write `"type"`, so the shape is fixed per family:
  - Records and events (`SessionRecord`, `EngineEvent`): `#[serde(tag = "type", rename_all = "snake_case")]`, every
    variant a struct variant or a newtype around a struct (`SessionRecord::Session(SessionHeader)`), never around a
    primitive or an enum.
  - Value enums nested in them, with any variant that carries data (`StopReason`, `ToolStatus`, `CloseReason`,
    `DecidedBy`, `RuleSource`, `Attendance`, `SpendRefusal`, `EgressVerdict`, `TrustState`, `Origin`, `BlockRecord`,
    `AskAnswer`, `AnsweredBy`, `Availability`, `UnavailableReason`): adjacently tagged, `#[serde(tag = "kind", content =
    "value", rename_all = "snake_case")]`; a unit variant is `{"kind": "cancelled"}`, a newtype `{"kind": "error",
    "value": "..."}`, and a nested value enum sits inside `"value"`, so no two tags collide.
  - Unit-only enums (`PermissionMode`, `Decision`, `BuiltinAsk`, `OutStream`, `Surface`, `CapScope`, `LimitKind`,
    `WarningCode`, `ToolCalling`, `Backend`): plain snake-case strings.
  - `Unknown { raw }` is not a serde variant. The reader parses each line to a `serde_json::Value`, reads `"v"` and
    `"type"`, and builds `Unknown { raw }` when the type is not one it knows; the variant is
    `#[serde(skip)]`, and the writer never writes it (the log is append-only, so a line once written is never written
    again).
- Readers accept any `v` up to the newest they know and ignore unknown fields (no `deny_unknown_fields`). A record with
  `v` above the newest known is refused with `LogError::NewerVersion`; a known type whose body does not parse is
  `LogError::Corrupt` with its line.
- Additive changes keep `v`: a new record or event type, or a new optional field written with
  `#[serde(default, skip_serializing_if = "Option::is_none")]` (or `Vec::is_empty` for lists), so that no existing
  golden file changes. Removing or renaming a field, changing a field's meaning, or adding a variant to a nested value
  enum raises `v` (an older reader could not parse the record otherwise); the reader keeps a decoder for every earlier
  version, and the writer writes only the newest.
- Golden files: `crates/udx-core/tests/golden/events/v1/*.ndjson` and `.../session-log/v1/*.jsonl`, produced with a
  fixed clock and sequential ids, and with synthetic paths only (`WorkspaceRef.path` is `/work/example` or
  `C:\work\example`, never a machine's path). A change that alters an existing golden file fails review unless it raises
  `v` and adds a new folder; golden files of an older version are never edited.
- Strings that came from a model, a tool or the user are stored as they are (after redaction), not escaped; escaping is
  a display concern (section 3.1.10). One exception: `AskCard`'s `DisplayText` fields are stored in their escaped
  display form, because the log records exactly what the card showed the user.
- Redaction (design 5.11; P57): `SessionLogWriter::append` passes every record through `Redactor::redact_json` before
  serializing; the CLI's `--json` output and the automation socket's `state` pass through the same redactor, and
  streamed text through `StreamRedactor` (section 3.1.7). An opaque reasoning block that the redactor would change is
  dropped, not edited (an edited block would break the provider's signature on resume): the log keeps a
  `BlockRecord::ReasoningDropped` in its place, resume sends nothing for it, and the turn emits a warning.

### 4.2 `EngineEvent`

```rust
pub struct EventEnvelope { pub v: u32, pub session: SessionId, pub seq: u64, pub ts_ms: u64, pub event: EngineEvent }

pub enum EngineEvent {
    SessionStarted { workspace: WorkspaceRef, attendance: Attendance, trust: TrustState, sandbox: AvailabilitySummary, mode: PermissionMode },
    TurnStarted { turn: TurnId, route: RouteSummary },
    ContextAssembled { turn: TurnId, items: Vec<ContextItemSummary>, exposed: bool, input_tokens: TokenCountSummary },
    ExposureChanged { turn: TurnId, exposed: bool, origin: OriginSummary },
    TextDelta { turn: TurnId, message: MessageId, text: String },
    ReasoningDelta { turn: TurnId, message: MessageId, text: String },
    MessageCompleted { turn: TurnId, message: MessageId, blocks: Vec<BlockRecord> },
    ToolCallProposed { turn: TurnId, call: CallId, tool: String, args: serde_json::Value /* redacted */, intent: IntentSummary },
    PermissionDecided { turn: TurnId, call: CallId, decision: Decision, by: DecidedBy, builtin: Option<BuiltinAsk>, exposed: bool },
    AskRaised { turn: TurnId, call: Option<CallId>, card: AskCard },
    AskAnswered { turn: TurnId, ask: AskId, answer: AskAnswer, by: AnsweredBy },
    ToolStarted { turn: TurnId, call: CallId, pid: Option<u32> },
    ToolOutputDelta { turn: TurnId, call: CallId, stream: OutStream, text: String },
    ToolFinished { turn: TurnId, call: CallId, status: ToolStatus, origin: OriginSummary, truncated: bool, exposes: bool },
    ProcessesChanged { live: Vec<ProcessInfo> },
    SpendReserved { turn: TurnId, reservation: ReservationId, worst_case: MicroUsd },
    SpendSettled { turn: TurnId, reservation: ReservationId, actual: MicroUsd, usage: UsageSummary },
    Retrying { turn: TurnId, attempt: u32, after_ms: u64, reason: String },
    TurnStopped { turn: TurnId, reason: StopReason },
    TurnCompleted { turn: TurnId, usage: UsageSummary, cost: MicroUsd },
    SessionClosed { reason: CloseReason },
    Warning { code: WarningCode, message: String },
    /// P15: the result of SessionCommand::LoadImage. `data` is the image's bytes as standard base64 in JSON. Not stored
    /// in the session log (a Permission record is).
    ImageLoaded { message: MessageId, image: u32, media_type: String, data: ImageData },
    ImageRefused { message: MessageId, image: u32, reason: String },
}
pub struct ImageData(pub Vec<u8>);   // serialized as a base64 string
pub enum StopReason {
    Cancelled, ContextFull, MaxModelCalls { limit: u32 }, RepeatedCall { tool: String, repeats: u32 }, InvalidCallStreak { count: u32 },
    DenialLimit { denials: u32 },                      // P58
    EgressCap { scope: CapScope },                     // P14
    PromptCut { reported: u64, expected_at_least: u64 },   // P59
    Spend(SpendRefusal),                               // P40
    FreeSpaceFloor { free: u64, floor: u64 },          // P51
    Provider { error: String, retryable: bool },
}
pub enum AnsweredBy { User, UnattendedDefault, Timeout, RateLimited }
pub enum ToolStatus { Ok, Error(String), Refused(String), Cancelled }
pub enum CloseReason { User, Error(String), AppQuit, UnattendedRunEnded }
pub enum WarningCode { SandboxUnavailable, NetworkGrantsUnavailable, BuiltinAskOff, InstructionFilesNotLoaded,
                       PromptIntegrityUnknown, KeyMissing, ReasoningBlockDropped, DenyScanIncomplete }

// The summary types: small serializable views that never carry secret values or full file contents.
pub struct AvailabilitySummary { pub backend: Backend, pub full: bool, pub reason: Option<UnavailableReason>, pub network_grants: bool }
pub struct RouteSummary { pub provider: ProviderId, pub model: ModelId, pub tool_calling: ToolCalling, pub reason: String }
pub struct ContextItemSummary { pub origin: OriginSummary, pub tokens_upper_bound: u64 }
pub struct TokenCountSummary { pub tokens: u64, pub exact: bool }
pub struct OriginSummary { pub kind: OriginKind, pub label: String /* path, URL or command, redacted */, pub exposes: bool }
pub enum OriginKind { User, App, File, InstructionFile, Web, Shell, Image }
pub struct IntentSummary { pub kind: IntentKind, pub subject: String /* redacted */, pub network: bool }
pub enum IntentKind { Read, Write, Edit, Glob, Grep, Command, Fetch, Todo, ImageLoad }
pub struct UsageSummary { pub input_tokens: Option<u64>, pub output_tokens: Option<u64>, pub cache_read_tokens: Option<u64>,
                          pub cache_write_tokens: Option<u64> }
pub struct ExitSummary { pub code: Option<i32>, pub signal: Option<i32>, pub timed_out: bool, pub limits_hit: Vec<LimitKind>,
                         pub denied_writes: Vec<PathBuf>, pub protected_created: Vec<PathBuf> }
pub struct GrantSummary { pub kind: GrantKind, pub vars: Vec<String> /* names only */ }
pub enum GrantKind { UseMyCredentials, ShellNetwork, UnsandboxedShell }
```

### 4.3 `SessionRecord` (the session log, version 1)

File: `StatePaths::session_log(ws, id)`, that is `<data_dir>/sessions/<workspace-key>/<session-id>.jsonl`, created
owner-only without following links (P39), written only through the file guard's private-file helper and checked by
`check_test_path` (so a test's logs land in its test root). The first line is always the header.

```rust
pub struct RecordEnvelope { pub v: u32, pub seq: u64, pub ts_ms: u64, pub record: SessionRecord }

pub enum SessionRecord {
    Session(SessionHeader),
    UserMessage { message: MessageId, text: String },
    TurnStarted { turn: TurnId, provider: String, model: String, mode: PermissionMode, attendance: Attendance },
    Context { turn: TurnId, items: Vec<ContextItemSummary>, instruction_hash: Option<String>, exposed: bool },
    AssistantMessage { turn: TurnId, message: MessageId, blocks: Vec<BlockRecord> },
    ToolCall { turn: TurnId, call: CallId, tool: String, args: serde_json::Value },
    /// Threat model 5: every permission decision, with the tool, its redacted arguments, the rule matched, the
    /// decision, who decided and whether the turn was exposed.
    Permission { turn: TurnId, call: CallId, tool: String, args: serde_json::Value, rule: Option<RuleRef>, decision: Decision,
                 by: DecidedBy, builtin: Option<BuiltinAsk>, exposed: bool },
    Ask { turn: TurnId, ask: AskId, call: Option<CallId>, card: AskCard },
    AskAnswer { turn: TurnId, ask: AskId, answer: AskAnswer, by: AnsweredBy },
    ToolResult { turn: TurnId, call: CallId, status: ToolStatus, content: String /* unlabelled, redacted, capped */, origin: OriginSummary,
                 truncated: bool, exposes: bool, exit: Option<ExitSummary> },
    Exposure { turn: TurnId, exposed: bool, origin: OriginSummary },
    Spend { turn: TurnId, reservation: ReservationId, phase: SpendPhase, amount: MicroUsd, usage: Option<UsageSummary> },
    Todo { items: Vec<TodoItem> },
    Grant { grant: GrantSummary, granted: bool },
    BuiltinAskOff { name: BuiltinAsk },
    Availability { sandbox: AvailabilitySummary },
    TurnStopped { turn: TurnId, reason: StopReason },
    TurnCompleted { turn: TurnId, usage: UsageSummary, cost: MicroUsd },
    Closed { reason: CloseReason },
    Unknown { raw: serde_json::Value },   // read side only (section 4.1)
}
/// No nonce or nonce hash: tool results are stored unlabelled, so the log holds no delimiter, and every run of a session
/// (new or resumed) draws a fresh nonce.
pub struct SessionHeader { pub session: SessionId, pub created_ms: u64, pub app_version: String, pub workspace: WorkspaceRef,
                           pub engine: EngineKind }
pub enum BlockRecord { Text { text: String }, Reasoning { provider: String, data: serde_json::Value }, ReasoningDropped { provider: String },
                       ToolUse { call: CallId, name: String, input: serde_json::Value } }
pub enum SpendPhase { Reserved, Settled, SettledAtWorstCase }
pub enum LogError { Io(std::io::Error), Guard(GuardError), Corrupt { line: u64, reason: String }, NewerVersion { found: u32, supported: u32 },
                    FreeSpaceFloor, LineTooLong { line: u64 } }
```

Resume rebuilds the provider messages from `UserMessage`, `AssistantMessage` (opaque reasoning blocks verbatim; a
dropped block sends nothing) and `ToolResult` records, labelling each result again with the resumed run's fresh
delimiters, and the exposure state from `Exposure` records; `Context` records are summaries, not content. A
reservation left `Reserved` by a crash is settled at its worst case after `unreported_after` (section 3.10).

## 5. Which providers M1 implements

**Proposal (D7): the hosted provider is Anthropic's Messages API; the local provider is Ollama's native API.**

Reasons:

- **Two different wire protocols, cheaply.** Anthropic's streaming is server-sent events with typed content blocks;
  Ollama's native streaming is newline-delimited JSON. Covering both proves the `Provider` abstraction and the
  conformance suite on unlike formats in M1, which is exactly what M2's breadth needs (design 6).
- **What M1's controls need from a local server.** Ollama's native API takes the context window the engine sets
  (`options.num_ctx`, design 5.2's "the engine sets the context size it needs") and reports prompt token counts, which
  P59 needs; its `/api/version` gives P40 a way to recognise a local model server on a loopback endpoint, so it can
  count as free without the user marking it (threat model 5.3). The OpenAI-compatible endpoints the other local servers
  offer do not take a context size in the request (not verified for each server); that client arrives in M2.
- **Vendor terms.** The gate report (question 6 and section g) records that OpenAI's usage policies change on 2026-11-20
  with a clause on services that let third parties reach OpenAI models, which its review noted may bear on UniDeX's own
  engine calling OpenAI with the user's key, and asks for a re-read if M1's hosted provider is OpenAI. Choosing
  Anthropic for M1 removes that dependency from M1's critical path. Anthropic's pages, as decision 0006 read them,
  direct third-party tools to API keys, which is exactly how the native engine calls the Messages API (design 3). This
  is an engineering reading, not legal advice (decision 0006's own caveat).
- **Opaque reasoning.** Anthropic returns reasoning blocks that must be sent back unchanged (design 5.2's "reasoning
  state"); building that path in M1 fixes the session-log format for it now (section 4.3 `BlockRecord::Reasoning`)
  rather than raising `v` in M2.
- **No test needs a real key or a running model.** Both clients run against recorded HTTP fixtures through
  `FixtureTransport`; the engine, tools, app and evaluation harness run against the scripted mock provider; live tests
  are opt-in (`UNIDEX_LIVE_TESTS=1` and a key or a running server), never on CI or forks.

What it costs if wrong: if Ollama is the wrong local choice (for example most users run llama.cpp's server or LM
Studio), M2's OpenAI-compatible client covers them, and nothing in M1 is lost except that P59's check would need a
second implementation there. If Anthropic were unusable (terms or price), the M1 gate's hosted evaluation run would wait
for M2's next provider; the engine, permissions and app do not depend on the choice. Prices: the built-in price table
holds only prices the builder reads from the vendor's published page, with the date; a model without one refuses calls
until the user sets a price (P40), so a stale or missing table cannot overspend.

## 6. Threat-model coverage

One row per accepted proposal. "Test" names are the test functions the owning task writes (file `tests/<name>.rs` or a
`#[test]` of that name in the crate shown); "CI" says where each runs. All 31 have an owner and at least one test.
"CI only" marks a test that changes OS state if the control failed; it runs behind `UNIDEX_TEST_OS_SIDE_EFFECTS` on CI
runners and is skipped with a notice on the development Mac (section 3.6, "Where tests run"). A residual is a part of
a proposal that cannot be fully met on a platform; each has a test pinning today's behaviour and is listed for the
threat-model update (T40).

| Proposal | Crate and interface | Tests (where they run) | Task |
|---|---|---|---|
| P1 No network for sandboxed commands | `udx-sandbox` backends; `NetworkGrant`; `Rule::network`; probe `LoopbackConnect`, `NameLookup` | `p1_no_loopback_connect`, `p1_no_name_lookup` (stub resolver and system resolver socket) on macOS, Linux (`sandbox-linux`), Windows; `p1_rule_network_shown_on_card_and_scoped_to_command` | T10, T11, T12, T13 |
| P2 Allowed commands as parsed argv | `udx_tools::parse_command`, `CommandShape`, `CommandProgram::Direct`; `ArgvPattern`; runner card | `p2_tar_checkpoint_leaves_no_marker`, `p2_compound_around_allowed_prefix_asks`, `p2_zsh_glob_qualifier_never_under_rule`, `p2_windows_every_command_asks`, `p2_rule_wildcard_never_matches_option`, `p2_runner_card_says_repository_code`, `p2_direct_skips_relative_and_empty_path_entries` (all platforms; Windows case on Windows) | T13 (matching, card), T21 (execution) |
| P3 Built-in asks; undo | `BuiltinAsk` (P3 set) and its conservative matcher (3.7); `UndoStore` | `p3_force_push_asks_in_auto_mode`, `p3_force_push_denied_unattended`, `p3_hostile_spellings_ask` (table: `env git push --force`, `nice git push`, `timeout 5 git push`, `git -C . push`, `/usr/bin/git push`, `git -c alias.p=push p`, `find . -delete`, `xargs rm -rf`, and more), `p3_overwrite_undone_byte_for_byte`, `p3_edit_undone_byte_for_byte`, `p3_no_remove_dir_all_outside_tests` (choke-point test). P3's "a folder delete lands in the trash" test is deferred with the delete tool (D14) | T13, T5, T1b |
| P4 Ask cards in the app's words | `AskCard`; `udx_core::display`; `CardQueue`; `EgressState::composed_ranges` | `p4_bidi_override_renders_escaped`, `p4_mixed_script_host_shown_ascii`, `p4_return_inside_guard_approves_nothing`, `p4_exposure_badge_after_mock_fetch`, `p4_composed_4kb_url_shown_whole_and_marked`, `p4_no_default_choice` (view-model, all platforms); `composed_ranges_marks_changed_components` (T15); `e2e_ui_card_guard_interval` (automation, three platforms) | T27 (T13 builds the card data, T15 the ranges, T32 the UI test) |
| P6 Random delimiters in the prompt protocol | `PromptProtocol` | `p6_fuzz_result_text_never_yields_call` (proptest), `p6_calls_parsed_only_from_current_output`, `p6_nonce_differs_per_session`, `p6_resumed_session_gets_fresh_nonce` | T18, T19 |
| P7 Shell off unless the full sandbox applies | `Availability`, `NetworkGrants`; shell tool gating; `SessionGrant::UnsandboxedShell` | `p7_forced_unavailable_shell_absent_and_refuses`, `p7_unattended_cannot_enable_unsandboxed`, `p7_linux_restricted_userns_without_profile_is_unavailable` (`sandbox-linux`, sysctl set, no profile), `p7_probe_check_allowed_makes_unavailable`, `p7_network_grants_unavailable_below_landlock_abi_6` (Linux; a test build reporting ABI 5), `p7_networked_check_allowed_disables_network_grants_only` | T21, T23, T11, T10 |
| P43 Sandbox profiles deny IPC | backends' profiles, with and without network (3.6); probe `IpcEscape`, `IpcEscapeNetworked` | `p43_escape_matrix_<platform>` (macOS: `open`, `osascript`, `launchctl`, `lsopen`; Linux: `systemd-run --user`, `gio open`, X11 by pathname and abstract socket, `TIOCSTI`, a parent-namespace abstract socket; Windows: `schtasks`, WMI, COM), `p43_escape_matrix_networked_<platform>` (macOS: a stand-in Unix socket outside the workspace; Linux: X11 abstract and pathname, a parent-namespace abstract socket, a stand-in Unix socket outside the workspace; Windows: a stand-in named pipe, `schtasks`, WMI, COM), `p43_networked_https_ok_secret_store_denied_<platform>` (3.6), all CI only. Residual P43-a (Linux: a pathname socket created inside the workspace from outside is reachable) pinned by `p43_workspace_socket_from_outside_reachable_residual` | T10, T11, T12 |
| P47 Rule sources and precedence | `PermissionEngine::evaluate`, `rule_for_always`, `turn_off_builtin`, `set_mode`; config `InteractiveOnly` | `p47_workspace_allow_changes_nothing`, `p47_allow_always_npm_test_does_not_admit_npm_exec`, `p47_user_allow_git_push_still_asks`, `p47_builtin_off_for_session_with_indicator_then_asks_again` (engine and view-model), `p47_permission_widening_refused_headless`, `p47_mode_widening_refused_headless` (config key, `--mode` without a terminal, `SetMode` unattended), `p47_cli_cannot_turn_off_builtin` (engine refuses `Attended(Terminal)`; the CLI has no verb). Its MCP clause has nothing to start in M1 (no MCP client); the `.mcp.json` write is covered by P8 | T13, T3, T24, T25, T27 |
| P49 CLI escapes; typed-word asks | `udx_cli::tty` | `p49_prefilled_stdin_approves_nothing`, `p49_cursor_movement_in_output_printed_escaped`, `p49_single_key_is_not_an_answer` (Unix pty in tests; Windows console where the runner allows, else the logic unit-tested) | T24 |
| P50 Session-owned process groups | `KillHandle`, `ProcessRegistry`, `ProcessSupervisor`, `EngineEvent::ProcessesChanged` | `p50_double_fork_setsid_gone_at_run_end` (Linux, Windows), `p50_macos_group_killed_and_marker_sweep_kills_daemon` (macOS, CI only), `p50_live_processes_listed` (three platforms, the same lifetime everywhere), `p50_kill_handle_works_while_wait_pending`, `p50_exposure_asks_about_networked_processes`, `p50_revoke_asks_about_processes_holding_grant` | T10, T11, T12, T23 |
| P51 Limits and a free-space floor | `Limits`, `LimitKind`; `fsguard::space`; output cap; `ProcessSupervisor::watch_free_space` | `p51_fork_bomb_contained` (Linux, Windows), `p51_macos_process_limit_reads_back_count_plus_margin`, `p51_output_cut_with_marker`, `p51_store_refuses_below_floor`, `p51_unattended_fill_kills_group_with_event` (test volume: a sized tmpfs on Linux; a small disk image on macOS, CI only). Residuals for T40: no memory limit on macOS (P51-a, pinned by `p51_macos_memory_limit_absent_reported`); no fill test on Windows (P51-b, skipped with a notice) | T10, T11, T12, T21, T5 |
| P58 Rate-limited asks; denial stop | `AskLimiter`; `NoticeGate` | `p58_turn_stops_after_denials_with_event`, `p58_second_unanswered_notice_not_raised`, `p58_session_lists_only_own_asks`. Its notification-title clause has no notification in M1 (notify is M6) | T13, T27 |
| P8 Protected paths in the workspace | `Verdict::Protected`; `SandboxProfile::write_deny` per backend (3.6); `ExitInfo::{denied_writes, protected_created}`; re-run flow | `p8_sandbox_write_to_git_hooks_fails_commit_succeeds` (three platforms), `p8_sandbox_write_to_absent_protected_path` (macOS denied; Linux and Windows reported and carded), `p8_rerun_wider_only_after_card` (macOS where denials are reported; elsewhere `p8_no_rerun_offer_without_reported_denial`), `p8_file_tool_write_mcp_json_asks`, `p8_file_tools_never_create_links_or_exec_bits`, `p8_other_agents_instruction_files_protected`. Residuals P8-a (absent paths, Linux and Windows) and P8-b (re-run offer only where denials are reported) | T5, T10 to T12, T23 |
| P9 The guard checks the handle | `FileGuard::open_read`, `write_atomic` | `p9_swap_race_never_returns_target` (helper thread swapping folder and symlink), `p9_windows_forms_refused_without_network` (Windows), `p9_short_name_expanded_before_compare` (Windows), `p9_write_through_symlink_lands_on_target_keeping_mode_and_xattrs`, `p9_hard_linked_other_name_keeps_old_content`, `guard_property_paths_never_escape` (proptest: case variants, NFC and NFD, `..`, link chains, junctions on Windows) | T5 |
| P10 An allow list for the sandbox's reads | `fsguard::read_allow`; `SandboxProfile::{read_allow, read_deny}` per backend (3.6) | `p10_planted_bash_history_unreadable`, `p10_unlisted_token_file_unreadable`, `p10_session_logs_unreadable_from_sandbox`, `p10_sandbox_read_of_in_tree_deny_listed_file_fails` (three platforms), `p10_deny_scan_past_cap_makes_commands_ask` (Linux, Windows). Residual P10-a (a deny-listed file created during a command is unmasked until the next) | T5, T10 to T12 |
| P48 Workspace edges | `check_workspace_root`; persistence list | `p48_home_as_workspace_needs_typed_confirmation`, `p48_sandbox_writes_to_startup_launch_agent_systemd_fail`, `p48_worktree_common_hooks_write_fails`, `p48_missing_hooks_path_target_reported`, `p48_app_config_write_denied_to_tools` | T5, T10 to T12, T24 |
| P13 Workspace trust | `TrustStore`; `resolve_instruction_set`; `is_third_party` | `p13_untrusted_agents_md_absent_from_system`, `p13_untrusted_file_read_exposes`, `p13_trusted_import_change_asks_again`, `p13_trusted_node_modules_read_exposes`, `p13_trust_refused_headless` | T14, T24 |
| P14 Egress after exposure | `EgressState::evaluate`; `EgressGate` | `p14_composed_path_to_injecting_host_asks`, `p14_canary_subdomain_navigation_asks`, `p14_redirect_to_new_origin_asks`, `p14_verbatim_link_to_injecting_origin_passes_until_cap`, `p14_distinct_subdomains_each_ask_before_lookup` (stub resolver sees no lookup before the answer), `p14_turn_total_stops_turn`, `p14_session_total_stops_later_turn`, `p14_body_always_asks`, `p14_shell_network_while_exposed_asks`, `p14_unattended_exposed_egress_denied`. MCP clauses have nothing to call in M1 | T15, T22 |
| P15 The renderer fetches nothing | `markdown::ImageRef`; `ChatAction::LoadRemoteImage`, `SessionCommand::LoadImage`, `EgressRequest::ImageLoad`, `fetch_bytes`; opener | `p15_remote_image_makes_no_request` (HTTP stand-in records none), `p15_exposed_image_load_raises_card_with_full_url`, `p15_image_load_goes_through_address_policy` (a loopback or metadata address is refused), `p15_link_opens_only_through_opener` | T26, T27, T22, T23, T38 |
| P54 One opener | `udx_core::opener::open` | `p54_file_and_custom_schemes_refused`, `p54_shell_metacharacters_reach_opener_as_one_argument`, `p54_untrusted_needs_confirmation`. The sign-in metadata clause has no sign-in flow in M1 (MCP is M2) | T4 |
| P59 A cut prompt stops the turn | `Provider::prompt_integrity`; `StopReason::PromptCut` | `p59_mock_local_underreport_stops_turn`, `p59_ollama_fixture_cut_detected` | T9, T19 |
| P21 Secrets redacted from tool results | `Redactor`, `StreamRedactor`; engine result path; `web_fetch` URL check | `p21_tool_result_key_reaches_mock_provider_redacted`, `p21_fetch_url_with_key_refused_before_connect`, `p21_key_split_across_stream_chunks_redacted`, `secrets_canary_never_in_test_root` (key in tool output, error, panic, crash report; whole root scanned) | T6, T19, T22 |
| P22 A key bound to its host | `BoundEndpoint::key_name`; `ConfigEdit` side effect; `InteractiveProof` | `p22_host_change_clears_keychain_entry`, `p22_host_change_from_tool_or_headless_refused` | T7, T25 |
| P23 Scrubbed child environment; store out of reach | `process_env::scrubbed`, `BaseEnv`; sandbox profiles; `UseMyCredentials`; probe `SecretStoreReach`, `OtherProcessEnv` | `p23_agent_socket_variable_absent`, `p23_provider_key_absent_from_child_env`, `p23_use_my_credentials_restores_only_that_variable`, `p23_use_my_credentials_socket_reachable_only_when_granted`, `p23_secret_store_unreachable_<platform>` (asserts the specific denial; plants an item under the opt-in on CI), `p23_other_process_env_unreadable_<platform>` (a process started outside the sandbox with a canary variable) | T6, T10 to T12 |
| P57 No dumps; no headers in logs | `harden_process`; `diag`; transport's `SensitiveHeader` | `p57_linux_process_non_dumpable`, `p57_windows_error_report_dump_off` (Windows: reads back the opt-out), `p57_trace_level_provider_call_leaves_no_authorization`, `p57_secret_header_absent_from_debug_and_fixtures` | T6, T7 |
| P24 Fetch caps; https providers | `FetchCaps`; `check_provider_endpoint` | `p24_huge_body_stops_at_cap`, `p24_compression_bomb_stops_at_decompressed_cap`, `p24_non_loopback_http_provider_refused`, `p24_attended_private_allow_not_applied_unattended`, `net_address_spellings_refused` (table of spellings), `net_rebinding_connects_to_checked_answer`, `net_redirect_to_loopback_refused`, `net_no_connection_at_start` | T22, T4, T7 |
| P37 Release check on every artifact | `xtask release-check`; nightly step; markers in release-profile builds | `p37_marker_absent_in_release`, `p37_marker_found_in_release_profile_build_with_feature` (the positive control), `p37_test_switches_refused_exit_2` (CLI flag included), `p37_entitlements_match_reviewed_list`; the nightly runs it on every artifact | T33 (markers defined in T2a, T31, T34) |
| P40 Spend caps from M1 | `SpendLedger`; `worst_case`; `read_cap_bytes` | `p40_two_processes_budget_for_one_call_make_one`, `p40_unpriced_remote_model_refuses`, `p40_unrecognised_loopback_refuses`, `p40_scripted_loop_stops_at_cap_with_event`, `p40_call_estimated_over_cap_never_sent`, `p40_cancelled_stream_keeps_reservation`, `p40_single_file_read_capped`. Index rebuild and routine budgets arrive with M3 and M6 | T17, T19, T20 |
| P41 Definition of unattended | `Attendance`, `detect_cli_attendance` | `p41_run_without_terminal_turns_asks_into_denies`, `p41_unattended_never_runs_with_builtin_off` | T3, T24, T13 |
| P60 Least privilege for a new workspace | `PermissionEngine` defaults; `TrustState::Untrusted` starts in `Ask` | `p60_new_workspace_defaults` (view-model test asserting each default of section 3.7's table), `p60_untrusted_workspace_starts_in_ask_despite_auto_config` | T13, T27 |
| P39 Owner-only private files | `fsguard::private` | `p39_private_files_mode_0600_or_owner_acl`, `p39_create_through_planted_symlink_fails` | T5 |

## 7. Acceptance coverage

`docs/acceptance/CHECKLIST.md` lists two M1 rows, both marked "M1?" for this architect to confirm. Both are confirmed in
M1: the CLI verbs they name are needed in M1 to configure the providers and to report the sandbox's state.

| Row | Behaviour (checklist) | Task | How verified | Test |
|---|---|---|---|---|
| C1 | `unidex config` lists every section with each row's source, prints what a set would write without changing any file under a dry-run flag, and unset restores the default | T25 | automated | `c1_list_shows_every_section_with_source`, `c1_dry_run_prints_diff_and_writes_nothing` (file hash unchanged), `c1_unset_restores_default` |
| C3 | `unidex doctor` shows a config row that warns about an unknown key | T25 | automated | `c3_doctor_warns_unknown_key_with_line` |

## 8. Design calls made here

Each call is either left open by the design or the maintainer, or (D1, D3, D12, D19) touched approved text and was put
to the maintainer, who decided it on 2026-11-17; those rows say so. Each has a reason and the cost if wrong. "Record"
marks a call that the implementing task writes as a numbered decision record (numbers are reserved in the plan so
parallel tasks do not collide).

| # | Call | Reason | Cost if wrong | Record |
|---|---|---|---|---|
| D1 | Linux sandbox availability is decided by the probe, not by the package: wherever the full sandbox applies, a tarball install on a distribution that does not restrict user namespaces included, the shell is on; on default Ubuntu without the `.deb`'s profile it is off | **Decided by the maintainer on 2026-11-17** (accepted as proposed). Fail closed is about the sandbox not applying (threat model 5); nothing is turned off where the full sandbox does apply | None known; T40 updates design 5.11 and section 8 ("the tarball ... off") to say "where the full sandbox does not apply" | yes (0015, with T11's mechanism, citing the answer) |
| D2 | macOS daemonising answer for P50: every sandboxed command carries a per-session random marker variable; at session end, app quit or run end the supervisor kills the process group and then sweeps the user's processes once for that marker (reading each process's environment through `sysctl KERN_PROCARGS2`) | macOS has no cgroup or job object (threat model 7); a one-time sweep is event-driven, not a polling loop | A process that rewrites its environment escapes the sweep; it stays sandboxed and keeps only the network grant it started with (threat model 7's residual) | yes (0017) |
| D3 | Windows sandbox: an AppContainer per workspace with a job object, not the restricted token design 5.11 names; the access-list footprint of section 3.6 (which folders get which entries, when, their removal for every workspace, the `unidex doctor` row, the tests) is part of the contract | **Decided by the maintainer on 2026-11-17** (accepted). Only an AppContainer limits network (threat model 7), so only it can meet P1; a restricted token would leave the shell off under P7 | Many ordinary tools fail inside an AppContainer (T12 reports which); entries on the user's folders, recorded and removable (3.6); T40 updates design 5.11's "a restricted token" | yes (0016, citing the answer) |
| D4 | The persistent shaped-text cache sits around GPUI (our cache of `WrappedLine`s painted by our element), not as a GPUI change; a measurement task isolates the cause of slow first-visit frames first (pre-prepare every row's markdown, re-run; then pre-shape, re-run) | The pinned source shows the needed API is public (section 3.13); a fork would be carried across every weekly upgrade (decision 0007) | If shaping is not the cause, the cache task is re-scoped by the measurement's result before it starts; if the kit renderer is adopted (question 23), its bodies do not use this cache and must meet the scroll target in its cached form | yes (0020) |
| D5 | HTTP and TLS: `reqwest` 0.13 without default features, `rustls` with the `ring` provider, the OS verifier through `rustls-platform-verifier`, no proxy from the environment, no automatic redirects | Fewer C builds than `aws-lc-sys`; certificate checks against the OS store; redirects and proxies must pass our policy | If `rustls-no-provider` does not work as read from the metadata, the fallback is `aws-lc-rs` (its licence expression passes the allow list) at a larger build cost | yes (0024) |
| D6 | OS secret stores through crates already in the tree (`objc2-security`, `windows-sys`, `oo7`), not `keyring` | No new crate; full control of the service and account names (P22's host-bound account) | Three small backends to maintain instead of one crate | no (T6 notes it in its pull request) |
| D7 | Providers: Anthropic Messages (hosted) and Ollama native (local) | Section 5 | Section 5 | yes (0018) |
| D8 | Session log in the app's data folder (`<data_dir>/sessions/<workspace-key>/`), not the workspace | A workspace is untrusted until trusted (P13); a log inside it could be planted or read by repository code; the vault does not exist yet | Moving to the vault in M3 needs a migration of the files (design 5.5's migration rule, L18) | yes (0019, with the format) |
| D9 | Spend in integer micro-dollars; the "day" is the UTC day; ledger in SQLite with WAL and `BEGIN IMMEDIATE` | No floating point in money; no time-zone crate; the threat model asks for an atomic shared reservation | A user's local day differs from UTC by up to a day; the cap still bounds spend per 24 h period | yes (0023) |
| D10 | Three permission modes: `read_only`, `ask` (default), `auto` | Smallest set that covers P60's default, a read-only review and a permissive mode for P3's built-in asks to guard | More modes (for example "accept edits") can be added without a format change (`PermissionMode` is an enum in the log) | no |
| D11 | Before the vault (M3), the app's private state lives under `AppPaths` (config, trust, sessions, undo, spend, logs) | `unidex chat` works with no vault (design 5.2) and every path then goes through the choke point and the test-root guard. P3 says the undo copy is kept "under `.unidex/`"; before the vault it is `<data_dir>/undo/<session-id>/` instead, outside the workspace, where repository code cannot reach it (recorded in 0026 and listed for T40) | M3 moves config and sessions into the vault's `.unidex/` with a migration | yes (0026) |
| D12 | The Linux `.deb` is built with cargo-deb from M1, so the AppArmor profile loads at install from the post-install script | **Decided by the maintainer on 2026-11-17** (accepted). cargo-packager's `.deb` runs no maintainer script (decision 0014, measured), so the profile would load only after a reboot and the shell would stay off until then | A second packaging tool earlier than decision 0014 sequenced it; T40 corrects design 8, and record 0022 says it supersedes decision 0014's sequencing sentence on this point (0014 itself is not edited) | yes (0022, citing the answer) |
| D13 | No logging subscriber for third-party crates; our own redacted diagnostics only | P57's "no HTTP headers in logs at every level, third-party trace output included" holds by construction | Less detail when debugging the HTTP stack; a developer build can add a subscriber locally, never committed | no |
| D14 | No file-tool delete in M1; P3's undo copy covers overwrites and edits; the app's own code never removes a folder outright (the choke-point test refuses `remove_dir_all` outside tests and temp-folder cleanup) | Design 5.3 lists read, write and edit; a delete tool adds the OS trash dependency for little M1 value | A delete tool with the trash comes in M2 or later; P3's test "a folder delete lands in the trash" is deferred with it and listed for T40 | no |
| D15 | Display: hosts always shown in ASCII (punycode) with the Unicode form beside it when non-ASCII; deny-list matching lower-cases after NFC rather than full case folding | No script-property crate; a superset of "punycode when scripts mix" | A few characters (for example German sharp s) fold differently; listed as a residual in the threat model update | no |
| D16 | Instruction files in M1: `AGENTS.md` at the workspace root and its `@path` imports only; nested files in sub-folders are not loaded | P13 needs a defined, hashable set; M1 has no per-folder context | Nested instruction files arrive with M2's context management. Writes to other agents' instruction files (CLAUDE.md and similar) are still protected (P8; section 3.1.8), since tools the user runs outside the sandbox load them | no |
| D17 | Fonts on Linux are named with fallbacks and the `.deb` depends on `fonts-dejavu-core`; no font is bundled in M1 | Keeps the download small and avoids a font licence in the notices | Text looks different per distribution; a bundled font can be added later | no |
| D18 | Widget-kit isolation (question 23): one `MessageBodyRenderer` boundary with exactly one implementation compiled in; the kit, if adopted, only behind the `kit-renderer` feature | Gate question 23, option C, as the gate report recommends; the answer is needed only before T38 (message bodies); T30 draws bodies with a minimal renderer behind the same trait | If the answer arrives after T38 started, T38's renderer is redone (bounded by the trait) | yes (0021, written by T37 with the maintainer's answer) |
| D19 | Governed paths (the privacy gate's code and tests, `privacy-allow.toml`, `.gitleaks.toml`, the workflows, `deny.toml`, `.cargo/`): "flag and review". A base-branch check, `governed-paths (flag)` (`pull_request_target`, never checking out or running the pull request's code, a read-only token), flags every pull request that touches one with an annotation and a job summary listing the files, and does not fail it for that reason; each such pull request gets an extra security review by an Opus reviewer limited to the governed files, and the controller reports each such change to the maintainer | **Decided by the maintainer on 2026-11-17**, differently from both a code-owners file and the architect's admin-review check: the hosting service does not let the account that opens a pull request approve it, and every pull request is opened from the maintainer's account, so an enforced approving review cannot work today | Enforcement is a process rule, not a hosting-service rule. Enforced human approval (a ruleset requirement, by a code-owners file or otherwise) is owed for when a second account exists (outside contributors, or a bot account for the build agents) and is listed for the gate report; no account handle goes into the repository. The base-branch privacy scan (`privacy (base)`) is separate and does fail a pull request | yes (0025, written by T1d, citing the answer) |
| D20 | Core benchmarks in CI are release-mode runs of non-GPUI code against generous ceilings stored in `perf/ceilings.toml`, not statistical comparisons | Shared runners are noisy (design 9); ceilings catch large regressions without false alarms | Small regressions pass unnoticed until a gate measures on the reference machine | no |
| D21 | Windows `FixedShell` is `cmd.exe /d /s /c` (no AutoRun); PowerShell later | Smallest, predictable; every command asks anyway | Users who expect PowerShell syntax get errors in M1 | no |
| D22 | Default spend caps: 0.50 USD per call and 5.00 USD per day | P40 needs defaults; small enough that a runaway loop costs little before it stops | A long context on a large model can pass the per-call cap and refuse until the user raises it | no |
| D23 | Numbers set before any measurement: the ask card's guard interval 600 ms (P4); loop guards of 50 model calls per turn, 3 identical repeated calls and 3 invalid calls in a row; P14's caps 3 per origin per turn, 10 per turn, 40 per session until T28 measures | Each needs a starting value; each is one constant or config key | Too strict asks or stops too often, too loose lets more through; the gate revisits each with T28's numbers | no |
| D24 | A workspace's key is the sha-256 of its canonical path, device and inode (file id on Windows) | Trust and history follow the folder's identity, so a different folder at a reused path does not inherit trust (P13) | A fresh clone, a restore from backup or a move to another volume starts untrusted and with an empty session list there; old sessions stay on disk under the old key | yes (in 0026) |
| D25 | M1's "home screen" for the start-up target is the chat window with its session list and composer | M1 has no app shell (M4) | The figure times a lighter screen than M4's; it is measured again when the shell arrives | no |
| D26 | Our own HTML-to-text converter for `web_fetch` and our own command parser for P2, no crate | Both are small and conservative; `html5ever` and `shell-words` are larger than needed (section 2.3) | Parser bugs: mitigated by the rule that anything the parser is unsure of needs a shell (and asks), and by fuzzing | no |
| D27 | The todo list lives in the session (its log records), not in a workspace file | It touches no file, so nothing for the guard to police | A todo list does not carry into a new session | no |
| D28 | Networked commands (section 3.6): IP-only network filters on macOS; on Linux the host's network namespace shared, with Landlock ABI 6's abstract-socket and signal scopes required and network grants unavailable below it; `internetClient` only on Windows | Keeps P43 for networked commands with nothing installed; a user-mode network stack on Linux would need an install | Where the kernel's Landlock is older than ABI 6, commands get no network (`npm install` fails there until the kernel is newer); residual P43-a | yes (in 0015, 0016, 0017) |
| D29 | The permission mode belongs to a session; the configured mode applies to trusted workspaces, an untrusted workspace starts in `ask`, and widening needs an attended session (section 3.7) | P60's least privilege for a new workspace and P47's interactive-only widening | A user who configured `auto` gets asks in a new repository until they trust it | no |

## 9. Open questions for the maintainer

The maintainer answered D1, D3, D12 and D19 on 2026-11-17 (section 8). One question remains open:

1. **Gate question 23, the widget kit for message bodies** (open; needed before T38, the message-body task, starts).
   T37 makes decision 0008's four measurements of the kit's cached form: the scroll, jump, stream and resize scenarios
   on the 5,000-message fixture; selection and copy within and across messages in the virtualised list; link activation;
   and that a message whose parse lands a frame later does not shift the rows around it. What T38 needs from the
   answer: one word, "adopt" or "own renderer". **Recommendation:** adopt only if all four pass and the cached form
   meets the scroll target at least as well as the plain renderer with the shaped-text cache (T36's numbers); otherwise
   our own renderer (gate question 23, option C then A or B).

For the maintainer's information, not blocking any task:

- **A change to the gate's first-task list.** "Package a binary target, not an example" moves from the first task to
  the packaging task (T33), because no app binary exists before T29; T33 also adds a nightly step that fails if a
  staged program comes from an `examples` folder.
- **Owed for the gate report (D19).** Enforced human approval of governed changes, once a second account exists.
- **Approvals at the gate** (section 1.3): a hosted key with a spend cap for the evaluation run, and installing a local
  model server with one small model on the development Mac.
- **Runs on the development Mac.** Opening the app's window there, for scripted checks and measurements, is listed per
  task in the plan (T29, T30, T34, T36, T37, T39 and T41) so the maintainer can keep the reference machine otherwise idle
  during measurements; none changes OS state outside its temporary folder (section 3.6).
- **Repository settings** the maintainer changes when tasks land (adding the new required checks) are listed in the
  plan at the task that needs them.
