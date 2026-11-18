# M1 Vertical Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Build M1, the vertical slice: one hosted and one local provider driving the native engine loop with the core
tools, a shell sandbox on all three platforms, permissions and asks, a session log, spend caps, a real GPUI chat window
and `unidex chat`, the first evaluation run and unsigned nightlies, with the 31 accepted threat-model proposals built
and tested.

**Architecture:** Eleven workspace crates layered as `udx-core` at the bottom (shared plain types, guards, events and
the `Engine` and `Sandbox` traits), then providers, tools and sandbox, then the engine, then the view-model, the app,
the CLI and the evaluation harness. Every public type, signature, event and session-log record is a contract in
`docs/design/specs/2026-11-17-m1-architecture.md` ("A", cited by section, for example "A 3.7"); this plan says who
builds what, in which order, with which tests.

**Tech Stack:** Rust 1.100.0 (pinned), GPUI `gpui-pre` 0.3.12 (pinned), Tokio, `reqwest` 0.13 with `rustls` and the
`ring` provider, `serde`/`serde_json`, `toml_edit`, `rusqlite` (bundled SQLite), `pulldown-cmark`, `ignore`,
`landlock` and `seccompiler` (Linux), `objc2-security` (macOS), `windows-sys` (Windows), `oo7` (Linux secret store),
`proptest`, `assert_cmd`, `rcgen`; `cargo xtask` for the privacy gate, hooks, release check and benchmarks;
cargo-packager and cargo-deb for packages; GitHub Actions on macOS, Linux and Windows runners.

**Spec:** `docs/design/specs/2026-11-16-unidex-rust-design.md` (the design), with
`docs/design/specs/2026-11-17-m1-architecture.md` (the contracts) and `docs/security/THREAT-MODEL.md`

Status: proposed by the architect for review, revised after review 1 · 2026-11-17. Other sources: the M0 gate report
(`docs/build/M0-GATE.md`), decision records 0001 to 0014, and the maintainer's answers of 2026-11-17 to gate questions 1
to 5 and to design calls D1, D3, D12 and D19 (A section 8). "Tn" in this file means "Task n".

## Global Constraints

Every builder's brief carries this section and the Review Focus with the task's own text (design 11.2).

- **Privacy (design 12).** Public repository. No personal information in any file, commit, pull request, ledger line or
  agent report: no person's name, email address, account handle, home-folder path, host name or private network address,
  and no description of anyone's private setup. Write "the maintainer" and "the build team". Fixtures use `example.com`,
  `.test`, `.invalid` and `.example` names, the documentation address ranges, and invented names.
  `CI=1 cargo xtask privacy --all` must be clean before every commit; never bypass the commit hooks.
- **Test vectors and the scanners.** Neither the privacy gate nor the secret scanner is ever loosened for a test. The
  convention, which T1d writes into `docs/build/test-vectors.md` and every later task follows:
  - No key-shaped value is written whole in any committed file, tests and fixtures included. A test builds it at run
    time from parts, each too short to match any secret rule (a prefix constant split in two, a body generated at run
    time). Fixtures and golden files use low-entropy invented values (`sig-0000`, `msg_0001`) that match no rule.
  - Addresses: the documentation ranges (192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24, 2001:db8::/32) and loopback,
    which the privacy rules never flag. A private, shared, link-local or metadata-service address is written only as a
    numeric constructor (`Ipv4Addr::new(..)`) or an octet array, its dotted spelling built at run time.
  - Home folders: only the placeholder names the privacy rules allow (`/home/user`, `/Users/example`,
    `C:\Users\user`); any other user name is built at run time.
  - Hosts: `example.com`, `example.org`, `example.net`, `.test`, `.example`, `.invalid`. A `.local` or `.lan` name, a
    hardware address or a `user@host` prompt is built at run time. Emails use the example domains.
  - The privacy gate's own tests (`xtask/tests/`) keep their path-scoped allow entries, because they test the rules.
  - The allow files (`privacy-allow.toml`, `.gitleaks.toml`) are frozen after T2a. A task whose vector cannot follow
    the convention stops and reports.
- **Governed paths: flag and review (D19, decided by the maintainer on 2026-11-17).** A pull request that touches the
  privacy gate's code or tests, `privacy-allow.toml`, `.gitleaks.toml`, a workflow, `deny.toml` or `.cargo/` is flagged
  by the base-branch check `governed-paths (flag)` (T1c). Every such pull request gets an extra security review by an
  Opus reviewer limited to the governed files, and the controller reports each such change to the maintainer. Enforced
  human approval is owed until a second account exists; no account handle goes into the repository.
- **Provenance (design 11.4).** Read permissively licensed code as reference only; copy nothing from code under any
  other licence; do not read or describe the editor GPUI comes from beyond what decision 0007 says. A fragment adapted
  from permissively licensed code carries its attribution in a comment and in the third-party notices.
- **Contracts first (design 11.2).** The public items of A are created by T2a and T2b and changed only with the
  controller's agreement, written in the pull request. A builder who finds a contract unbuildable stops and reports;
  the controller amends A in its own small pull request.
- **Nothing is installed** on the development machine (a toolchain target, a tool, a model server) without the
  maintainer's approval, and nothing in M1 launches a browser or CEF, in code, tests or CI.
- **Unverified external APIs.** Wherever A or a task says "not verified", the task's first step verifies it against the
  pinned source or the vendor's published reference and records what it read in the pull request. If it is wrong, the
  task stops and reports before building on it.
- **Tests.** Every test writes only inside its test root (`UNIDEX_TEST_ROOT`, through `TestRoot` or `TestEnv`) and
  contacts no real service: providers run on recorded fixtures or the scripted mock provider, `web_fetch` on the
  loopback HTTP stand-in with a stub resolver. Live tests are opt-in (`UNIDEX_LIVE_TESTS=1`) and never run in CI.
- **No side effects on the development Mac (A 3.1.1, A 3.6).** No test, probe or check run locally may change OS state
  outside its temporary folder: the secret store, trust stores, access lists, AppContainer profiles, launch items,
  disk images, installed packages. Such tests run only on CI runners behind `UNIDEX_TEST_OS_SIDE_EFFECTS` naming their
  kind (T6 sets it on the CI test jobs; T11 and T33 in their own workflows) and are skipped elsewhere with a printed
  notice. Each task's platform notes say which of its tests are CI only.
- **Runs on the development Mac.** Opening the app's window there for scripted checks or measurements (T29, T30, T34,
  T36, T37, T39, T41) needs the maintainer's approval, asked by the controller before the task starts, so the reference
  machine can be kept otherwise idle during measurements.
- **Shared files.** These are edited only by the tasks named, so parallel tasks never collide:

  | File or folder | Edited by |
  |---|---|
  | `Cargo.toml` (root), every member's `Cargo.toml` dependency and feature tables, `Cargo.lock` | T2a only (T2b compiles against them unchanged); a later need for a dependency stops the task and becomes a sequenced change. Exception: `[package.metadata.deb]` in `crates/udx-app/Cargo.toml`, by T33 |
  | `deny.toml` | nobody in M1 (any need stops the task) |
  | `privacy-allow.toml`, `.gitleaks.toml` | T1a, T1d, then T2a only if a golden file cannot follow the test-vector convention; nobody after T2a |
  | `.github/workflows/ci.yml` | T1d, T6, T35, never two at once |
  | New workflows | `governance.yml` (T1c), `privacy-base.yml` (T1d), `sandbox-linux.yml` (T11), `ui.yml` (T32); `nightly.yml`: T1d (its concurrency group), then T33 |
  | `xtask/src/main.rs` | T1c, T1d, T33, T35, never two at once |
  | `crates/*/src/lib.rs` (module lists) | T2a (`udx-core`, and a placeholder for each new crate), then T2b (each new crate); nobody after. A module list inside a task's own folder belongs to that folder's task (`udx-engine/src/turn/mod.rs`: T19, then T23) |
  | `crates/udx-test-support/src/{lib.rs,stand_in.rs}` | T1b; `stand_in.rs` is filled by T7 |
  | `crates/udx-app/src/main.rs`, `crates/udx-cli/src/main.rs` | T2b creates them; then T29 and T24 respectively, only |
  | `docs/build/LEDGER.md`, `docs/decisions/README.md` | the controller only, after each merge (M0 lost time to index conflicts) |

- **Decision record numbers are reserved** so parallel tasks do not collide: 0015 Linux sandbox (T11), 0016 Windows
  sandbox (T12), 0017 macOS sandbox (T10), 0018 the two providers (T9), 0019 session log format (T16), 0020 shaped-text
  cache (T39, drafted by T36), 0021 widget kit for message bodies (T37), 0022 the `.deb` built with cargo-deb (T33),
  0023 spend ledger (T17), 0024 HTTP and TLS stack (T7), 0025 governed paths: flag and review (T1c), 0026 state root
  before the vault (T3).
- **One task, one branch, one pull request** in its own worktree, small enough to review in one read (design 11.2). At
  most two compiling agents run at once locally (design 11.3); document work runs beside them. `cargo check` before any
  full build; sccache on when available (decision 0007).
- **Roles (design 11.1).** Core builder: Opus, for the novel parts. Builder: Sonnet, for well-specified units. Test
  author: Sonnet, never the builder, writes failing tests from the contract first. Reviewer: Opus, reads the diff
  against this plan and A without the builder's summary, and checks the builder's report against the diff (M0's
  lesson). Two failed Sonnet attempts escalate to Opus; two more stop the task and go to the maintainer.
- **Budgets** (gate report, section f, "Proposed budgets"), per task, all roles together, a hard stop that reports to
  the controller. Classes: code unit 1.0M (test author 150k, builder 300k, reviewers 500k, reserve 50k); core code 1.3M
  (re-split so a test author fits, as design 11.1 requires: test author 150k, builder 550k, reviewer 350k, fix agent
  200k, reserve 50k); CI or packaging 1.3M (builders and test author 450k, reviewers 700k, reserve 150k); trial 1.3M
  (builder 600k, reviewer 250k, fix agent 300k, re-reviewer 100k, reserve 50k); document 0.6M; research document 1.2M;
  the gate's security review 0.5M. Two task caps below a class's are the architect's proposal for small units, not
  section f's: small code unit 0.7M (test author 100k, builder 250k, reviewers 300k, reserve 50k) and small CI unit
  0.9M (builder 350k, reviewers 450k, reserve 100k). Section f offers 700k and 900k as lower caps for the whole code and
  CI classes, as an alternative the maintainer may choose; a unit as large as M0's Task 2 (825k) would stop at 0.7M.
  Fix rounds for agents above about 400k go to fresh agents with a findings file; large Opus agents run one at a time
  (gate report, "Pacing").
- **Done means** these pass locally on macOS and in CI on three platforms, plus the task's own commands (tests that
  need `UNIDEX_TEST_OS_SIDE_EFFECTS` are skipped locally with a notice):

  ```sh
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --locked -- -D warnings
  cargo test --workspace --locked
  cargo deny --locked check
  CI=1 cargo xtask privacy --all
  ```

  CI also runs `cargo audit` and, for the crates in its scope, mutation testing; both must pass.
- **Platform evidence.** macOS is verified locally and in CI. Linux and Windows are verified in CI only (the maintainer
  has no Linux or Windows machine; gate question 24 is not answered). A pull request states which platform verified
  which test, and never claims a hands-on result nobody produced.
- **Screens.** A pull request that changes a screen attaches the `ui (macos)` job's frame and baseline difference
  (design 10.1).
- **Repository settings.** New required checks are added to the ruleset by the maintainer; the task that creates a check
  says so in its pull request. Merges follow the repository's existing merge procedure.

## Review Focus

The five input classes or failure modes most likely to bite a person using M1 that no single task's tests would
otherwise exercise, most likely first. Each has a named test in a task whose Needs cover everything that test uses:
T23 (the real tools, sandbox and supervisor wired into the engine) for the engine side, T24 for the CLI's output, and
T9 for the local provider's half of item 4. Reviewers of every task keep them in mind.

1. **Unusual workspace paths** (spaces, quotes, parentheses, non-ASCII names in NFC and NFD, Windows paths past 260
   characters) through the guard, the sandbox profile's parameters and the session-log key and folder:
   `review_focus_hostile_workspace_paths` (T23); and through the CLI's escaped output:
   `review_focus_hostile_workspace_paths_cli` (T24, which needs T23 and owns `udx_cli`).
2. **Cancel or crash in the middle of a turn, then resume**: a write half done, a command still running, a spend
   reservation open, a truncated last log line: `review_focus_cancel_then_resume` (T23).
3. **Large real repositories** (100,000 files with `node_modules` and git-ignored secret files): the per-command
   deny-list scan and masks, glob and grep, and start-up of a sandboxed command stay bounded and correct:
   `review_focus_large_workspace` (T23; T12 also measures Windows access-entry propagation on such a tree).
4. **A small local model's quirks**: a small context window, tool calls without ids, output that imitates the prompt
   protocol's delimiters, a prompt the server silently cut. Split at the `StreamEvent` contract (A 3.2), so each half
   sits in a task that can see what it uses: `review_focus_local_model_quirks_provider` (T9, Ollama fixtures through
   `OllamaProvider`: what the provider emits for each quirk) and `review_focus_local_model_quirks` (T23, the scripted
   mock provider emitting exactly that through the real engine and tools).
5. **Two front ends at once on one workspace** (the app and `unidex chat`, or two CLI sessions): the shared spend
   ledger, the trust file and the session list: `review_focus_two_front_ends` (T23, two processes).

## Dependency graph and waves

Arrows read "needs". Tasks on one line may run beside each other (disjoint files); the two-compiler limit still applies,
so the controller runs at most two of them at once.

```
Wave 0  T1a, T1b, T1c (no needs)   T1d (T1a, T1c)   T2a (T1a to T1d)   T2b (T2a)
Wave 1  T3 (T2b) -> T6 (T3) -> T7 (T4, T6)   T4 (T2b)   T5 (T1b, T2b)
Wave 2  T8, T9 (T7)   T10 (T5, T6) -> T11, T12 (T10)   T13 (T3, T5)   T14 (T3, T5)   T15 (T3)
        T16 (T3, T5, T6)   T17 (T3)
Wave 3  T18 (T14)   T19 (T7, T13 to T18)   T20 (T5)   T21 (T10, T13)   T22 (T4, T6)   T26 (T2b) -> T27 (T13, T26)
        T23 (T10, T19 to T22) -> T24 (T23), T28 (T23)   T25 (T3, T6, T7, T10, T12)
Wave 4  T29 (T23, T27) -> T30 -> T31 -> T32   T33 (T11, T29)   T34 (T30) -> T35 (T33, T34)
Wave 5  T36, T37 (T34, T35) -> maintainer answers question 23 -> T38 (T26, T30) -> T39 (T36, T38)
Wave 6  T40 (T3 to T28, T32, T33, T38)   T41 (everything)
```

| Wave | Tasks | Compiling pairs the controller can run together |
|---|---|---|
| 0 | T1a, T1b, T1c, then T1d, then T2a, then T2b | any two of T1a, T1b, T1c |
| 1 | T3, T4, T5, then T6, then T7 | T3 with T5; T4 with T5; T6 with T5 |
| 2 | T8 to T17 | any two; T11 and T12 after T10 (they reuse its supervisor and probe framework) |
| 3 | T18 to T28 | any two whose dependencies merged |
| 4 | T29 to T35 | T32 with T33; T33 with T35 not together (both edit `xtask/src/main.rs`) |
| 5 | T36 to T39 | T36 with T37 |
| 6 | T40 (document), T41 (gate) | T40 runs beside anything |

## Wave 0: the first task of M1, then the contracts

The gate report requires every item of its section g, "The first task of M1: harden the privacy gate and the path
guard", in M1's first task. That is too large for one pull request, so the first task is four pull requests, T1a to
T1d, which together hold every item (one routed, see T1d); nothing else starts before all four merge.

### Task 1a: Privacy gate: what it cannot see, and the false positive

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** Nothing. **Runs beside.** T1b, T1c.
- **Files.** Modify `xtask/src/privacy/scan.rs`, `xtask/src/privacy/rules.rs`, `xtask/src/privacy/git.rs`,
  `xtask/src/privacy/mod.rs`, `xtask/tests/privacy.rs`, `xtask/tests/privacy_mutants.rs`,
  `xtask/tests/github_committer.rs`, `privacy-allow.toml` (path-scoped entries for this task's own test vectors only),
  `.cargo/mutants.toml` (only for a justified equivalent mutant).
- **Consumes.** Nothing new. **Produces.** The same `cargo xtask privacy` interface, with wider coverage.
- **Items covered (gate report, section g).** UTF-16 content in every scan mode (`scan.rs:162-164`); host names beyond
  the one pattern: local network names, `user@host` prompts, IPv6 unique-local and link-local addresses, hardware
  addresses (`rules.rs:74-75`); a home-folder path inside a URL's query string (`rules.rs:121-133`); a byte-order mark
  or UTF-16 encoding in the private term list, which must fail loudly rather than silently disable terms
  (`scan.rs:43-55`, `mod.rs:155`); non-UTF-8 file names in the whole-tree mode (`git.rs:45-50`); submodule path names
  (`git.rs:94-95`); an empty range reported with the number of commits scanned, never a bare "clean"
  (`mod.rs:403-404`); allow-list values compared on the whole value, not a truncated excerpt (`mod.rs:140-142`); the
  home-folder rule anchored to the start of a path so a `home` or `users` module directory is not flagged
  (`rules.rs:53-54`); test vectors moved to reserved domains, keeping a real domain only where the rule under test is
  about that domain (the hosting service's no-reply address, the private-network host suffix), each with a comment
  saying why. Also: the two xtask tests that write a git config under the OS temp folder
  (`xtask/tests/privacy.rs:122`, `xtask/tests/privacy_mutants.rs:14`, through `std::env::temp_dir()`) move onto a
  folder under `UNIDEX_TEST_ROOT`, which cargo sets for every test through `.cargo/config.toml`.
- **Tests** (failing first; every vector below is written in the test files, which the path-scoped allow entries cover;
  none is written literally in this plan or in A, so the new rules do not flag either document):
  - `utf16le_and_utf16be_files_are_scanned`: a personal term in a UTF-16 file (each byte order, with and without a BOM)
    is found in `--all`, `--staged`, `--range` and `--stdin`.
  - `local_network_names_flagged`: a `.local` name, a `.lan` name, a shell prompt line naming a user and a host, an
    IPv6 link-local and a unique-local address and a hardware address are each flagged; `example.com`, `::1`,
    `127.0.0.1` and a version string are not.
  - `home_path_in_query_string_flagged`: an `example.com` URL whose query string holds an invented user's home-folder
    path, plain and percent-encoded, is flagged.
  - `term_list_with_bom_or_utf16_fails_loudly`: a term list saved with a BOM is read correctly; one saved as UTF-16
    makes the scan exit 2 with a message, even without `--require-local`.
  - `non_utf8_file_name_scanned`: a file whose name is not UTF-8 is scanned and its name checked (Unix).
  - `submodule_path_name_checked`: a submodule at a path holding a personal term is flagged.
  - `empty_range_reports_zero_commits`: an empty range prints that zero commits were scanned and exits 0.
  - `allow_entry_must_match_whole_value`: an allow entry matching only the first 60 characters of a longer value does
    not exempt it.
  - `relative_home_segment_not_flagged`: a relative path with a `home` or `users` module folder is clean; an absolute
    home-folder path of an invented user, in the Unix and the Windows form, is flagged.
  - `xtask_tests_write_only_under_test_root`: the git-config helpers of both test files place their file under
    `UNIDEX_TEST_ROOT`.
- **Done when.** The common commands; `CI=1 cargo xtask privacy --all` still clean on the whole repository with the new
  rules (this plan and A included); `cargo mutants -p xtask --no-shuffle -j 2` locally with no missed mutant.
- **Platform notes.** UTF-16 tests run on all three CI platforms; the non-UTF-8 name test is Unix only (Windows file
  names are UTF-16), skipped with a printed notice on Windows. Nothing here changes OS state.

### Task 1b: Path guard: hardening, the single choke point, and the test-support cycle

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** Nothing. **Runs beside.** T1a, T1c.
- **Files.** Modify `crates/udx-core/src/paths.rs`, `crates/udx-core/Cargo.toml` (add the `testing` feature; the
  dev-dependency on `udx-test-support` stays, since once that crate no longer depends on `udx-core` it closes no cycle,
  A 2.2), `crates/udx-core/tests/paths.rs`, `crates/udx-test-support/Cargo.toml` (drop `udx-core`),
  `crates/udx-test-support/src/lib.rs`. Create `crates/udx-core/tests/choke_point.rs`, its fixtures
  `crates/udx-core/tests/fixtures/choke_point/{allowed,refused}/*.txt`, and `crates/udx-test-support/src/stand_in.rs`
  (the stand-in's API as stubs naming T7).
- **Consumes.** Nothing new. **Produces.** A 3.1.1 (`Env::var_os`, the `PathError` additions, `home_dir`, `UserVar`,
  `user_var`, `test_root` (not cached), `check_test_path`, `os_side_effects_allowed` and `OsSideEffect` with the single
  opt-in variable `UNIDEX_TEST_OS_SIDE_EFFECTS`, `paths::testing::TestEnv`) and A 3.14 (`TestRoot`, `TempHome` and
  `scan_for_canary` implemented; `HttpStandIn`, `StandInRoute`, `StandInResponse`, `RecordedRequest`, `TestCert` as
  stubs for T7).
- **Items covered (gate report, section g, "The path guard" and "Architecture notes").** Tests for a relative override
  and a missing root; the right error for absolute-path failures (`paths.rs:73`, `:77`, `:96`, now `NotAbsolute`);
  values that are only whitespace (`paths.rs:63`, now `Blank`); a printed notice when a test is skipped because it runs
  as root (`tests/paths.rs:316-323`); `udx-core`'s path resolution as the single choke point for every user-folder path
  (the choke-point test, with `home_dir` and `user_var` as the accessors other modules use); the test environment
  helper required in tests (the `testing` feature refuses OS folders); the per-test guard on every resolved path
  (`check_test_path`); the scheduler's refusal to register an OS wake-up from a process with a test root
  (`os_side_effects_allowed(WakeUp)`, which M6's scheduler must call); per-crate test helpers instead of a test-support
  crate that depends on the crates it tests (A 2.2).
- **Tests:**
  - `relative_override_is_made_absolute_inside_root` and `relative_override_escaping_root_refused`.
  - `missing_root_is_created_by_test_env_and_refused_by_resolve_when_unresolvable`.
  - `non_absolute_value_reports_not_absolute` (each of the three variables).
  - `whitespace_only_value_reports_blank` (each variable, and the test root).
  - `skipped_as_root_prints_notice` (asserts the notice text through a captured writer).
  - `testing_feature_never_returns_os_folders`: with the feature on and neither variable set, `resolve` returns
    `TestBuildWithoutTestRoot`.
  - `check_test_path_refuses_outside_and_accepts_inside`, including a symlink that escapes.
  - `test_root_not_cached_across_envs`: two `TestEnv`s with different roots each see their own.
  - `home_dir_inside_test_root` and `user_var_reads_only_listed_names`.
  - `os_side_effects_refused_with_test_root` for each `OsSideEffect`; `os_side_effects_opt_in_by_kind` (the variable
    names kinds in snake case, comma-separated, or `all`; an unnamed kind stays refused).
  - `choke_point_rule_accepts_sanctioned_accessor` and `choke_point_rule_refuses_direct_lookups`: the rule of A 3.1.1
    ("The choke-point rule") run over the fixture files. A legitimate caller passes: `udx_core::paths::home_dir(env)`,
    `crate::paths::home_dir(env)`, `home_dir(env)` after `use udx_core::paths::home_dir`, the same for `user_var`, a
    method `var` reading `UNIDEX_TEST_ROOT`, and `"HOME"` as a map key. Each direct lookup fails: `std::env::home_dir()`,
    `env::home_dir()` after `use std::env`, a use tree importing `home_dir` or `temp_dir` from `std::env` (one split
    over lines, one renamed), `use std::env as e`, `std::env::temp_dir()`, a path into `dirs`, `directories` or `home`,
    each OS home lookup the rule names (`getpwuid`, `SHGetKnownFolderPath`, ...), `std::env::var("HOME")`,
    `env.var("USERPROFILE")`, `std::env::var_os("XDG_CONFIG_HOME")`, and `env!("HOME")`.
  - `choke_point_no_direct_user_folder_access`: the same rule over every `.rs` file under `crates/` except
    `crates/udx-core/src/paths.rs` and `crates/udx-cookbook/` (the fixtures are `.txt`, so the scan never reads them).
    The bare names `home_dir`, `user_var` and `temp_dir` never fail it; only the routes to the OS do, so T1b's own
    `home_dir_inside_test_root`, `GuardConfig`'s home (T5) and the CLI's and app's P48 checks pass. `xtask/` lies
    outside `crates/`; the test says why it is not scanned (the build tool, never shipped; its tests are moved onto the
    test root by T1a). The test's named allow list of (file, pattern) pairs is empty at merge and grows only with the
    controller's agreement.
  - `choke_point_no_remove_dir_all`: fails on `remove_dir_all` outside `#[cfg(test)]` code, test folders and a named
    allow list of temp-folder cleanups (P3; A D14).
  - `choke_point_no_direct_chrono`: fails if any workspace member's `Cargo.toml` names `chrono` (gate report, section
    g; decision 0007).
  - `temp_home_plants_files`, `scan_for_canary_finds_each_encoding` (plain, base64 both alphabets, percent, hex).
- **Done when.** The common commands; `cargo tree -p udx-test-support -e normal,dev` shows no workspace crate (so
  `udx-core`'s dev-dependency on it closes no cycle, A 2.2); `cargo mutants -p udx-core` locally with no missed mutant.
- **Platform notes.** All tests on three CI platforms; the root notice is Unix only. Nothing here changes OS state.

### Task 1c: Governed paths: flag and review

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Small CI unit, 0.9M.
- **Needs.** Nothing. **Runs beside.** T1a, T1b.
- **Files.** Create `.github/workflows/governance.yml`, `xtask/src/governed.rs`, `xtask/tests/governed.rs`,
  `docs/decisions/0025-governed-paths-review.md`. Modify `xtask/src/main.rs` (the `governed` subcommand only).
- **Consumes.** Nothing. **Produces.** The check `governed-paths (flag)` and the subcommand
  `cargo xtask governed --files <list-file>`.
- **What it builds (D19, decided by the maintainer on 2026-11-17: "flag and review").** The gate report's first-task
  item "required review, through a code-owners file" cannot be enforced today: the hosting service does not let the
  account that opens a pull request approve it, and every pull request is opened from the maintainer's account. So:
  - `governance.yml` runs on `pull_request_target` (types `opened`, `synchronize`, `reopened`, `ready_for_review`), so
    the base branch's definition of the workflow runs, never the pull request's. `permissions: contents: read,
    pull-requests: read`; the checkout is the base branch with `persist-credentials: false`; it never checks out,
    builds or runs the pull request's code. One job, `governed-paths (flag)`: build the base branch's `xtask`; list the
    pull request's changed files through the hosting service's API with the read-only token, into a file, as data;
    run `cargo xtask governed --files <file>`.
  - `governed` reads the list, matches each path against the governed list (below), prints one warning annotation per
    governed file and writes a job summary table of them, and exits 0 whether or not any were found. It exits nonzero
    only on its own error (an unreadable list, an API failure upstream). File names are data: every name is escaped
    for workflow commands (`%`, carriage return, newline, `:` and `,` in properties) before it is printed.
  - The governed list, as one table in `governed.rs` with a reason per row: `xtask/src/privacy/**`,
    `xtask/src/hooks.rs`, `xtask/src/governed.rs`, `xtask/tests/privacy*.rs`, `xtask/tests/secrets.rs`,
    `xtask/tests/hooks.rs`, `xtask/tests/github_committer.rs`, `xtask/tests/governed.rs`, `privacy-allow.toml`,
    `.gitleaks.toml`, `.github/workflows/**`, `deny.toml`, `.cargo/**`.
  - The process rule (Global Constraints): every flagged pull request gets an extra Opus security review limited to
    the governed files, and the controller reports each such change to the maintainer.
  - Record 0025 states the decision with the maintainer's answer, and lists as owed, for the gate report, enforced
    human approval (a ruleset requirement, by a code-owners file or otherwise) once a second account exists (outside
    contributors or a bot account for the build agents). No account handle goes into the repository.
- **Tests:** `governed_list_matches_record` (each listed path is flagged; near misses such as `xtask/src/release_check.rs`
  and `docs/build/LEDGER.md` are not); `governed_names_escaped_in_annotations` (a file name holding a newline, `%` and
  `::` cannot inject a workflow command); `governed_exits_zero_when_flagged`; `governed_fails_on_unreadable_list`.
- **Done when.** The common commands. After merge (a `pull_request_target` workflow runs only from the base branch, so
  it cannot judge its own pull request), the controller observes and records in the ledger: a scratch pull request
  touching `deny.toml` shows the annotation and the summary and the check passes; a scratch pull request that edits
  `governance.yml` to print nothing still gets flagged, because the base branch's definition runs. Scratch pull
  requests are closed and their branches deleted (as M0's Task 4 did). Not verified, recorded from the scratch run:
  whether a `pull_request_target` run attaches to the pull request's head commit as a check the ruleset could require
  (informative only, since the check never fails for a governed change).
- **Platform notes.** The workflow job runs on Linux; the `governed` tests run on all three CI platforms. Nothing here
  changes OS state.
- **Repository settings.** None required. The maintainer may add `governed-paths (flag)` to the required checks; it
  fails only on its own error.
- **Record.** 0025.

### Task 1d: Privacy gate: local secret scan, pre-push hook, installer, and base-branch CI coverage

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. CI or packaging, 1.3M.
- **Needs.** T1a (shares `xtask/src/privacy/`), T1c (shares `xtask/src/main.rs`). **Runs beside.** T1b if it has not
  merged.
- **Files.** Create `xtask/src/privacy/secrets.rs`, `xtask/tests/secrets.rs`, `xtask/tests/hooks.rs`,
  `.github/workflows/privacy-base.yml`, `docs/build/test-vectors.md`. Modify `xtask/src/main.rs`, `xtask/src/hooks.rs`,
  `xtask/src/privacy/mod.rs`, `.github/workflows/ci.yml` (the macOS cookbook step only),
  `.github/workflows/nightly.yml` (concurrency group only), `privacy-allow.toml` and `.gitleaks.toml` (only if a
  pattern needs it; key-shaped vectors are built at run time, so none should).
- **Consumes.** T1a's scanner. **Produces.** Secret patterns in `xtask`, the pre-push hook, the check `privacy (base)`,
  and the test-vector convention document.
- **Items covered (gate report, section g).** A local secret scan before M1 brings provider keys into the workflow:
  provider-key patterns in `xtask` (Anthropic and OpenAI key prefixes, cloud access keys, the hosting service's tokens,
  private key blocks, generic `Authorization: Bearer` values), run by the pre-commit hook and by `--all`
  (`ci.yml:337-344` today runs the secret scanner in CI only); a pre-push hook scanning the range being pushed, which
  covers rebases, cherry-picks, merges, bypassed hooks and commits made before the hooks were installed
  (`hooks.rs:44-50`); CI scanning pull request titles and bodies (`ci.yml:270-344`); CI judging a pull request with the
  default branch's gate and allow file, so a pull request cannot weaken its own check (`ci.yml:306-336`); the hook
  installer writing into a global hooks path and then refusing commits in other repositories (`hooks.rs:24-38`): it now
  writes only into this repository's hooks folder and refuses with a message when `core.hooksPath` points outside it;
  an existing hook that cannot be read, or is not UTF-8, is no longer overwritten (`hooks.rs:51-60`): the installer
  stops and says so. "Required review, through a code-owners file" is T1c (D19).
- **The base-branch scan (`privacy-base.yml`).** It runs on `pull_request_target` (types `opened`, `synchronize`,
  `reopened`, `edited`, `ready_for_review`), so the base branch's workflow definition, gate code and allow files judge
  the pull request. `permissions: contents: read, pull-requests: read`; the checkout is the base branch with
  `persist-credentials: false` and full history; the pull request's commits are fetched as objects only
  (`refs/pull/<n>/head` into a local ref, never checked out); nothing from the pull request is built or run, and its
  `.cargo/` and hooks are never read because the working tree is the base branch's. Steps: build the base branch's
  `xtask`; run `cargo xtask privacy --range <base>..<pull request head>` (it reads the pull request's blobs through
  git as data, with the base branch's allow file); run the pinned, hash-checked secret scanner over the same range with
  the base branch's `.gitleaks.toml`; scan the title and body, passed through environment variables and read by
  `cargo xtask privacy --stdin`, never interpolated into a script. The existing `privacy` job in `ci.yml` keeps running
  on the pull request's own code; it is the base job that a pull request cannot weaken.
- **The first task's architecture notes** (gate report, section g, "Architecture notes for M1's plan"), each done here
  or routed: the nightly's concurrency group, so a scheduled run and a manual run do not cancel each other
  (`nightly.yml:41-43`): done here, the group gains `github.event_name`; the macOS cookbook step that recompiles the
  platform crates (`ci.yml:137-143`): done here, by building the examples in the same feature resolution as the tests
  and measuring the saving; per-crate test helpers: done in T1b; no direct `chrono` dependency: enforced in T1b's
  choke-point test; test vectors on reserved domains: done in T1a; package a binary target, not an example
  (`Packager.toml:28-30`): routed to T33, because no app binary exists before T29 (A section 9 tells the maintainer).
- **The test-vector convention.** `docs/build/test-vectors.md` states the convention exactly as the Global Constraints
  give it, with one worked example per kind; this task's own tests follow it (every key-shaped vector is assembled at
  run time).
- **Tests:**
  - `secret_patterns_flag_each_vector` (each vector assembled at run time) and `secret_patterns_ignore_look_alikes`
    (placeholders such as a prefix followed by `EXAMPLE`, hashes, UUIDs).
  - `pre_push_scans_pushed_range` (a commit made with `--no-verify` is caught at push),
    `pre_push_new_branch_scans_from_merge_base`.
  - `installer_refuses_global_hooks_path`, `installer_keeps_unreadable_hook`, `installer_keeps_non_utf8_hook`.
  - `range_scan_reads_blobs_without_checkout`: a range whose head commit is fetched but not checked out is scanned with
    the working tree's allow file, not the one the range adds.
- **Done when.** The common commands. After merge, the controller observes and records in the ledger, with links: a
  scratch pull request whose title holds an invented home path fails `privacy (base)`; a scratch commit that adds an
  allow entry for its own finding still fails `privacy (base)`; a scratch pull request that edits `privacy-base.yml` to
  exit 0 still fails `privacy (base)`, because the base branch's definition runs. Scratch pull requests are closed and
  their branches deleted. Not verified, checked on the scratch runs: that the read-only token can fetch
  `refs/pull/<n>/head`; that a `pull_request_target` run's result attaches to the pull request's head commit so the
  ruleset can require it (if it cannot, the controller says so and the maintainer decides how to require it).
- **Platform notes.** Hooks are tested on all three CI platforms (Windows runs them through Git's shell). The workflow
  jobs run on Linux. Nothing here changes OS state on the development Mac (hooks are installed into a test
  repository under the test root).
- **Repository settings.** The maintainer adds `privacy (base)` to the ruleset's required checks.

### Task 2a: Contracts: dependencies, every manifest, and udx-core's shared types, events and golden files

- **Role and budget.** Opus core builder (every later task builds on its output), Opus reviewer. No separate test
  author: its tests are the golden files, the crate-map check and the compile. Core code, 1.3M.
- **Needs.** T1a to T1d. **Runs beside.** Nothing (it touches every manifest).
- **Files.** Modify `Cargo.toml` (members stay `crates/*`; `[workspace.dependencies]` gains every external crate and
  feature of A 2.3), `Cargo.lock`, `crates/udx-core/Cargo.toml`, `crates/udx-core/src/lib.rs`,
  `crates/udx-cli/Cargo.toml`,
  `crates/udx-test-support/Cargo.toml` (`rcgen`, `tokio-rustls`, `tokio`). Create the crates `udx-providers`,
  `udx-tools`, `udx-sandbox`, `udx-engine`, `udx-ui-model`, `udx-app`, `udx-evals`, each with `Cargo.toml` (every
  dependency and feature it will need, including target-specific and test-only ones, with exactly A 2.1's arrows) and a
  placeholder `src/lib.rs` (`src/main.rs` for `udx-app` with the bin target `unidex-rust-preview`; the bin
  `unidex-sandbox-helper` in `udx-sandbox`) holding only the crate's doc comment. In `udx-core`, create and fully
  implement (they are data, with serde): `src/{model.rs,policy.rs,content.rs,workspace.rs,engine.rs,ask.rs,sandbox.rs}`,
  `src/events/{mod.rs,engine_event.rs,session_record.rs,summary.rs}`, and the id and attendance types in
  `src/{ids.rs,attendance.rs}`; create every other `udx-core` module of A 3.1 (`state.rs`, `config/`, `display.rs`,
  `opener.rs`, `net.rs`, `secret/`, `process_env.rs`, `diag.rs`, `harden.rs`, `fsguard/`) with its public items and
  bodies `unimplemented!("T<n>")` naming the owning task. Tests: `crates/udx-core/tests/{events_golden.rs,crate_map.rs,
  fake_supervisor.rs}`, `crates/udx-core/tests/golden/events/v1/*.ndjson`,
  `crates/udx-core/tests/golden/session-log/v1/*.jsonl`.
- **Consumes.** T1b's paths module. **Produces.** A 3.1.0 whole; A 3.1.4, 3.1.5 and 3.8's types, including the
  `Supervisor` trait and `FreeSpaceWatch` (A 3.1.5); `udx_core::sandbox::testing::{FakeSupervisor, SupervisorCall,
  RecordingKiller}` written whole, not as stubs, so T19 and T21 use the fake supervisor without any sandbox task; the
  declarations of `SystemClock` and `V7Ids` (A 3.1.2; T3 writes their bodies); A 4's event and record types with A
  4.1's serde shape; every manifest, with the dev-dependency edges of A 2.2 (`udx-tools` to `udx-sandbox` with
  `testing` among them). Features: `testing` on `udx-core`, `udx-providers`, `udx-sandbox`,
  `udx-tools`, `udx-engine`, each defining its `UNIDEX_TESTING_MARKER_<crate>` static; on `udx-app`, `automation`
  (enables `gpui/test-support`), `perf` (enables `gpui/profiler`, `udx-providers/testing` and `udx-core/testing`, A 3.13)
  and `kit-renderer` (empty until question 23 says otherwise).
- **Tests:**
  - `golden_engine_events_v1` and `golden_session_records_v1`: every variant serialises to the committed golden line
    and back, with a fixed clock and sequential ids; golden paths are synthetic (`/work/example`,
    `C:\work\example`).
  - `value_enums_adjacently_tagged` (each nested value enum of A 4.1 round-trips, unit, newtype and struct variants);
    `unknown_record_type_kept`, `unknown_field_ignored`, `newer_version_refused`, `known_type_bad_body_is_corrupt`;
    `new_optional_field_absent_keeps_golden` (an added `Option` field left `None` leaves the golden line unchanged).
  - `crate_map_arrows_exact`: from `cargo metadata`, each workspace crate's normal workspace dependencies equal A 2.1's
    table exactly; the graph of normal and dev-dependencies between workspace crates has no cycle, dev-dependencies
    counted; and `udx-test-support` has no workspace dependency of any kind (A 2.2's rule). Run on synthetic metadata
    holding a dev-dependency cycle, or with `udx-test-support` depending on `udx-core`, the check fails; with
    `udx-tools` dev-depending on `udx-sandbox`, it passes.
  - `fake_supervisor_records_registrations_and_kills`: registrations, unregistrations, `kill`, `end_session`,
    `end_all` and a free-space watch and its drop are recorded in order; `end_session` calls `kill_group` on that
    session's registered `RecordingKiller`s and on no other session's; `live` and `holding_network_or_credentials`
    answer from the registered `ProcessInfo`; `as_registry` returns the same object.
  - `golden_files_pass_both_scans`: the golden files hold no value either scanner flags (the test-vector convention).
- **Done when.** The common commands, including `cargo deny --locked check` with every new crate (the pull request
  lists every crate added to the lockfile and its licence, and the `rcgen` version and licence read with `cargo info`);
  `cargo build -p udx-app --bin unidex-rust-preview --release` compiles with no feature;
  `cargo tree -e features -p udx-app` shows no `test-support` in that build.
- **Platform notes.** The target-specific dependencies (`landlock`, `seccompiler`, `oo7` on Linux; `objc2-security` on
  macOS; the `windows-sys` features on Windows) are proven to build only by CI on each platform. Nothing here changes OS
  state.

### Task 2b: Contracts: the stubs of every other crate, wired

- **Role and budget.** Sonnet builder, Opus reviewer. No separate test author: the compile and two source checks are
  its tests. Code unit, 1.0M.
- **Needs.** T2a. **Runs beside.** Nothing (it touches every crate's sources).
- **Files.** Create every module file of A 3.2 to 3.15 in `crates/{udx-providers,udx-tools,udx-sandbox,udx-engine,
  udx-ui-model,udx-app,udx-evals}/src/**`, replacing T2a's placeholder `lib.rs` (or `main.rs`) with the full module
  list; `crates/udx-tools/src/registry.rs` with `m1_tools()` written whole (wiring only, A 3.4);
  `crates/udx-engine/src/wiring.rs` with `engine_deps` as a stub naming T23 and its `pub mod wiring;` line in
  `udx-engine`'s `lib.rs` (A 3.11), so T23 edits only `wiring.rs`; `crates/udx-sandbox/src/entries.rs` with
  `access_entries` and `clean_access_entries` as stubs naming T10, and the crate-private Windows hooks `remove_entry`
  and `delete_profile` as stubs naming T12 (A 3.6); each crate's
  `src/testing.rs` (or `testing/mod.rs`) with its testing items as stubs; `crates/udx-cli/src/{main.rs,chat.rs,run.rs,
  config.rs,doctor.rs,trust.rs,key.rs,sandbox.rs,tty.rs,render.rs,wire.rs}` with every M1 subcommand registered and
  dispatching to stubs; `crates/udx-core/tests/stubs.rs`. No manifest changes.
- **Consumes.** T2a's types and manifests. **Produces.** Every public item of A 3.2 to 3.15 as a stub whose body is
  `unimplemented!("T<n>")` naming the owning task; the `udx-app` binary's `main` that parses no flags yet; the hidden
  argument `--udx-sandbox-probe <check>` dispatching to `udx_sandbox::probe_main` (stub) in both binaries' `main`.
- **Tests:** `every_stub_names_its_task` (a source scan asserting each `unimplemented!` carries a task id that exists
  in this plan); `public_items_match_contract_names` (a source scan for each public type named in A 3 and A 4, by its
  home path).
- **Done when.** The common commands; the stubs compile with exactly the crate map's dependency arrows
  (`crate_map_arrows_exact` from T2a passes with no manifest edit); `cargo build -p udx-app --bin unidex-rust-preview
  --release` still has no `test-support`.
- **Platform notes.** Target-specific stubs (`#[cfg(target_os = ...)]` backends) compile on their CI platform only.
  Nothing here changes OS state.

## Wave 1: core foundations

### Task 3: Config, state paths, ids, clock, attendance and the workspace key

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** T2b. **Runs beside.** T4, T5.
- **Files.** `crates/udx-core/src/{state.rs,ids.rs,attendance.rs,workspace.rs}` (functions only; the types are T2a's),
  `crates/udx-core/src/config/{mod.rs,schema.rs,interactive.rs}`; tests
  `crates/udx-core/tests/{config.rs,state.rs,attendance.rs,workspace_key.rs}`;
  `docs/decisions/0026-state-root-before-the-vault.md`.
- **Consumes.** `AppPaths`, `check_test_path`, `os_side_effects_allowed` (T1b). **Produces.** A 3.1.1 (`StatePaths`),
  A 3.1.2 (`IdGen`, `Clock`, their production `V7Ids` and `SystemClock`, `detect_cli_attendance`), A 3.1.3
  (`Config`, `KeyPath`, `ConfigEdit` with its target path, `apply` taking `Env`, `InteractiveProof`), A 3.1.0's
  `workspace_key`.
- **Tests:**
  - `config_rows_cover_every_section_with_source` (default, user file with line, env, flag, workspace narrowing).
  - `config_unknown_key_reported_with_line_not_an_error`; `config_malformed_toml_reports_line` (hostile input: a
    truncated file, a duplicate key, a 10 MB file, a key holding control characters).
  - `config_plan_set_shows_before_after_and_writes_nothing`; `config_apply_is_atomic_and_owner_only` (mode 0600 on
    Unix; the access list on Windows); `config_apply_keeps_comments_and_order`;
    `config_apply_refuses_path_outside_test_root`.
  - `config_host_change_plans_key_clear`; `config_interactive_key_without_proof_refused` (P22, P47, P24 rows).
  - `p47_mode_widening_is_interactive_only` (`read_only` to `ask`, `ask` to `auto` need proof; narrowing does not);
    `config_env_keys_hold_no_interactive_key`.
  - `workspace_narrowing_allow_rules_dropped_with_warning` (P47 input side).
  - `state_paths_all_inside_test_root` (through `check_test_path`); `state_dirs_created_0700`.
  - `ids_v7_sortable`, `fixed_ids_sequential_in_testing`.
  - `workspace_key_includes_device_and_inode` (a symlinked spelling gives the same key; a new folder at a reused path
    after deletion gives a different one; A D24).
  - `p41_attendance_unattended_when_stdin_not_terminal` and `..._when_no_tty_device` (Unix pseudo-terminal in the
    test; Windows console logic unit-tested).
- **Done when.** The common commands.
- **Platform notes.** Owner-only file checks differ per platform; Windows access-list assertions run on the Windows
  runner. The pseudo-terminal tests are Unix only. Nothing here changes OS state.
- **Record.** 0026 (the state root before the vault, D11, with the undo-copy location that differs from P3's
  ".unidex/" wording, and the workspace key, D24).

### Task 4: Display escaping, the opener and address classification

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Small code unit, 0.7M.
- **Needs.** T2b. **Runs beside.** T3, T5.
- **Files.** `crates/udx-core/src/{display.rs,opener.rs,net.rs}`; tests
  `crates/udx-core/tests/{display.rs,opener.rs,net.rs}`; `crates/udx-core/tests/vectors/addresses.toml`.
- **Consumes.** T2a's types. **Produces.** A 3.1.9 (with `HostPort`), A 3.1.10.
- **Tests:**
  - `display_escapes_c0_c1_del_bidi_and_zero_width`; `display_keeps_ordinary_cjk_arabic_emoji` (escaping is not
    over-eager).
  - `display_host_non_ascii_shows_ascii_and_unicode` (A D15).
  - `p54_file_and_custom_schemes_refused`, `p54_credentials_in_url_refused`,
    `p54_shell_metacharacters_reach_opener_as_one_argument` (the launcher is a test double recording argv),
    `p54_untrusted_needs_confirmation`, `opener_reveals_executable_and_model_html`.
  - `net_address_spellings_classified` over the vector table: integer, octal, hex, short IPv4, IPv4-mapped and
    -compatible IPv6, bracketed forms, trailing dots, the metadata-service address, every private, loopback,
    link-local, shared and documentation range. Following the test-vector convention, the vector file stores private,
    shared, link-local and metadata addresses as octet arrays or integer spellings, and the test builds their dotted
    forms at run time; no allow entry is added.
  - `p24_non_loopback_http_provider_refused`, `provider_endpoint_with_credentials_refused`.
- **Done when.** The common commands; the vector file passes the privacy scan with no allow entry.
- **Platform notes.** The opener's real launch is never exercised in tests (no window, no browser): the launcher is
  injected. Nothing here changes OS state.

### Task 5: File guard

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M.
- **Needs.** T1b, T2b. **Runs beside.** T3, T4, T6, T7.
- **Files.** In `crates/udx-core/src/fsguard/`: `mod.rs`, `deny.rs`, `read_allow.rs`, `persistence.rs`, `protected.rs`,
  `open_unix.rs`, `open_windows.rs`, `write.rs`, `undo.rs`, `private.rs`, `space.rs`, `workspace.rs`; tests
  `crates/udx-core/tests/{fsguard.rs,fsguard_props.rs,fsguard_race.rs,fsguard_windows.rs}`.
- **Consumes.** `check_test_path`, `home_dir` (T1b). **Produces.** A 3.1.8 (`FileGuard`, `GuardConfig` with `home` from
  `paths::home_dir`, `DenyEntry`, `UndoRef`, `UndoStore`, `create_private_dir`/`create_private_file` taking `Env`,
  `free_space`, `check_workspace_root`, `SandboxLists`).
- **Verify first.** `openat2` with `RESOLVE_BENEATH` and `RESOLVE_NO_SYMLINKS` in `rustix` 1.1.6 (its source has
  `fs/openat2.rs`) and the runner kernel's support; `O_NOFOLLOW_ANY` (present in `libc` 0.2.191 for Apple targets);
  `NtCreateFile` relative opens with `FILE_OPEN_REPARSE_POINT` (present in `windows-sys` 0.61.2 under
  `Wdk::Storage::FileSystem`); `F_GETPATH`, `/proc/self/fd`, `GetFinalPathNameByHandleW` for reading the final path
  back; `GetLongPathNameW` for short names.
- **Tests:**
  - `guard_property_paths_never_escape` (proptest over case variants, NFC and NFD, `..`, symlink and hard-link chains,
    Windows junctions on the Windows runner): no accepted path resolves outside the workspace or onto a denied entry.
  - `deny_list_matches_as_asked_and_as_resolved` (a symlink `docs/assets` to a fake `.ssh` in a temp home;
    `.SSH/Id_Rsa`; a decomposed `.env`); `denied_folder_matched_by_device_and_inode`.
  - `p9_swap_race_never_returns_target` (a helper thread swaps a folder and a symlink in a loop for 2 s while reads
    run); `p9_windows_forms_refused_without_network` (UNC to a share on `attacker.example.com`, `\\?\`, `\\.\`, `NUL`,
    `file.txt:stream`, trailing dot and space; a loopback listener records no connection);
    `p9_short_name_expanded_before_compare` (Windows: an 8.3 spelling of a denied folder is denied);
    `p9_write_through_symlink_lands_on_target_keeping_mode_and_xattrs`; `p9_hard_linked_other_name_keeps_old_content`.
  - `p8_protected_paths_classified` (each `Protected` kind, including a worktree's common git folder, a `gitdir:` target
    and a configured `core.hooksPath` whose folder does not exist); `p8_other_agents_instruction_files_protected`
    (`CLAUDE.md`, `.github/copilot-instructions.md`, `.cursorrules` at any depth are protected for writes);
    `p8_file_tools_never_create_links_or_exec_bits` (a write never sets the executable bit, even over an executable
    file it replaces).
  - `p3_overwrite_undone_byte_for_byte`, `p3_edit_undone_byte_for_byte` (through `UndoStore::restore`).
  - `p39_private_files_mode_0600_or_owner_acl`, `p39_create_through_planted_symlink_fails`,
    `private_create_refuses_path_outside_test_root`.
  - `p48_home_as_workspace_needs_typed_confirmation`, `p48_root_and_parents_refused`,
    `p48_persistence_locations_write_denied` (shell startup files, launch agents, systemd user units, autostart, the
    Windows Startup folder, global git config, the app's own config and state), `p48_missing_hooks_path_target_reported`
    (`sandbox_lists` includes the configured but missing `core.hooksPath` target in `write_deny`).
  - `p51_store_refuses_below_floor` (the floor set above the volume's free space).
  - `outside_write_needs_rule_naming_folder`; `outside_read_classified_outside`.
  - Hostile input: `guard_rejects_nul_bytes`, `guard_handles_long_paths` (Windows 260+ characters),
    `guard_link_loop_refused`.
- **Done when.** The common commands; `cargo mutants -p udx-core --file 'crates/udx-core/src/fsguard/**'` with no missed
  mutant.
- **Platform notes.** Junction, short-name and Windows-form tests run only on the Windows runner; `openat2` tests only
  on Linux; `O_NOFOLLOW_ANY` on macOS. Each falls back to a skip with a printed notice elsewhere, never a silent pass.
  Nothing here changes OS state.

### Task 6: Secrets, redaction, child environment, diagnostics and process hardening

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** T3 (for `StatePaths`). **Runs beside.** T5.
- **Files.** `crates/udx-core/src/secret/{mod.rs,redact.rs,stream.rs,macos.rs,linux.rs,windows.rs,env.rs,memory.rs}`,
  `crates/udx-core/src/{process_env.rs,diag.rs,harden.rs}`; tests
  `crates/udx-core/tests/{secrets.rs,redact.rs,process_env.rs,diag.rs,harden.rs}`; `.github/workflows/ci.yml` (the
  test jobs: set `UNIDEX_TEST_OS_SIDE_EFFECTS` to `secret_store,trust_store_cert,disk_image,sandbox_escape_matrix` on the
  macOS runner and `secret_store,windows_acl,app_container_profile,sandbox_escape_matrix` on the Windows runner, and
  nothing on Linux, whose sandbox jobs set their own in T11's workflow).
- **Consumes.** `StatePaths` (T3), `user_var` (T1b). **Produces.** A 3.1.6 (`ChildEnv`, `scrubbed`, `BaseEnv` with its
  sealed credential map, `capture_base_env`, `resolve_login_path`), A 3.1.7 (`Secret`, `SecretStore`,
  `open_secret_store`, `Redactor`, `StreamRedactor`, `harden_process`), A 3.1.11 (`diag`, `Level`).
- **Verify first.** That `objc2-security` 0.3.2's `SecItem` bindings cover add, copy-matching and delete for generic
  passwords; otherwise use `security-framework` (transitive already) and say so (A 2.3). That `oo7` 0.6.0 can be used
  with the Tokio runtime the workspace already resolves. How a Windows process opts out of error-report dumps
  (`WerAddExcludedApplication` or `WerSetFlags`, through `Win32_System_ErrorReporting`), and how to read the setting
  back.
- **Tests** (every key-shaped value assembled at run time, per the test-vector convention):
  - `redactor_redacts_value_and_each_encoding` (base64 both alphabets, padded and not; percent; hex);
    `redactor_redacts_token_shapes` (bearer header, PEM block, key prefixes); `redactor_leaves_ordinary_text`.
  - `p21_key_split_across_stream_chunks_redacted` (a key split at every byte position across two `StreamRedactor`
    pushes, in each encoding).
  - `secrets_canary_never_in_test_root`: a planted key in a tool-output string, an error message, a panic and a
    simulated crash report written through `diag`; the whole test root is scanned with `scan_for_canary` (P21, P57).
  - `secret_debug_and_display_print_redacted`; `base_env_debug_prints_names_only`.
  - `os_store_refused_without_opt_in` (with a test root and no variable, `open_secret_store` returns the memory store
    and never touches the OS store); `os_store_round_trip` (opt-in `secret_store`, CI only: macOS keychain and Windows
    Credential Manager; the Linux runner has no unlocked Secret Service, so `env_fallback_used_when_no_store` runs
    there).
  - `p23_agent_socket_variable_absent`, `p23_provider_key_absent_from_child_env`, `p23_loader_variables_absent`,
    `p23_parent_agent_markers_absent`, `p23_use_my_credentials_restores_only_that_variable`,
    `p23_restore_of_uncaptured_name_ignored`.
  - `login_path_resolved_off_the_start_path` (`capture_base_env` returns before the login shell finishes; a command
    waiting for it gets the inherited `PATH` minus relative and empty entries after the timeout).
  - `p57_linux_process_non_dumpable` (reads `/proc/self/status` after `harden_process`);
    `p57_windows_error_report_dump_off` (reads the opt-out back); `p57_rust_log_trace_has_no_effect`.
- **Done when.** The common commands; on CI the opt-in store test ran on macOS and Windows (named in the pull request).
- **Platform notes.** The OS store tests are CI only (opt-in `secret_store`), never on the development Mac. The
  non-dumpable test is Linux only; the error-report test is Windows only; each skips with a notice elsewhere.

### Task 7: Provider transport, fixtures, mock provider, endpoint binding, routing, and the HTTP stand-in

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** T4 (`check_provider_endpoint`), T6 (`Secret`, `Redactor`). **Runs beside.** T5.
- **Files.** `crates/udx-providers/src/{transport.rs,endpoint.rs,router.rs,set.rs,sanitize.rs,price.rs}`,
  `crates/udx-providers/src/testing/{mod.rs,mock.rs,fixture.rs,conformance.rs}`,
  `crates/udx-test-support/src/stand_in.rs`; tests `crates/udx-providers/tests/{transport.rs,router.rs,mock.rs,set.rs}`,
  `crates/udx-test-support/tests/stand_in.rs`; `docs/decisions/0024-http-and-tls-stack.md`.
- **Consumes.** `Secret`, `Redactor`, `check_provider_endpoint`, `Config`. **Produces.** A 3.2 (all but the two
  providers): `ReqwestTransport`, `HttpTransport`, `TransportError`, `BoundEndpoint`, `Router`, `RouteError`,
  `ProviderSet` (with `from_providers`),
  `sanitize_tools`, `testing::{MockProvider, RequestMatcher, FixtureTransport}`, and the conformance harness the two
  provider tasks call; A 3.14's `HttpStandIn` over HTTP and TLS with `TestCert`.
- **Verify first.** reqwest 0.13.5 without default features, with `rustls-no-provider`, `http2` and `stream`, and the
  `ring` provider installed as the process default: it builds on three platforms and verifies certificates through
  the OS (A 2.3, A D5). No proxy is read from the environment. `rcgen` with its `ring` feature generates a CA and leaf
  the stand-in serves with `tokio-rustls`.
- **Tests:**
  - `p22_host_change_finds_no_key` (the key name includes the host); `p24_https_required_for_remote_endpoint`.
  - `p57_secret_header_absent_from_debug_and_fixtures`; `p57_trace_level_provider_call_leaves_no_authorization` (with
    `RUST_LOG=trace`, a mock exchange leaves no `authorization` anywhere under the test root).
  - `transport_refuses_redirects_and_env_proxy` (against the loopback stand-in with `HTTPS_PROXY` set to a dead port).
  - `router_falls_back_only_between_turns`; `router_reports_reason`; `provider_set_from_config`;
    `provider_set_from_providers` (a set holding the mock provider routes through `Router::from_config`).
  - `mock_provider_replays_script_and_records_requests`; `mock_first_delta_delay_honoured`;
    `mock_request_matcher_mismatch_fails_call`.
  - `fixture_transport_rejects_request_mismatch`.
  - `stand_in_records_requests`, `stand_in_tls_with_generated_cert` (a client trusting the generated CA from a file
    connects; one that does not, fails), `stand_in_serves_redirects_slow_gzip_and_huge_bodies`.
- **Done when.** The common commands.
- **Platform notes.** TLS is exercised only against the loopback stand-in with a certificate generated at run time (no
  real host, no key material committed); OS trust-store verification is proven by building and by a unit test of the
  verifier's configuration. Nothing here changes OS state.
- **Record.** 0024.

## Wave 2: providers, sandboxes, permissions, context, egress, log and spend

### Task 8: Anthropic provider

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** T7. **Runs beside.** T9, and any wave-2 task.
- **Files.** `crates/udx-providers/src/anthropic/{mod.rs,sse.rs,wire.rs,prices.toml}`; tests
  `crates/udx-providers/tests/anthropic.rs`; fixtures `crates/udx-providers/tests/fixtures/anthropic/*.json`.
- **Consumes.** T7's transport, fixtures and conformance harness. **Produces.** `AnthropicProvider` (A 3.3).
- **Verify first.** Every "not verified" item for Anthropic in A 3.3, from the vendor's published API reference; the
  pages and dates go in the pull request. Prices only from the vendor's published page, with the date (A 5); none if it
  cannot be read.
- **Tests:** the conformance harness's cases over the fixtures (text stream, one tool call with streamed arguments, two
  parallel tool calls, usage with cache counts, rate limit with `retry-after`, overloaded, malformed event, stream cut
  mid-event, empty response, refusal); `anthropic_reasoning_block_round_trips_unchanged` (an opaque block with its
  signature is sent back byte for byte); `anthropic_key_never_in_request_body_or_url`; `anthropic_error_body_parsed`;
  `anthropic_count_tokens_exact_flag`.
- **Done when.** The common commands. Fixtures are hand-written from the reference, or recorded by the maintainer's
  opt-in run; either way reviewed for secrets before commit, with low-entropy invented values for ids and signatures
  (the test-vector convention), and passing both scans.
- **Platform notes.** Pure Rust over fixtures; all three CI platforms. Nothing here changes OS state.

### Task 9: Ollama provider and the prompt-integrity measurement

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** T7. **Runs beside.** T8.
- **Files.** `crates/udx-providers/src/ollama/{mod.rs,ndjson.rs,wire.rs,integrity.rs}`; tests
  `crates/udx-providers/tests/ollama.rs`; fixtures `crates/udx-providers/tests/fixtures/ollama/*.json`;
  `docs/decisions/0018-m1-providers.md`.
- **Consumes.** T7's transport, fixtures and conformance harness. **Produces.** `OllamaProvider`, its `probe`
  (recognition through `/api/version`) and `prompt_integrity` (A 3.3).
- **Verify first.** Every "not verified" item for Ollama in A 3.3. The measurement: whether `prompt_eval_count` counts
  only tokens not served from the prompt cache. It is made by the maintainer's opt-in live run if approved; otherwise
  from the server's published documentation and source (Ollama is MIT-licensed), recorded with its limits. The P59
  formula, or the guarantee mode of A 3.3, is chosen from it and written in record 0018.
- **Tests:** the conformance cases; `p59_ollama_fixture_cut_detected` (a fixture whose reported prompt count is below
  the lower bound); `ollama_num_ctx_sent_as_set`; `ollama_tools_capability_read_from_show`;
  `ollama_probe_recognises_server` and `ollama_probe_unrecognised_on_other_json`;
  `ollama_tool_call_without_id_gets_engine_id`; `review_focus_local_model_quirks_provider` (Review Focus 4, provider
  half; fixtures `crates/udx-providers/tests/fixtures/ollama/quirk_*.json`): two parallel tool calls without ids get
  distinct engine-made ids (`udx-<uuid>`); a model whose `/api/show` reports a small context window yields it in
  `ModelInfo.context_window` and sends `num_ctx` as set; assistant text that imitates the prompt protocol's
  delimiters arrives as `TextDelta` unchanged (the provider parses no call from text); a reported prompt count below
  the lower bound gives `PromptIntegrity::Cut`. The `StreamEvent` sequence each fixture yields is asserted in full,
  so T23's engine half can script the same sequence.
- **Done when.** The common commands.
- **Platform notes.** Pure Rust over fixtures; all three CI platforms. Nothing installed; the live run is the
  maintainer's opt-in only.
- **Record.** 0018 (both providers, citing section 5 of A).

### Task 10: macOS sandbox, the process supervisor and the probe framework

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M.
- **Needs.** T5 (`sandbox_lists`), T6 (`process_env`). **Runs beside.** T13 to T17, T8, T9.
- **Files.** `crates/udx-sandbox/src/{supervisor.rs,probe.rs,limits.rs,space_watch.rs,entries.rs,macos/mod.rs,
  macos/profile.sb,macos/sweep.rs,macos/denials.rs}`, `crates/udx-sandbox/src/testing.rs`; tests
  `crates/udx-sandbox/tests/{macos.rs,supervisor.rs,probe.rs,entries.rs,escape_macos.rs}`;
  `docs/decisions/0017-macos-sandbox.md`.
- **Consumes.** `SandboxLists`, `ChildEnv`, `StatePaths`, the `Supervisor` and `ProcessRegistry` traits (T2a).
  **Produces.** `ProcessSupervisor` (implementing `udx_core::sandbox::{ProcessRegistry, Supervisor}`, with
  `watch_free_space`), `KillHandle` and `GroupKiller` implementations, `probe_main` and the probe used by every backend
  (`ProbeReport`, `ProbeOutcome`), `testing::UnavailableSandbox` (the fake supervisor is T2a's, in `udx-core`),
  `access_entries` and `clean_access_entries` working on every platform (A 3.6, "The access-entry functions on every
  platform": the records file, the scope filter, the rewrite, the Windows hooks called only under `cfg(windows)`), the
  macOS backend (A 3.6) with its networked profile, D2's marker sweep.
- **Verify first** (each on the macOS CI runner, recorded in record 0017): the seatbelt operation and filter names that
  keep `process-info*`, `signal` and `sysctl-read` to the sandbox's own processes and a named sysctl list (not
  `kern.procargs2`); the `mach-lookup` names the certificate trust service and name resolution need, and whether name
  resolution needs the resolver's Unix socket; whether an unprivileged process can read the unified log's sandbox
  violation records for its child (if not, `reports_denied_writes` is false and no re-run is ever offered); that
  `(remote ip)` and `(local ip)` filters exclude Unix-domain sockets.
- **What it builds (A 3.1.5, A 3.6).** The profile of A 3.6's macOS item; the networked profile (network over IP only,
  never a `unix-socket` filter or an unfiltered `network*`, Unix sockets only in the session temp folder and the exact
  `credential_sockets`); `read_deny` as regex rules compiled from each `DenyEntry`; `write_deny` as path rules, which
  cover absent paths; the probe checks of A 3.1.5's table with outcomes that name the specific denial; the uniform
  process lifetime (a group lives until the supervisor ends it or it is empty); `KillHandle` usable while `wait` is
  pending. `probe.rs` runs the checks through a private trait each backend implements in its own folder, and
  `supervisor.rs` holds only `KillHandle`s, so T11 and T12 add no lines to either file.
- **Tests** (macOS; each escape payload only writes a marker in the test root if it succeeds):
  - `macos_probe_full_on_runner`; `p7_probe_check_allowed_makes_unavailable` (a deliberately broken profile in a test
    build is reported `Unavailable(ProbeCheckFailed)`); `p7_networked_check_allowed_disables_network_grants_only`;
    `probe_outcomes_name_the_denial` (no outcome is a bare failure; a missing resource is `NotApplicable`).
  - `p1_no_loopback_connect`, `p1_no_name_lookup`.
  - `p43_escape_matrix_macos` (`open`, `osascript`, `launchctl`, an `lsopen` attempt) and
    `p43_escape_matrix_networked_macos` (a stand-in Unix socket the test binds outside the workspace refuses the
    connect; `mach-lookup` of a denied service still fails), both CI only (opt-in `sandbox_escape_matrix`).
  - `p43_networked_https_ok_secret_store_denied_macos` (https to the TLS stand-in with the test CA in a temporary
    keychain under the opt-in `trust_store_cert`, CI only; the security server's `mach-lookup` fails with the sandbox's
    denial in the same profile).
  - `p23_secret_store_unreachable_macos` (specific denial; under the opt-in `secret_store` on CI, an item the test
    planted is not returned); `p23_other_process_env_unreadable_macos` (a process the test starts outside the sandbox
    with a canary variable; `kern.procargs2` on it fails from inside);
    `p23_use_my_credentials_socket_reachable_only_when_granted` (a stand-in agent socket under the test root).
  - `p10_planted_bash_history_unreadable`, `p10_unlisted_token_file_unreadable`,
    `p10_session_logs_unreadable_from_sandbox`, `p10_sandbox_read_of_in_tree_deny_listed_file_fails` (an `.env` and a
    `*.pem` inside the workspace, including one created during the command).
  - `p8_sandbox_write_to_git_hooks_fails_commit_succeeds` (a temp repository; `git commit` inside the sandbox works);
    `p8_sandbox_write_to_absent_protected_path` (an absent `.github/workflows/x.yml` and a missing `core.hooksPath`
    folder stay uncreated); `denied_write_reported_when_log_readable` (or, if the log cannot be read,
    `reports_denied_writes_false_and_no_rerun`).
  - `p48_sandbox_writes_to_startup_launch_agent_fail` (temp home).
  - `p50_macos_group_killed_and_marker_sweep_kills_daemon` (a command that double-forks and calls `setsid` is gone after
    `end_session`; CI only, opt-in `sandbox_escape_matrix`); `p50_live_processes_listed` (a background process outlives
    its command and is listed until the session ends); `p50_kill_handle_works_while_wait_pending`.
  - `p51_macos_process_limit_reads_back_count_plus_margin`; `p51_wall_time_kills_group`;
    `p51_unattended_fill_kills_group_with_event` (a small disk image as the test volume, opt-in `disk_image`, CI only);
    `p51_macos_memory_limit_absent_reported` (pins residual P51-a: `BackendDetails::memory_limit` is false).
  - `profile_parameters_not_interpolated` (a workspace path containing quotes and parentheses still yields a valid
    profile and confines correctly).
  - `children_waited_by_pid` (two children exit in either order; each wait returns its own status; a third unrelated
    child is not reaped).
  - `free_space_watch_idle_when_no_unattended_group` (no timer is registered while nothing runs).
  - `access_entries_missing_file_is_empty` and `access_entries_reads_records` (three platforms);
    `clean_workspace_scope_only_its_records` (records planted for two workspaces in the test root: `Workspace(dir)`
    removes only that one's, `All` removes every record, capability entries included) and
    `clean_failure_keeps_record_and_reports`, both unit tests of `entries.rs` with a remover injected through a
    crate-internal seam, so they run on three platforms and on Windows reach no T12 hook before T12 merges;
    `clean_drops_records_without_os_entries` (macOS and Linux, through the public function: a planted record is
    dropped from the file and nothing else changes).
- **Done when.** The common commands, run locally on macOS (only the tests whose effects stay in the test root) and
  green on the macOS CI runner with the opt-in tests.
- **Platform notes.** macOS only for the backend; `supervisor.rs`, `probe.rs`, `space_watch.rs` and `entries.rs` are
  platform-neutral and unit-tested on three platforms. CI only: the escape matrices, the daemon sweep, the trust-store
  certificate, the disk image and the planted secret-store item. Locally: write outside the workspace aimed at a file
  in the test root, loopback connect to the test's own listener, planted files under a `TempHome`, profile quoting,
  the probe.
- **Record.** 0017 (the macOS profile, D2, the networked profile's part of D28, and what the verify-first steps found).

### Task 11: Linux sandbox: namespaces, Landlock, seccomp and the AppArmor profile

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M (the controller should
  expect this one to be the most likely to need its reserve; see "Risks").
- **Needs.** T10. **Runs beside.** T12 (different files; both verified in CI only).
- **Files.** `crates/udx-sandbox/src/linux/{mod.rs,helper.rs,mounts.rs,masks.rs,seccomp.rs,landlock.rs,cgroup.rs}`,
  `crates/udx-sandbox/src/linux/resolver.rs`, `crates/udx-sandbox/src/bin/unidex-sandbox-helper.rs`;
  `packaging/linux/apparmor/unidex-rust-preview-sandbox`; tests
  `crates/udx-sandbox/tests/{linux.rs,escape_linux.rs,linux_resolver.rs}` (the last with the test's own small DNS
  stand-in, no new crate); `.github/workflows/sandbox-linux.yml`; `docs/decisions/0015-linux-sandbox.md`.
- **Consumes.** T10's supervisor, probe framework and `KillHandle`. **Produces.** The Linux backend and helper (A 3.6),
  D1's probe-decided availability (decided by the maintainer on 2026-11-17), the Linux part of D28, name resolution in
  the networked root (A 3.6, "Name resolution in the Linux networked root"), the probe check `NetworkedResolverConfig`,
  and the single-socket binding of `credential_sockets`.
- **Verify first** (on the Ubuntu runner, in this order, before building):
  1. Name resolution for a networked command, which nothing in the repository verifies yet: the target of the runner's
     `/etc/resolv.conf` link and the nameservers its content names (a loopback stub resolver is expected on Ubuntu's
     default configuration); the `hosts:` line of its `/etc/nsswitch.conf` and that the C library falls through to the
     `dns` source when the modules' sockets under `/run` are absent; that the helper can place a regular file at the
     new root's `/etc/resolv.conf` beside the read list's `/etc` entries; and, in a prototype networked root holding
     the copy and no `/run`, that a lookup reaches the step's own stand-in resolver on a loopback address (no lookup
     leaves the runner). What a default desktop or server install does differently, if anything, cannot be seen on CI
     and is recorded as not verified. If no sound way exists without the host's `/run`, the task stops and reports, and
     the contract of A 3.6 applies: network grants unavailable there with the reason.
  2. Which Landlock ABI the runner's kernel offers and whether Landlock is enabled, because it decides which networked
     case applies (ABI 6 or higher: network grants available with the abstract-socket and signal scopes; below:
     `NetworkGrants::Unavailable(LandlockScopeMissing)` and the networked tests assert the refusal).
  3. Whether `kernel.apparmor_restrict_unprivileged_userns` is on by default; the AppArmor profile syntax that grants
     `userns` to one binary on that AppArmor version; whether a delegated cgroup is writable for the runner's user.
- **What it builds (A 3.6).** The helper as the PID namespace's first process (it reaps orphans, so a background
  process outlives its command as on the other platforms), its arguments ended by `--`; a network namespace only for
  commands without network; for a networked command, Landlock ABI 6 with `Scope::AbstractUnixSocket` and
  `Scope::Signal` required, and a new root that never contains `/run`, `/var/run`, the host's `/tmp` or `/dev/shm`,
  `$XDG_RUNTIME_DIR` or the home folder (a read-list entry that is or contains one is refused when the profile is
  built); a fresh `/proc`; masks over each existing `read_deny` match and read-only mounts over each existing
  `write_deny` path, from a walk before each command (cap `limits.deny_walk_entries`, default 500,000; past it every
  command in that workspace asks); a before-and-after check of `write_deny` paths filling `ExitInfo::protected_created`;
  the seccomp filter of A 3.6 (including `clone3` refused with `ENOSYS`, `io_uring_*`, `unshare`, `setns`); closing
  inherited descriptors before exec; for a networked command, the read-only copy of the host's resolver configuration
  at `/etc/resolv.conf` (never the host's link or its folder) with the per-spawn failure and the probe check of A 3.6;
  each granted `credential_sockets` path bound alone, never its folder.
- **`sandbox-linux.yml`.** Two jobs on `ubuntu-26.04`: `sandbox-linux (restricted)` sets the sysctl to 1, installs no
  profile, and runs `cargo test --workspace --locked -- p7_` (every P7 test, so T21's shell-tool case runs there with
  the real, unavailable backend once T21 has merged); `sandbox-linux (profile)` sets the sysctl to 1, installs the
  profile from `packaging/linux/apparmor/` with the helper at the profile's path, loads it with `apparmor_parser -r`,
  lowers `net.ipv4.ip_unprivileged_port_start` to 53 on the runner (so the resolver test's stand-in can bind port 53 on
  a loopback address without privileges; a disposable runner's setting, never the development Mac's), sets
  `UNIDEX_TEST_OS_SIDE_EFFECTS=sandbox_escape_matrix`, starts a test Xvfb on a test display, and runs
  `cargo test --workspace --locked`, so every crate's sandbox-dependent tests (the shell tool, the engine integration,
  the evaluation suite's shell tasks) run with the full sandbox once their tasks merge. Both upload the probe's report.
- **Tests:**
  - `p7_linux_restricted_userns_without_profile_is_unavailable` (restricted job): the probe reports
    `Unavailable(UserNamespacesRestricted)` with the remedy text. That the shell tool is then absent is T21's
    `p7_forced_unavailable_shell_absent_and_refuses`, which runs in this job with the real backend once T21 has
    merged; no `udx-sandbox` test uses `udx-tools`, because `udx-tools` dev-depends on `udx-sandbox` (A 2.2).
  - Name resolution (A 3.6), profile job: `p43_networked_name_lookup_uses_resolver_copy_linux`,
    `resolver_copy_follows_link_never_binds_folder`, `p7_resolver_config_failure_disables_network_grants_only` and
    `networked_resolver_config_probe_check`, as A 3.6 describes them; where network grants are unavailable on the
    runner (Landlock below ABI 6), the cases that run a networked command are replaced by the refusal, as for the
    networked escape matrix.
  - `p23_use_my_credentials_socket_reachable_only_when_granted_linux` (profile job, CI only; the Linux counterpart of
    T10's macOS test): a stand-in agent socket in a folder of the test root outside the workspace, beside a second
    socket; without the grant neither is in the new root; with it, the granted socket is connectable at its own path
    and the second one is still absent, with and without a network grant.
  - `linux_probe_full_with_profile` (profile job); `p7_network_grants_unavailable_below_landlock_abi_6` (a test build
    reporting ABI 5).
  - `p1_no_loopback_connect`, `p1_no_name_lookup` (neither the stub resolver nor the system resolver's socket is
    reached).
  - `p43_escape_matrix_linux` (`systemd-run --user`, `gio open`, an X11 connection to the test Xvfb by pathname and by
    abstract socket, `TIOCSTI` into the controlling terminal, an abstract socket in the parent namespace) and
    `p43_escape_matrix_networked_linux` (with a network grant: X11 abstract and pathname, a parent-namespace abstract
    socket, a stand-in Unix socket outside the workspace), run where network grants are available, otherwise replaced
    by the refusal test above; `p43_networked_https_ok_secret_store_denied_linux` (`SSL_CERT_FILE` points at the test
    CA in the session temp folder; no OS change); `p43_workspace_socket_from_outside_reachable_residual` (pins residual
    P43-a).
  - `p23_secret_store_unreachable_linux` (no session bus in the root and the abstract bus refused, or `NotApplicable`
    when the runner has none); `p23_other_process_env_unreadable_linux` (a process outside with a canary variable is
    absent from the namespace's `/proc`).
  - `p10_planted_bash_history_unreadable`, `p10_unlisted_token_file_unreadable`,
    `p10_session_logs_unreadable_from_sandbox`, `p10_sandbox_read_of_in_tree_deny_listed_file_fails`,
    `p10_deny_scan_past_cap_makes_commands_ask`, `p8_sandbox_write_to_git_hooks_fails_commit_succeeds`,
    `p8_sandbox_write_to_absent_protected_path` (reported in `protected_created`),
    `p48_sandbox_writes_to_startup_systemd_autostart_fail`.
  - `p50_double_fork_setsid_gone_at_run_end`; `p50_live_processes_listed`.
  - `p51_fork_bomb_contained`; `p51_tmpfs_size_enforced`; `p51_unattended_fill_kills_group_with_event` (a sized tmpfs
    as the test volume).
  - `seccomp_blocks_keyctl_new_namespaces_clone3_and_io_uring`; `helper_closes_inherited_descriptors`;
    `landlock_missing_makes_unavailable` (a test build that reports ABI 0).
- **Done when.** The common commands locally (the Linux code is checked on macOS only through `cargo check --target`,
  whose target is not installed; installing it needs the maintainer's approval, so by default the controller relies on
  CI and says so); both `sandbox-linux` jobs green.
- **Platform notes.** Linux, CI only. Wayland and X11 sockets are tested with a test Xvfb and a test socket path, never
  a real display. The escape matrices run under the opt-in set by the workflow.
- **Repository settings.** The maintainer adds `sandbox-linux (restricted)` and `sandbox-linux (profile)` to the
  required checks.
- **Record.** 0015 (citing the maintainer's answer to D1; the Landlock ABI found on the runner; the networked profile,
  with what the first verify-first step found about name resolution).

### Task 12: Windows sandbox: AppContainer, job object and the access-list footprint

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M.
- **Needs.** T10. **Runs beside.** T11.
- **Files.** `crates/udx-sandbox/src/windows/{mod.rs,appcontainer.rs,job.rs,acl.rs,grants.rs,capability.rs}`; tests
  `crates/udx-sandbox/tests/{windows.rs,escape_windows.rs,acl.rs}`; `docs/decisions/0016-windows-sandbox.md`.
- **Consumes.** T10's supervisor, probe framework, `KillHandle` and its platform-neutral `access_entries` and
  `clean_access_entries`; `StatePaths::sandbox_dir`. **Produces.** The Windows backend (A 3.6); the write-ahead record
  writer in the records format of A 3.6; the crate-private hooks `windows::remove_entry` and `windows::delete_profile`
  behind T10's `clean_access_entries` (in `windows/`, so this task adds no line to `entries.rs`); D3 (decided by the
  maintainer on 2026-11-17), D21.
- **Verify first.** The `windows-sys` 0.61.2 features for AppContainer profiles, security capabilities in
  `STARTUPINFOEX`, job objects and `SetEntriesInAclW`; that a process without network capabilities cannot reach
  loopback on the runner; that a lowbox token's custom capability SID (from `DeriveCapabilitySidsFromName`) is honoured
  by the access check (if not, read-list entries are made per workspace SID, recorded the same way); whether
  `CredReadW` from inside the container fails with access denied rather than not-found; whether a less privileged
  AppContainer runs ordinary tools (tried first; fall back if not, recording which).
- **What it builds (A 3.6, "The Windows access-list footprint").** An AppContainer profile per workspace; entries
  written to `StatePaths::sandbox_dir/grants.toml` before they are added; the workspace root's inheritable allow
  entry; deny entries for existing protected paths and in-tree deny-list matches, refreshed before each command (walk
  cap `limits.deny_walk_entries`, default 500,000; past it every command in that workspace asks); the session temp
  folder's entry, removed with the folder; one capability SID for the home read
  list; `internetClient` only for a networked command; the job object with kill-on-close, process and memory limits,
  UI restrictions and no breakaway; the before-and-after `protected_created` check; removal through the hooks T10's
  `clean_access_entries` calls, for one workspace or all, never-trusted workspaces included; the first card's notice
  text.
- **Tests** (Windows runner; access entries and profiles under the opt-in `windows_acl,app_container_profile`):
  - `windows_probe_full_on_runner`.
  - `p1_no_loopback_connect`, `p1_no_name_lookup`.
  - `p43_escape_matrix_windows` (`schtasks /create`, WMI `Win32_Process.Create` through PowerShell, a COM activation)
    and `p43_escape_matrix_networked_windows` (with `internetClient`: a stand-in named pipe outside the container's
    access refuses the open; `schtasks`, WMI and COM still fail), CI only;
    `p43_networked_https_ok_secret_store_denied_windows` (the client trusts the test CA from a file, so this test does
    not exercise the OS trust store, and says so; `CredReadW` fails with the specific denial).
  - `p23_secret_store_unreachable_windows` (under the opt-in `secret_store`, an item the test planted is not
    returned); `p23_other_process_env_unreadable_windows` (`OpenProcess` on a process outside fails).
  - `p10_planted_bash_history_unreadable`, `p10_unlisted_token_file_unreadable`,
    `p10_session_logs_unreadable_from_sandbox`, `p10_sandbox_read_of_in_tree_deny_listed_file_fails`,
    `p10_deny_scan_past_cap_makes_commands_ask`,
    `p8_sandbox_write_to_git_hooks_fails_commit_succeeds` (if `git` runs inside the AppContainer on the runner;
    otherwise the hooks write alone, and the pull request records that `git` does not run there),
    `p8_sandbox_write_to_absent_protected_path` (reported in `protected_created`),
    `p48_sandbox_write_to_startup_folder_fails`.
  - `p50_double_fork_gone_at_run_end` (`start /b` chains and a detached child stay in the job and die with it);
    `p50_live_processes_listed`.
  - `p51_fork_bomb_contained`; `p51_job_memory_limit_kills`. (No fill test on Windows: residual P51-b, skipped with a
    notice.)
  - `acl_entries_recorded_before_added`, `acl_clean_removes_workspace_entries_and_profile`,
    `acl_never_trusted_workspace_cleaned`, `acl_home_read_list_entries_via_capability_sid`,
    `acl_other_workspace_sid_cannot_read`, `acl_revoke_removes_entries`, `no_acl_change_without_opt_in` (test root, no
    variable).
  - `acl_propagation_time_on_large_tree` (a generated 100,000-file tree; the time is recorded in 0016, not asserted).
  - `tool_compatibility_report` (not a pass/fail test: runs `cmd /c dir`, `git --version`, `cargo --version`,
    `python --version`, `node --version` inside the AppContainer and writes the results as an artifact for the gate
    report; threat model 7).
- **Done when.** The common commands; the Windows CI job green; the compatibility report and the propagation figure
  attached.
- **Platform notes.** Windows, CI only. The opt-in variable is set on the Windows test job by T6; this task does not
  edit `ci.yml`.
- **Record.** 0016 (citing the maintainer's answer to D3; the footprint; the propagation figure; tool compatibility).

### Task 13: Permissions, built-in asks, modes, the ask limiter and the command parser

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M.
- **Needs.** T3, T5. **Runs beside.** T14 to T17, T10.
- **Files.** `crates/udx-engine/src/permissions/{mod.rs,rules.rs,builtin.rs,modes.rs,limiter.rs,card.rs}`,
  `crates/udx-tools/src/shell/parse.rs`; tests `crates/udx-engine/tests/{permissions.rs,builtin_spellings.rs}`,
  `crates/udx-tools/tests/parse.rs`.
- **Consumes.** `Config`, `Attendance`, the policy types (A 3.1.0). **Produces.** A 3.7 (`Rule` with `network`,
  `PermissionEngine` with `set_mode` and `turn_off_builtin`, the conservative built-in matcher and its tables, the
  runner card), the card builder of A 3.8 (`AskCard` from an intent and a verdict, with the app-written title
  templates, `created_protected` and the first-command access-entry notice on Windows), `udx_tools::parse_command`.
- **Tests:**
  - `p47_workspace_allow_changes_nothing`; `p47_allow_always_npm_test_does_not_admit_npm_exec`;
    `p47_user_allow_git_push_still_asks`; `p47_builtin_off_for_session_with_indicator_then_asks_again`;
    `p47_cli_cannot_turn_off_builtin` (refused for `Attended(Terminal)` and `Unattended`);
    `p47_mode_widening_refused_headless` (`set_mode` past the starting mode, unattended);
    `p41_unattended_never_runs_with_builtin_off`.
  - `p3_force_push_asks_in_auto_mode`; `p3_force_push_denied_unattended`; one case per `BuiltinAsk` of P3;
    `p3_hostile_spellings_ask`, a table test over at least: `env git push --force`, `nice git push`,
    `timeout 5 git push`, `git -C . push`, `/usr/bin/git push`, `git -c alias.p=push p`, `git --git-dir=.git push`,
    `find . -delete`, `find . -exec rm {} ;`, `xargs rm -rf`, `rm -fR x`, `sudo true`, `npm publish`, `gh pr merge`.
  - `p2_compound_around_allowed_prefix_asks` (`git status; nc ...`, `git status && x`, `$(...)`, backticks,
    redirections, here-documents, a leading assignment); `p2_zsh_glob_qualifier_never_under_rule`;
    `p2_rule_wildcard_never_matches_option` (`rg *` vs `rg --pre=./x foo`); `p2_windows_every_command_asks`;
    `p2_runner_card_says_repository_code`.
  - `p1_rule_network_shown_on_card_and_scoped_to_command`.
  - Parser hostile input: `parse_never_panics` (proptest over arbitrary bytes and Unicode);
    `parse_unbalanced_quotes_needs_shell`; `parse_unicode_lookalike_operators_needs_shell` (full-width semicolon,
    U+2028).
  - `p58_turn_stops_after_denials_with_event`; `p58_asks_rate_limited`.
  - `p60_defaults_table` (each cell of A 3.7's table, per mode);
    `p60_untrusted_workspace_starts_in_ask_despite_auto_config`.
  - `precedence_deny_beats_everything`, `outside_write_needs_folder_rule`, `unattended_ask_becomes_deny_with_no_card`.
  - `card_title_is_app_words_and_subject_escaped` (a model-supplied command with a right-to-left override).
- **Done when.** The common commands; mutation testing of `crates/udx-engine/src/permissions/` and
  `crates/udx-tools/src/shell/parse.rs` with no missed mutant (run locally; CI scope arrives with T35).
- **Platform notes.** Pure logic; all three CI platforms. The Windows case of P2 is a unit test on every platform with
  `Platform::Windows`. Nothing here changes OS state.

### Task 14: Context, labels, exposure, workspace trust and instruction files

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M.
- **Needs.** T3, T5. **Runs beside.** T13, T15, T16, T17.
- **Files.** `crates/udx-engine/src/context/{mod.rs,labels.rs,exposure.rs,instructions.rs,third_party.rs,ignore.rs}`,
  `crates/udx-engine/src/trust.rs`; tests `crates/udx-engine/tests/{context.rs,trust.rs}`.
- **Consumes.** `FileGuard`, `WorkspaceInfo`, `InteractiveProof`, `Origin`, `TrustState`. **Produces.** A 3.9's context
  part: `Delimiters` (a fresh nonce per run of a session), `LabelledItem`, `exposes`, `Exposure`, `ContextError`,
  `TrustStore` (with `open`, which the engine wiring and the CLI use), `TrustError`, `resolve_instruction_set`,
  `InstructionSet`, `is_third_party`, and the `IgnoreMatcher` implementation with the `ignore` crate.
- **Tests:**
  - `labels_wrap_each_origin_kind` and `label_delimiter_in_content_escaped` (golden context tests).
  - `exposure_table_row_<n>` (one per M1 row of threat model 4.3's table).
  - `p13_untrusted_agents_md_absent_from_system`; `p13_untrusted_file_read_exposes`;
    `p13_trusted_import_change_asks_again`; `p13_trusted_node_modules_read_exposes`; `p13_submodule_not_trusted`;
    `p13_gitignored_path_exposes`; `instruction_import_outside_workspace_refused`; `instruction_import_depth_capped`;
    `instruction_import_cycle_detected`.
  - `delimiters_fresh_per_run` (a resumed session's nonce differs from the first run's).
  - `trust_requires_interactive_proof`; `trust_keyed_by_identity_not_spelling` (a symlinked spelling of the same folder
    shares trust; a different folder at a reused path after deletion does not); `trust_file_corrupt_reported_with_line`.
- **Done when.** The common commands.
- **Platform notes.** All three CI platforms; symlink cases need a Windows runner with symlink rights or are skipped
  there with a notice. Nothing here changes OS state.

### Task 15: Egress after exposure (P14) and the image-load verdict (P15)

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M.
- **Needs.** T3. **Runs beside.** T13, T14, T16, T17.
- **Files.** `crates/udx-engine/src/egress.rs`; tests `crates/udx-engine/tests/egress.rs`.
- **Consumes.** `EgressRequest`, `EgressVerdict`, `CapScope`, `Attendance` (A 3.1.0). **Produces.** A 3.9's
  `EgressState` (with `composed_ranges`), `EgressCaps`, and an `EgressGate` implementation over it for the engine.
- **Tests:**
  - `p14_composed_path_to_injecting_host_asks`; `p14_canary_subdomain_navigation_asks`;
    `p14_redirect_to_new_origin_asks`; `p14_verbatim_link_to_injecting_origin_passes_until_cap`;
    `p14_turn_total_stops_turn`; `p14_session_total_stops_later_turn`; `p14_body_always_asks`;
    `p14_shell_network_while_exposed_asks`; `p14_unattended_exposed_egress_denied`; `p14_typed_url_byte_identical_only`
    (a typed URL with one changed byte asks); `p14_host_history_never_exempts`.
  - `p15_image_from_exposed_turn_asks` (even when the current turn is not exposed); `p15_image_load_unattended_denied`.
  - `composed_ranges_marks_changed_components` (host labels, path segments, query names and values, fragment; with no
    known URL of the origin, the host and everything after it are marked; a 4 KB composed URL yields ranges covering
    exactly the composed parts).
- **Done when.** The common commands; mutation testing of `egress.rs` locally with no missed mutant.
- **Platform notes.** Pure logic; all three CI platforms. Nothing here changes OS state.

### Task 16: Session log: writer, reader, redaction and the open-from-the-end path

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** T3, T5, T6. **Runs beside.** T13, T14, T15, T17.
- **Files.** `crates/udx-engine/src/log/{mod.rs,writer.rs,reader.rs}`; tests `crates/udx-engine/tests/log.rs`;
  `docs/decisions/0019-session-log-format.md`.
- **Consumes.** A 4's record types and `LogError` (T2a), `Redactor`, `create_private_file`. **Produces.** A 3.11's
  `SessionLogWriter`, `SessionLogReader`.
- **Tests:**
  - `log_round_trips_every_record_v1` (against T2a's golden files).
  - `log_redacts_before_write` (a key in a tool result, an argument and an error never reaches the file; the whole
    test root is scanned).
  - `log_reasoning_block_changed_by_redaction_is_dropped` (the record holds `ReasoningDropped` and a warning is
    returned to the caller).
  - `log_tool_results_stored_unlabelled_and_header_holds_no_nonce`.
  - `log_file_owner_only_and_not_through_symlink` (P39).
  - `log_in_test_root_only` (`check_test_path`).
  - `log_tail_reads_last_n_without_reading_whole_file` (counts bytes read on the 5,000-message fixture);
    `log_tail_then_rest_equals_full_read`.
  - Hostile input: `log_truncated_last_line_recovered` (a crash mid-write),
    `log_corrupt_middle_line_reported_with_line`, `log_newer_version_refused`, `log_huge_line_refused` (over 16 MiB),
    `log_unknown_type_kept`.
  - `p51_log_refuses_below_floor`.
- **Done when.** The common commands.
- **Platform notes.** All three CI platforms; the owner-only check is mode 0600 on Unix and an owner-only access list on
  Windows (asserted on the Windows runner). Nothing here changes OS
  state.
- **Record.** 0019 (format v1, A 4.1's serde shape and versioning rules, location, D8).

### Task 17: Spend ledger

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Small code unit, 0.7M.
- **Needs.** T3. **Runs beside.** T13 to T16.
- **Files.** `crates/udx-engine/src/spend.rs`; tests `crates/udx-engine/tests/spend.rs`;
  `docs/decisions/0023-spend-ledger.md`.
- **Consumes.** `MicroUsd`, `SpendRefusal`, `Price`, `TokenCount`. **Produces.** A 3.10 (`SpendLedger`, `Caps`,
  `Reservation`, `DaySummary`, `SpendError`, `worst_case`).
- **Tests:**
  - `p40_two_processes_budget_for_one_call_make_one` (two OS processes started from the test binary race to reserve).
  - `p40_unpriced_remote_model_refuses`; `p40_unrecognised_loopback_refuses`; `p40_marked_free_loopback_passes`.
  - `p40_cancelled_stream_keeps_reservation`; `unreported_reservation_settled_at_worst_case_after_timeout`.
  - `per_call_cap_refuses_before_send`; `utc_day_rollover_resets_day_budget`.
  - `ledger_file_owner_only`; `ledger_survives_corrupt_wal` (hostile: a truncated WAL file).
- **Done when.** The common commands.
- **Platform notes.** All three CI platforms (bundled SQLite builds everywhere). Nothing here changes OS state.
- **Record.** 0023 (D9, D22).

## Wave 3: the engine loop, tools, integration, CLI and view-model

### Task 18: The prompt-based tool protocol (P6)

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Small code unit, 0.7M.
- **Needs.** T14 (`Delimiters`). **Runs beside.** T19 setup, T20, T21, T22.
- **Files.** `crates/udx-engine/src/protocol.rs`; tests `crates/udx-engine/tests/protocol.rs`.
- **Consumes.** `Delimiters`, `ToolSpec`, `CallId`. **Produces.** A 3.11's `PromptProtocol`, `ParsedCall`,
  `ProtocolError`.
- **Tests:** `p6_fuzz_result_text_never_yields_call` (proptest); `p6_calls_parsed_only_from_current_output`;
  `p6_nonce_differs_per_session`; `escape_result_never_contains_a_delimiter`; `parsed_call_ids_engine_made`;
  `protocol_hostile_inputs` (unterminated call, a call over 1 MiB, nested and mismatched delimiters, a delimiter with
  another session's nonce): each an error, no panic.
- **Done when.** The common commands; mutation testing of `protocol.rs` locally with no missed mutant.
- **Platform notes.** Pure logic; all three CI platforms. Nothing here changes OS state.

### Task 19: The engine loop and `NativeEngine`

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M.
- **Needs.** T7, T13, T14, T15, T16, T17, T18 (no sandbox task: the fake supervisor is `udx-core`'s, built by T2a).
  **Runs beside.** T20, T26.
- **Files.** `crates/udx-engine/src/engine.rs`, `crates/udx-engine/src/turn/{mod.rs,stream.rs,tools.rs,retry.rs,
  guards.rs,resume.rs,commands.rs}`, `crates/udx-engine/src/testing.rs`; tests
  `crates/udx-engine/tests/{engine_golden.rs,engine_tools.rs,engine_spend.rs,engine_commands.rs}`; golden event logs
  `crates/udx-engine/tests/golden/*.ndjson`.
- **Consumes.** The mock provider (T7), permissions (T13), context and trust (T14), egress (T15), the log (T16), spend
  (T17), the protocol (T18), the `Supervisor` trait and `udx_core::sandbox::testing::FakeSupervisor` (T2a).
  **Produces.** A 3.11 (`NativeEngine`, `EngineDeps` with `supervisor: Arc<dyn Supervisor>`, `LoopGuards`,
  `RetryPolicy`), the `Engine` trait's implementation (A 3.1.4) including every
  `SessionCommand` rule (widening `SetMode` refused unattended, `TurnOffBuiltinAsk` only from `Attended(App)`),
  `testing::TurnHarness`.
- **Tests** (scripted mock provider, fake tools and the fake supervisor from `testing`):
  - Golden event-log tests (design 10.1): a plain reply; one tool call; two parallel read calls; a write after a read;
    an ask answered allow, deny and timed out; a denied call not executed and leaving a permission record naming the
    rule; an "ask" in an unattended run becoming a deny with no card; a resumed session.
  - `p21_tool_result_key_reaches_mock_provider_redacted`.
  - `p6_resumed_session_gets_fresh_nonce` (results re-labelled with the new delimiters on resume).
  - `p59_mock_local_underreport_stops_turn`.
  - `p40_scripted_loop_stops_at_cap_with_event`; `p40_call_estimated_over_cap_never_sent` (the mock records no request).
  - `p47_cli_cannot_turn_off_builtin` (engine half: the command is refused for an `Attended(Terminal)` session);
    `p47_mode_widening_refused_headless` (engine half).
  - `p60_untrusted_workspace_starts_in_ask_despite_auto_config`.
  - `loop_guard_repeated_call_stops`, `loop_guard_invalid_streak_stops`, `loop_guard_max_calls_stops`.
  - `retry_honours_retry_after_and_never_repeats_tool`; `fallback_only_between_turns`.
  - `cancel_kills_group_and_stops_turn` (a fake tool registers a `RecordingKiller` through `ToolContext::processes`;
    on cancel `FakeSupervisor` records the kill and the killer counts it); `context_full_stops_with_reason`.
  - `reasoning_block_dropped_by_redaction_sends_nothing_on_resume`.
  - Hostile model output: `tool_args_not_json_counted_invalid`, `unknown_tool_name_counted_invalid`,
    `huge_tool_args_refused`, `tool_call_ids_duplicated_handled`.
- **Done when.** The common commands; the golden files reviewed line by line in the pull request.
- **Platform notes.** All three CI platforms; no real sandbox or process is used. Nothing here changes OS state.

### Task 20: File tools and `todo`

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** T5. **Runs beside.** T19, T21.
- **Files.** `crates/udx-tools/src/files/{read.rs,write.rs,edit.rs,glob.rs,grep.rs}`, `crates/udx-tools/src/todo.rs`;
  tests `crates/udx-tools/tests/files.rs`.
- **Consumes.** `FileGuard`, `UndoStore`, `TodoItem`, `TodoChange`. **Produces.** `read_file`, `write_file`,
  `edit_file`, `glob`, `grep`, `todo` (A 3.4).
- **Tests:** `p40_single_file_read_capped`; `read_outside_classified_outside_and_origin_marked`;
  `write_protected_classified_protected` (`.mcp.json`, `.git/config`, `.husky/pre-commit`, `CLAUDE.md` in a
  sub-folder); `edit_ambiguous_old_text_fails`; `edit_missing_old_text_fails`; `glob_skips_deny_listed_and_counts`;
  `grep_no_link_following`; `grep_binary_files_skipped`; `glob_pattern_hostile_inputs` (`**/../..`, absolute patterns,
  very long patterns); `todo_changes_produce_items`. (The engine-level ask for a protected write is T23's, since it
  needs the engine.)
- **Done when.** The common commands.
- **Platform notes.** All three CI platforms. Nothing here changes OS state.

### Task 21: The shell tool

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** T10, T13 (`parse_command`). **Runs beside.** T20, T22.
- **Files.** `crates/udx-tools/src/shell/{mod.rs,run.rs,path.rs}`; tests `crates/udx-tools/tests/shell.rs`.
- **Consumes.** `Sandbox`, `ProcessRegistry`, `KillHandle`, `ChildEnv`, `parse_command`; in tests,
  `udx_core::sandbox::testing::{FakeSupervisor, RecordingKiller}` (T2a) and, through the dev-dependency `udx-tools` to
  `udx-sandbox` with `testing` that A 2.2 allows, `udx_sandbox::testing::UnavailableSandbox` and the real backend
  (T10). **Produces.** `shell` (A 3.4):
  `Simple` runs `Direct` with `argv[0]` resolved against the child's `PATH` skipping relative and empty entries;
  `NeedsShell` runs `FixedShell`; each spawned group's `KillHandle` registered through `ToolContext::processes`;
  network only with a grant or an approved ask, refused with the reason when `NetworkGrants` is unavailable; output
  streamed and capped; `ExitInfo` (with `denied_writes` and `protected_created`) in the result.
- **Tests:** `p7_forced_unavailable_shell_absent_and_refuses` (with the testing sandbox everywhere, and in
  `sandbox-linux (restricted)` also with the real backend, which reports itself unavailable there);
  `p2_tar_checkpoint_leaves_no_marker` (a
  rule-allowed `tar czf o.tgz *` beside a planted `--checkpoint-action=exec=...` file: argv with no expansion, so no
  marker); `p2_direct_skips_relative_and_empty_path_entries` (a planted `./git` is never chosen);
  `simple_runs_direct_needs_shell_runs_fixed_shell`; `fixed_shell_has_no_startup_files` (a planted `.profile` and `ENV`
  do nothing); `p51_output_cut_with_marker`; `group_registered_with_process_registry` (the fake supervisor records the
  handle); `network_refused_when_grants_unavailable`; `cwd_must_be_inside_workspace`; `secret_never_in_argv`;
  `first_command_waits_for_login_path_then_falls_back`.
- **Done when.** The common commands; on macOS locally (only tests whose effects stay in the test root) and on CI, on
  Linux in `sandbox-linux (profile)` once T11 has merged, on Windows in the test job once T12 has merged (each later
  platform is added by the controller as a follow-up line in the ledger, not a code change, if the tests pass
  unchanged).
- **Platform notes.** Until T11 and T12 merge, Linux and Windows run only the `testing` sandbox cases. Nothing here
  changes OS state.

### Task 22: `web_fetch`, its network policy, and the image fetch path

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** T4, T6. **Runs beside.** T20, T21.
- **Files.** `crates/udx-tools/src/web_fetch/{mod.rs,policy.rs,resolve.rs,html.rs,decode.rs,bytes.rs}`,
  `crates/udx-tools/src/testing.rs`; tests `crates/udx-tools/tests/web_fetch.rs`.
- **Consumes.** `parse_host`, `classify`, `Redactor`, the `EgressGate` trait. **Produces.** `web_fetch`, `NetPolicy`,
  `FetchCaps`, `Resolver`, `SystemResolver` (the production resolver the engine wiring uses), `fetch_bytes` (A 3.5),
  `testing::{StubResolver, RecordingGate}`.
- **Tests** (loopback stand-in with a stub resolver and a recording gate; no real network):
  - `net_rebinding_connects_to_checked_answer` (a stub that answers public then private; the connection goes to the
    first answer or is refused, never to the second).
  - `net_redirect_to_loopback_refused`; `net_redirect_to_metadata_refused`; `net_too_many_redirects_refused`.
  - `p24_huge_body_stops_at_cap`; `p24_compression_bomb_stops_at_decompressed_cap`;
    `p24_attended_private_allow_not_applied_unattended`; `p24_private_allow_names_host_and_port_only`.
  - `p21_fetch_url_with_key_refused_before_connect`.
  - `p14_distinct_subdomains_each_ask_before_lookup` (the stub resolver records no lookup before the gate answers).
  - `p15_image_load_goes_through_address_policy` (`fetch_bytes` refuses a loopback and a metadata address and an
    unlisted content type).
  - `links_extracted_and_recorded_verbatim`; `html_scripts_and_styles_dropped`; `unsupported_encoding_refused`;
    `net_no_connection_at_start` (constructing the tool and the engine opens no socket).
- **Done when.** The common commands; mutation testing of `policy.rs` locally with no missed mutant.
- **Platform notes.** All three CI platforms. Nothing here changes OS state.

### Task 23: Engine integration: real tools, sandbox and supervisor, and the review-focus tests

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M.
- **Needs.** T10, T19, T20, T21, T22. **Runs beside.** T26, T27.
- **Files.** `crates/udx-engine/src/wiring.rs` (the body of T2b's stub; `udx-engine`'s `lib.rs` already declares the
  module), `crates/udx-engine/src/turn/{rerun.rs,images.rs,processes.rs}` and their lines in
  `crates/udx-engine/src/turn/mod.rs`; tests `crates/udx-engine/tests/{integration_files.rs,integration_shell.rs,
  integration_processes.rs,integration_images.rs,wiring.rs,review_focus.rs}`.
- **Consumes.** T19's engine, the real tools (T20 to T22), the macOS backend and supervisor (T10), `Router::from_config`
  and `ProviderSet` (T7), `SpendLedger::open` (T17), `TrustStore::open` (T14), `SystemClock` and `V7Ids` (T3),
  `SystemResolver` (T22). **Produces.** `udx_engine::wiring::engine_deps` exactly as A 3.11 gives it (the function the
  CLI, the app and the evaluation harness call); the flows A 3.11 assigns here: P8's wider re-run on an approved card
  (only for a backend-reported denial) and the card for `protected_created`; P7's unsandboxed-shell grant; P50's asks
  on exposure and on a revoked grant; P15's image load (`SessionCommand::LoadImage` through the egress gate and
  `fetch_bytes`, ending in `ImageLoaded` or `ImageRefused`).
- **Tests:**
  - `p8_file_tool_write_mcp_json_asks`; `p8_rerun_wider_only_after_card` (where the backend reports denials) and
    `p8_no_rerun_offer_without_reported_denial` (where it does not); `p8_protected_created_raises_card`.
  - `p7_unattended_cannot_enable_unsandboxed`.
  - `p50_exposure_asks_about_networked_processes`; `p50_revoke_asks_about_processes_holding_grant`.
  - `p15_exposed_image_load_raises_card_with_full_url` (engine side); `p15_image_load_goes_through_address_policy`
    (end to end).
  - `wiring_builds_deps_from_config_in_test_root` (with a `ProviderSet::from_providers` holding the mock provider:
    every field filled as A 3.11 says, the supervisor is the real one behind `Arc<dyn Supervisor>`, no socket opened;
    it calls `platform_sandbox`, so it runs where that platform's backend has merged, like the shell-dependent tests);
    `wiring_reports_setup_errors` (an unroutable role and an unreadable trust file each give `EngineError::Setup`
    naming the part).
  - The review-focus tests (Review Focus above), engine side: `review_focus_hostile_workspace_paths` (the guard, the
    sandbox profile's parameters and the session-log key and folder; the CLI's output is T24's),
    `review_focus_cancel_then_resume`, `review_focus_large_workspace` (a generated 100,000-file tree with
    `node_modules` and git-ignored `.env` files: a sandboxed command's start bounded, deny-listed files unreadable,
    glob and grep bounded), `review_focus_local_model_quirks` (engine half: the scripted mock provider emits the
    `StreamEvent` sequences T9's `review_focus_local_model_quirks_provider` asserts for the Ollama provider, which A
    3.2 and 3.3 fix: engine-made `udx-` ids for id-less parallel calls, a small `context_window`, text imitating the
    protocol's delimiters under `ToolCalling::Prompt`, and a prompt count below the lower bound; through the real
    engine and tools, the calls run under their engine ids with matching results, the turn stops with `ContextFull`
    rather than sending past the window, no call is parsed from the imitation (P6), and the turn stops with
    `PromptCut` (P59)), `review_focus_two_front_ends` (two processes on one workspace share the ledger and trust file
    correctly).
- **Done when.** The common commands; the shell-dependent tests green where the sandbox is full: macOS (locally only
  those whose effects stay in the test root; all on CI), Linux in `sandbox-linux (profile)`, Windows once T12 has
  merged (added by the controller as a ledger line if they pass unchanged).
- **Platform notes.** Tests that need a full sandbox skip with a notice where it is unavailable. Nothing here changes
  OS state on the development Mac.

### Task 24: CLI: `chat`, `run`, `trust`, `key`

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** T23. **Runs beside.** T25, T27, T28.
- **Files.** `crates/udx-cli/src/{main.rs,chat.rs,run.rs,trust.rs,key.rs,tty.rs,render.rs,wire.rs}`; tests
  `crates/udx-cli/tests/{chat.rs,run.rs,trust.rs,key.rs,tty.rs}`.
- **Consumes.** `udx_engine::wiring::engine_deps` (A 3.11; T23), `ProviderSet::from_config` and `from_providers` (T7),
  `TrustStore::open` (T14), `clean_access_entries` (T10's platform-neutral function, A 3.6, reached through T23's
  Needs; nothing here needs T12), `StreamRedactor`, `Tty` needs (`rustix` `termios`, `windows-sys` console).
  **Produces.** A 3.12 verbs `chat`, `run`, `trust`, `key`; `udx_cli::tty`. `main` calls `harden_process` first,
  dispatches the hidden `--udx-sandbox-probe`, and starts `resolve_login_path` on a background thread.
- **Tests** (through `assert_cmd` with a test root and the mock provider selected by the test-only flag
  `--test-provider-script FILE`, which only the `testing` feature compiles):
  - `p41_run_without_terminal_turns_asks_into_denies`.
  - `p49_prefilled_stdin_approves_nothing`; `p49_cursor_movement_in_output_printed_escaped`;
    `p49_single_key_is_not_an_answer` (a pseudo-terminal on Unix).
  - `p47_mode_widening_refused_headless` (`unidex run --mode auto` without a terminal exits 4; `unidex chat --mode auto`
    asks for a typed confirmation); `p47_cli_cannot_turn_off_builtin` (no verb or flag exists).
  - `chat_refuses_without_terminal_and_points_to_run`.
  - `run_exit_codes` (completed 0, stopped 3, refused 4, error 1); `run_json_is_ndjson_envelopes`;
    `run_json_key_split_across_deltas_redacted`.
  - `p13_trust_refused_headless`; `p48_trust_home_needs_long_confirmation`; `trust_revoke_cleans_access_entries`
    (macOS and Linux, through a pseudo-terminal: records planted in the test root's `sandbox_dir` for the revoked
    workspace and another one; after `unidex trust DIR --revoke` only DIR's records are gone and the printed report
    counts them, which shows the CLI calls `clean_access_entries` with `CleanScope::Workspace(DIR)`);
    `trust_revoke_without_records_reports_nothing` (three platforms, Windows included before and after T12: no
    records, nothing removed, success; on Windows as a unit test of the revoke path where the runner has no console,
    like the other console paths). The real removal on Windows is T12's `acl_revoke_removes_entries`.
  - `review_focus_hostile_workspace_paths_cli` (Review Focus 1, CLI side): `unidex run` and `unidex run --json` in
    workspaces whose paths hold spaces, quotes, parentheses, NFC and NFD non-ASCII names and, on Windows, more than 260
    characters, with tool output naming files whose names hold a right-to-left override and control characters: the
    plain output prints every path and name escaped (P49), `--json` keeps them exact after redaction, and the run
    completes; `unidex chat` does the same through a pseudo-terminal on Unix.
  - `key_never_from_argument` (no positional key accepted); `key_from_stdin_flag_stored_in_memory_store`.
  - `cli_makes_no_network_call_without_provider_config` (a recording resolver sees nothing).
  - `release_cli_refuses_test_provider_flag` (exit 2 without the feature).
- **Done when.** The common commands; `unidex chat` and `unidex run` against the mock provider demonstrated in the pull
  request with a transcript (no real key).
- **Platform notes.** Pseudo-terminal tests are Unix only; the Windows console path is unit-tested where the runner has
  no console. Nothing here changes OS state.

### Task 25: CLI: `config`, `doctor` and `sandbox clean` (acceptance rows C1 and C3)

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Small code unit, 0.7M.
- **Needs.** T3, T6, T7, T10, T12. **Runs beside.** T24.
- **Files.** `crates/udx-cli/src/{config.rs,doctor.rs,sandbox.rs}`; tests
  `crates/udx-cli/tests/{config.rs,doctor.rs,sandbox.rs}`.
- **Consumes.** `Config`, `apply`, `open_secret_store`, `platform_sandbox`, and T10's platform-neutral `access_entries`
  and `clean_access_entries` (A 3.6; T12 supplies only their Windows removal hooks).
  **Produces.** A 3.12 verbs `config`, `doctor` (rows: config with unknown keys, paths, secret store, sandbox with
  reason, remedy and network grants, AppArmor profile on Linux, Windows sandbox access entries, providers, workspace
  trust) and `sandbox clean`. (T24 owns `main.rs`; these subcommands are registered there by stubs T2b created, so this
  task edits only its own files.)
- **Tests:** `c1_list_shows_every_section_with_source`; `c1_dry_run_prints_diff_and_writes_nothing` (file hash
  unchanged); `c1_unset_restores_default`; `c3_doctor_warns_unknown_key_with_line`;
  `p22_host_change_from_tool_or_headless_refused`; `p22_host_change_clears_keychain_entry` (memory store);
  `p47_permission_widening_refused_headless` (allow rules and a widening `permissions.mode`);
  `doctor_reports_sandbox_reason_and_remedy` (testing sandbox); `doctor_reports_network_grants_unavailable`;
  `doctor_lists_acl_entries` (from planted records, every platform); `sandbox_clean_reports_nothing_on_unix`;
  `doctor_offline_by_default`; `doctor_json_stable`.
- **Done when.** The common commands.
- **Platform notes.** All three CI platforms; the real removal of access entries is T12's (Windows, opt-in). Nothing
  here changes OS state.

### Task 26: Markdown model, streaming document and the prepared-message cache

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Small code unit, 0.7M.
- **Needs.** T2b (it may start any time after). **Runs beside.** T19, T20, T23.
- **Files.** `crates/udx-ui-model/src/{markdown.rs,stream_doc.rs,cache.rs}`; tests
  `crates/udx-ui-model/tests/{markdown.rs,stream_doc.rs,cache.rs}`.
- **Consumes.** `DisplayText`, `MessageId`. **Produces.** A 3.13 `MessageBody`, `Block`, `Inline`, `LinkRef`,
  `ImageRef`, `StreamDoc`, `BodyDelta`, `PreparedCache`.
- **Tests:** `p15_remote_image_is_placeholder_with_full_url` (no fetch type exists in the crate);
  `raw_html_shown_as_text`; `stream_doc_equals_full_parse_after_every_token` (a 40 KB reply token by token, as decision
  0008 checked); `stream_doc_reparses_only_open_tail`; `cache_evicts_to_budget_lru`; `cache_never_evicts_pinned`;
  `cache_bytes_accounted`; `markdown_hostile_inputs` (deep nesting 10,000 levels, a 5 MB line, unbalanced fences,
  invalid UTF-8 replaced, a table with 10,000 columns): no panic and bounded time.
- **Done when.** The common commands.
- **Platform notes.** All three CI platforms (no window). Nothing here changes OS state.

### Task 27: Chat view-model: screen, thread, cards, notices and status

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** T13 (card data), T26. **Runs beside.** T23, T24, T25.
- **Files.** `crates/udx-ui-model/src/{screen.rs,thread.rs,card.rs,notice.rs,status.rs,actions.rs}`; tests
  `crates/udx-ui-model/tests/{screen.rs,cards.rs,notices.rs,p60.rs}`.
- **Consumes.** `EventEnvelope`, `SessionRecord`, `AskCard`, the policy types. **Produces.** A 3.13 `ChatScreen`,
  `ChatAction`, `Effect`, `ScreenChange`, `StatusVm` (with `tool_calling`), `WorkspaceVm`, `SessionSummary`,
  `ComposerVm`, `Banner`, `BannerKind`, `ToolRowVm`, `CardQueue`, `NoticeGate`, `SessionVm`, `MessageVm`.
- **Tests:** `p4_bidi_override_renders_escaped`; `p4_mixed_script_host_shown_ascii`;
  `p4_return_inside_guard_approves_nothing`; `p4_exposure_badge_after_mock_fetch`;
  `p4_composed_4kb_url_shown_whole_and_marked` (the ranges from an `EgressVerdict::Ask` mapped onto the escaped URL,
  nothing truncated); `p4_no_default_choice`; `p58_second_unanswered_notice_not_raised`;
  `p58_session_lists_only_own_asks`; `p47_builtin_off_indicator_persists_for_session`; `p60_new_workspace_defaults`;
  `p15_exposed_image_load_raises_card_with_full_url` (view-model side: the action yields
  `Effect::Engine(SessionCommand::LoadImage)` and the card shows the full URL); `p15_link_opens_only_through_opener`
  (the effect is an `OpenRequest`, untrusted origin, needing confirmation); `status_shows_tool_calling_mode`;
  `banner_for_network_grants_unavailable`; `protected_created_card_names_files`;
  `load_tail_then_rest_keeps_order_and_ids`; `streaming_delta_updates_one_row`; `cancel_effect_sent`.
- **Done when.** The common commands; the view-model tests run on three CI platforms (design 10.1).
- **Platform notes.** All three CI platforms (no window). Nothing here changes OS state.

### Task 28: The evaluation harness and the M1 task set

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. Code unit, 1.0M.
- **Needs.** T23. **Runs beside.** T24, T25, wave-4 tasks.
- **Files.** `crates/udx-evals/src/{task.rs,run.rs,check.rs,report.rs}`, `crates/udx-evals/src/bin/udx-evals.rs`,
  `crates/udx-evals/tasks/<id>/{task.toml,workspace/,mock-script.json}` for the 12 tasks of A 3.15; tests
  `crates/udx-evals/tests/suite_mock.rs`.
- **Consumes.** `udx_engine::wiring::engine_deps` (A 3.11; T23), with `ProviderSet::from_providers` and the mock
  provider for `ProviderSel::Mock` (T7). **Produces.** A 3.15's harness (`Task`, `Check`, `RunConfig`,
  `TaskResult`, `SuiteReport`, `EvalError`, `run_suite`), the task set, the egress-ask measurement output (asks per task
  and per P14 cap).
- **Tests:** `suite_runs_against_mock_and_all_pass` (each task's mock script solves it);
  `canary_tasks_fail_when_canary_leaks` (a deliberately leaking script makes the task fail);
  `report_records_pass_rate_invalid_calls_turns_cost_asks`; `workspace_copied_into_test_root`; `budget_cap_stops_task`;
  `skipped_tasks_listed_with_reason`.
- **Done when.** The common commands; `cargo run -p udx-evals -- run --suite m1 --provider mock` writes the report.
- **Platform notes.** The mock suite runs on three CI platforms inside the normal test job; the shell tasks run only
  where the sandbox is full (macOS, `sandbox-linux (profile)`, Windows once T12 merged) and are reported as skipped
  with the reason elsewhere. Nothing here changes OS state.

## Wave 4: the app, its tests and its packages

### Task 29: App foundations

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M.
- **Needs.** T23, T27. **Runs beside.** T28.
- **Files.** `crates/udx-app/src/{main.rs,app.rs,bridge.rs,theme.rs,fonts.rs,registry.rs,gpu.rs,workspace.rs}`; tests
  `crates/udx-app/tests/{gpu.rs,registry.rs,bridge.rs,fonts.rs}`.
- **Consumes.** `udx_engine::wiring::engine_deps` (A 3.11; T23), the view-model (T27). **Produces.** The binary's entry
  point (calls `harden_process` first; dispatches the hidden `--udx-sandbox-probe`; parses every M1 flag of A 3.13
  and calls the `automation` and `perf` hooks T2b declared, compiled only with their features, refusing them with exit
  2 otherwise; starts
  `resolve_login_path` on a background thread after the first frame), the Tokio-to-GPUI bridge (decision 0008's
  batch-draining pattern from the cookbook's `streaming_list`), `Theme` from the Electron app's tokens for this
  window, named fonts with fallbacks (A D17), `ElementRegistry`, `ElementRole` and `tagged`, `is_software_renderer`,
  the "Open folder" flow with P48's workspace check.
- **Tests:** `is_software_renderer_names` (WARP's name, vendor id 0x1414, llvmpipe, lavapipe, real adapters);
  `registry_records_bounds_in_prepaint` (`#[gpui::test]`); `bridge_drains_in_batches_no_polling` (no timer is
  registered while idle); `release_build_refuses_test_flags` (exit 2 for each flag); `open_folder_home_needs_confirmation`
  (view-model path); `fonts_resolved_through_text_system` (`all_font_names` and `get_font_for_id(resolve_font(..))`
  give the face chosen for the theme's family; the result is printed for the pull request, no logger installed).
- **Done when.** The common commands; with the maintainer's approval, the window opens on the development Mac for the
  builder's own scripted check (no screen capture of the person's desktop; the frame comes from the `ui` jobs once T32
  exists).
- **Platform notes.** Linux fonts: the face chosen on the runner is recorded in the pull request (decision 0013 left it
  unverified). Opening the window on the development Mac needs the maintainer's approval; nothing changes OS state.

### Task 30: The chat window

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M.
- **Needs.** T29. **Runs beside.** T33.
- **Files.** `crates/udx-app/src/chat/{window.rs,thread_list.rs,composer.rs,cards.rs,status.rs,sessions.rs,banner.rs}`,
  `crates/udx-app/src/render/{mod.rs,minimal.rs}`; tests `crates/udx-app/tests/chat_view.rs`.
- **Consumes.** `ChatScreen` and its types (T27), `PreparedCache` (T26). **Produces.** The M1 chat window of A 3.13 on
  GPUI's `list` (decision 0008): session list, message rows and tool rows, composer with text input (the cookbook's
  `text_input` pattern), ask cards with the guard interval, the status bar (provider and model, how the model calls
  tools, mode switch, sandbox state and network grants with reasons, the exposure badge, the indicator for built-in
  asks turned off, today's spend, trust state with a Trust button), banners, the prepared-message cache wired to
  `PreparedCache`. Message bodies go through the `MessageBodyRenderer` trait with a minimal renderer (plain paragraphs
  as styled text), replaced in T38.
- **Tests** (`#[gpui::test]` view tests): `card_appears_and_return_inside_guard_does_nothing`;
  `composer_send_dispatches_action`; `streaming_row_remeasured_only`;
  `cache_evicts_offscreen_rows_and_restores_on_return`; `status_shows_sandbox_reason_and_tool_calling`.
- **Done when.** The common commands; the `ui (macos)` frame artifact attached (after T32) or a `#[gpui::test]`
  rendering check until then.
- **Platform notes.** View tests on three CI platforms. Any run of the window on the development Mac needs the
  maintainer's approval; nothing changes OS state.

### Task 31: The automation socket

- **Role and budget.** Sonnet test author, Opus builder (the endpoint's security properties), Opus reviewer. Code
  unit, 1.0M.
- **Needs.** T30. **Runs beside.** T33, T34.
- **Files.** `crates/udx-app/src/automation/{mod.rs,endpoint_unix.rs,endpoint_windows.rs,verbs.rs,capture.rs}`; tests
  `crates/udx-app/tests/automation_endpoint.rs`.
- **Consumes.** `ChatScreen`, `ElementRegistry`, GPUI's `render_to_image` (with `automation`). **Produces.** A 3.13's
  automation socket: the endpoint, the verbs, the `UNIDEX_AUTOMATION_SOCKET_v1` marker, `--provider-script` and
  `--state-root` in test builds, `state` passed through the redactor.
- **Tests:** `endpoint_dir_created_0700_and_refuses_existing`; `peer_uid_mismatch_refused` (Unix: a second socket
  client faked through a test double of the peer check); `windows_pipe_dacl_current_user_only`,
  `windows_pipe_every_instance_first`, `windows_token_user_compared`; `unknown_verb_refused`; `line_over_64k_refused`;
  `idle_timeout_closes`; `rate_limit_applies`; `no_verb_takes_a_path_or_runs_a_program`; `state_redacted`.
- **Done when.** The common commands.
- **Platform notes.** Unix endpoint tests on macOS and Linux; pipe tests on Windows. The endpoint lives in a fresh
  folder in the test root; nothing changes OS state.

### Task 32: The end-to-end UI jobs and the macOS screenshot baselines

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. CI or packaging, 1.3M.
- **Needs.** T31. **Runs beside.** T33.
- **Files.** The driver in `crates/udx-app/tests/e2e/` (a Rust test that starts the test build and speaks the
  protocol); `crates/udx-app/tests/e2e/baselines/macos/*.png` (synthetic content only); `.github/workflows/ui.yml`.
- **Consumes.** T31's socket. **Produces.** The `ui` jobs and the baseline comparison (A 3.13).
- **`ui.yml`.** Jobs `ui (macos)`, `ui (linux-x11)` (Xvfb, Mesa lavapipe, DejaVu fonts, AT-SPI on, as decision 0013
  ran), `ui (linux-wayland)` (headless sway; action and tree checks are hard failures; frame steps skipped only on the
  exact `no_inapp_readback` answer), `ui (windows)`. Each builds `udx-app` with `automation`, runs the e2e tests
  against the scripted mock provider with `--state-root` in a fresh folder, and uploads the frames. It reuses the main
  CI cache key scheme (decision 0013 says a harness that reuses the build adds about half a minute).
- **Tests:** e2e: `e2e_send_prompt_shows_reply`, `e2e_tool_ask_card_allow_once`, `e2e_ui_card_guard_interval`,
  `e2e_trust_banner_and_trust`, `e2e_open_5000_thread_shows_end`, `e2e_frame_check_chat_window` (decision 0013's
  frame-check method at registry bounds: background, composer, a card's colours, text presence),
  `e2e_macos_baseline_within_tolerance` (`ui (macos)` only: per-pixel tolerance and mismatch budget; the difference
  image is uploaded), `e2e_idle_counter_zero_for_10s`.
- **Done when.** The common commands; all four `ui` jobs green on the task's pull request.
- **Platform notes.** CI only for the jobs; the e2e driver can run locally on the development Mac only with the
  maintainer's approval (it opens a window). Nothing changes OS state.
- **Repository settings.** The maintainer adds the four `ui` checks to the required checks.

### Task 33: Nightly packages of the real app and the release check

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. CI or packaging, 1.3M.
- **Needs.** T11 (the helper and the AppArmor profile file), T29. **Runs beside.** T30, T31, T32 (not T35).
- **Files.** `.github/workflows/nightly.yml`; `packaging/Packager.toml`; `packaging/linux/deb/{postinst,prerm}`;
  `packaging/macos/entitlements.toml`; `crates/udx-app/Cargo.toml` (`[package.metadata.deb]` only);
  `xtask/src/{main.rs,release_check.rs}`; tests `xtask/tests/release_check.rs`;
  `docs/decisions/0022-deb-with-cargo-deb.md`.
- **Consumes.** The app and CLI binaries, the helper, the AppArmor profile. **Produces.** A 3.15's release check;
  nightly packages of `unidex-rust-preview` (the app binary target, not an example; gate report, section g), `unidex`
  and, on Linux, `unidex-sandbox-helper`.
- **Changes to `nightly.yml`.** Build with
  `cargo build --release --locked -p udx-app --bin unidex-rust-preview -p udx-cli --bin unidex` (and the helper on
  Linux), never `--workspace`; the identity from gate question 5: display name "UniDeX Rust Preview", identifier
  `example.unidex.rust-preview`, publisher "UniDeX contributors"; packages stay unsigned and named so; the `.dmg` and
  NSIS installer from cargo-packager; the `.deb` from cargo-deb (D12, decided by the maintainer on 2026-11-17) with the
  AppArmor profile, its post-install load (`apparmor_parser -r`) and pre-removal unload, and `Depends` including
  `fonts-dejavu-core`; the tarball as now (no profile; on it the shell is on only where the probe finds the full
  sandbox, D1); `xtask release-check` on every artifact, each format on its own platform's runner; a size check
  failing the macOS job when the `.dmg` is 40,000,000 bytes or more; a step that fails if a staged program comes from
  an `examples` folder; release notes for the Windows installer saying to run `unidex sandbox clean --all` before
  uninstalling (A 3.6); the daily schedule stays (question 5).
- **Tests:** `p37_marker_absent_in_release`; `p37_marker_found_in_release_profile_build_with_feature` (the positive
  control is a release-profile build with each feature on, not a test build; if a `#[used]` marker does not survive,
  this task replaces it with a reference the binary cannot drop before merging); `p37_test_switches_refused_exit_2`
  (`--automation`, `--provider-script`, `--state-root`, `--perf-scenario`, and the CLI's `--test-provider-script`);
  `p37_entitlements_match_reviewed_list`; `release_check_reads_each_format` (small synthetic archives of each format,
  each tested only on the CI platform whose tool it needs: `hdiutil` on macOS, `dpkg-deb` on Linux, `7z` on Windows);
  on CI, the `.deb` installed on the Linux runner under the opt-in `package_install` and the probe reporting `Full`
  through the installed profile, then removed cleanly.
- **Done when.** The common commands; a nightly run started by hand on the task's branch green on three platforms with
  the release check passing and the artifacts attached.
- **Platform notes.** Packages are opened on no display. The format tests and the `.deb` install are CI only; nothing
  is attached, mounted or installed on the development Mac. The maintainer's hand check of the macOS package stays
  owed (gate report, section d).
- **Record.** 0022 (citing the maintainer's answer to D12, and stating that it supersedes decision 0014's sequencing
  sentence on the `.deb`; decision 0014 itself is not edited).

### Task 34: Perf instrumentation, the gate's scenarios and the idle floor

- **Role and budget.** Sonnet test author, Opus builder (GPUI profiler use), Opus reviewer. Code unit, 1.0M.
- **Needs.** T30. **Runs beside.** T31, T33.
- **Files.** `crates/udx-app/src/perf/{mod.rs,scenarios.rs,recorder.rs}`; tests `crates/udx-app/tests/perf.rs`.
- **Consumes.** The chat window, GPUI's profiler (`perf` feature). **Produces.** The `perf` feature's recorder and
  scenarios (A 3.13: `--perf-scenario scroll|open-thread|first-token|idle`, the recorder file named by
  `UNIDEX_PERF_OUT`, the `UNIDEX_PERF_MARKER_v1` marker); the idle floor measurement of an empty native window with the
  cookbook's `hello_window`, written up in the pull request for the gate (gate question 3's placeholders).
- **Verify first.** The GPUI `profiler` feature's present-time records at the pin (decision 0008 used them).
- **Tests:** `perf_marker_present_only_with_feature`; `recorder_writes_presented_frames_and_milestones` (a
  `#[gpui::test]` with a fake clock); `perf_build_refuses_os_folders_without_state_root`.
- **Done when.** The common commands; with the maintainer's approval, the scenarios run on the development Mac with the
  perf build (`cargo build --release --locked -p udx-app --bin unidex-rust-preview --features perf`, run with
  `--state-root`) and write their files (the numbers are T41's).
- **Platform notes.** Measurements on the development Mac need the maintainer's approval (they open a window); the
  tests run on three CI platforms. Nothing changes OS state.

### Task 35: CI perf checks, the size check, the fixture converter, the start-up launcher and the mutation scope

- **Role and budget.** Sonnet test author, Sonnet builder, Opus reviewer. CI or packaging, 1.3M.
- **Needs.** T33 (the nightly `.dmg` to calibrate against), T34 (the recorder's file format). **Runs beside.** T36,
  T37 (not T33).
- **Files.** `xtask/src/{main.rs,bench.rs,fixture.rs,perf_launch.rs,size.rs}`; `perf/ceilings.toml`;
  `.github/workflows/ci.yml` (jobs `perf-checks` and `perf-checks (size)`; the `mutants` job's scope and path filter);
  tests `xtask/tests/{fixture.rs,bench.rs,size.rs}`.
- **Consumes.** The session-log format, the recorder file. **Produces.** `xtask fixture convert-electron-thread <in>
  <out>` (the baseline's fixture, `docs/baseline/fixtures/long-thread.mjs`, converted to a v1 session log; checked
  against the fixture's documented counts: 10,002 lines, 5,000 messages); `xtask bench` (A D20); `xtask perf-launch`
  (A 3.13's start-up launcher); `xtask size` (the size proxy).
- **`perf-checks` job.** Linux: `cargo run --release -p xtask -- bench` (markdown parse of the 5,000-message fixture,
  session-log tail open and full replay, guard checks, permission evaluation) against `perf/ceilings.toml`.
- **`perf-checks (size)` job** (design 9: download size on every change). On the macOS arm64 runner for every pull
  request: build the release app and CLI binaries with no feature, `tar` them, compress at the `.dmg`'s level (zlib, as
  UDZO uses), and fail at 36,000,000 bytes (the 40,000,000-byte target less a 4,000,000-byte margin for the disk
  image's own overhead and the bundle's other files). The task calibrates the proxy against the nightly's `.dmg` and
  records the ratio in the pull request; the nightly's check of the real `.dmg` stays as the backstop.
- **Mutation scope.** The `mutants` job's scope grows to `udx-engine` (`permissions/`, `egress.rs`, `protocol.rs`,
  `spend.rs`) and `udx-tools` (`shell/parse.rs`, `web_fetch/policy.rs`); the task measures the job's time and splits it
  into a matrix if it passes the 30-minute limit (the maintainer then updates the required check names).
- **Tests:** `fixture_conversion_counts`; `bench_ceiling_exceeded_fails`; `size_over_threshold_fails`;
  `perf_launch_reports_median_and_range` (against a fake binary that writes a recorder file).
- **Done when.** The common commands; `perf-checks` and `perf-checks (size)` green.
- **Platform notes.** The jobs run on Linux and on the macOS runner; `perf-launch` runs on the development Mac only
  with the maintainer's approval (T41's measurements). Nothing changes OS state.
- **Repository settings.** The maintainer adds `perf-checks` and `perf-checks (size)` to the required checks.

## Wave 5: the scroll target and the message bodies

### Task 36: Measurement: what makes first-visit frames slow

- **Role and budget.** Opus builder, Sonnet reviewer. Trial, 1.3M. Throwaway measurement code stays on the task's
  branch; only the findings merge (into record 0020's draft, which T39 completes).
- **Needs.** T34, T35 (the fixture converter). **Runs beside.** T37.
- **Files.** Merged: `docs/decisions/0020-shaped-text-cache.md` (status "draft: measurement"), nothing else.
- **What it measures** (A D4; decision 0008 said the cause is likely, not proven): on the development Mac (100 Hz
  display, the reference machine), with the maintainer's approval, the perf build's scroll scenarios at 2,000 and
  10,000 px/s on the 5,000-message fixture from five start points, in three variants: as built; with every row's
  markdown prepared before the scroll; with every row's markdown prepared and its text shaped and kept before the
  scroll. Presented-frame intervals over 1.5 and 2 times the refresh interval for each, three runs each, the machine's
  load recorded.
- **Output.** Which share of slow frames each step removes; whether a persistent shaped-text cache is expected to meet
  the target; if not, what else (row layout, image placeholders, tool rows) the measurement implicates. T39 is
  re-scoped from it before T39 starts.
- **Done when.** The record's draft holds the tables and the runs' commands; the reviewer reproduces one run.
- **Platform notes.** Development Mac only, with the maintainer's approval (it opens a window); nothing changes OS
  state.

### Task 37: Measurement: the widget kit for message bodies (gate question 23)

- **Role and budget.** Opus builder, Sonnet reviewer. Trial, 1.3M. Its code (a `kit-renderer` implementation of
  `MessageBodyRenderer` in the kit's cached form) stays on its branch; only the record merges.
- **Needs.** T34, T35. **Runs beside.** T36.
- **Files.** Merged: `docs/decisions/0021-widget-kit-for-message-bodies.md` (status "proposed: awaiting the
  maintainer").
- **What it measures** (decision 0008's four, all on the development Mac with the perf build and the maintainer's
  approval): the cached form in the scroll, jump, stream and resize scenarios; selection and copy within a message and
  across messages in the virtualised list; link activation through the opener; and that a long message whose parse
  lands a frame later does not shift the rows around it. Also: the crates the kit adds and `cargo deny` on them (0BSD
  is allowed; decision 0007).
- **What T38 needs from the maintainer's answer** (A section 9): one word, "adopt" or "own renderer". The record states
  the build team's recommendation from the numbers; the maintainer's answer is written into it by the controller.
- **Done when.** The record holds the numbers and the recommendation; the maintainer has answered.
- **Platform notes.** Development Mac only, with the maintainer's approval; nothing changes OS state.
- **Record.** 0021.

### Task 38: Message bodies

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M.
- **Needs.** T26, T30, and the maintainer's answer to question 23 (T37). **Runs beside.** nothing compiling in
  `udx-app`.
- **Files.** `crates/udx-app/src/render/{plain.rs,blocks.rs,selection.rs,links.rs,images.rs}` (own renderer) or
  `crates/udx-app/src/render/kit.rs` brought from T37's branch (kit adopted; then `crates/udx-app/Cargo.toml` gains
  `gpui-base` behind `kit-renderer` as a sequenced dependency change approved by the controller), and
  `crates/udx-app/src/render/minimal.rs` removed; tests `crates/udx-app/tests/bodies.rs`.
- **Consumes.** `MessageBody`, `BodyDelta` (T26), the chat window (T30), `ImageLoaded` events. **Produces.** The
  message-body renderer behind `MessageBodyRenderer`: paragraphs, headings, lists, quotes, tables, code blocks (no
  highlighting in M1), links opened only through the opener after showing the full URL (P15, P54), remote images as
  placeholders with their full URL and a load action (`ChatAction::LoadRemoteImage`, which asks in exposed output and
  draws the image from `ImageLoaded`), selection and copy within and across messages.
- **Tests:** `p15_remote_image_makes_no_request` (the HTTP stand-in records none while the thread renders);
  `link_click_produces_open_request_with_full_url`; `image_load_action_produces_load_image_effect`;
  `selection_copy_within_message`; `selection_copy_across_messages`;
  `streaming_body_appends_without_shifting_rows_above`; `rtl_and_cjk_runs_render_without_panic`.
- **Done when.** The common commands; the `ui (macos)` frame artifact and baseline difference attached.
- **Platform notes.** View tests on three CI platforms. Nothing changes OS state.

### Task 39: The persistent shaped-text cache

- **Role and budget.** Opus core builder, Sonnet test author, Opus reviewer. Core code, 1.3M.
- **Needs.** T36, T38. **Runs beside.** T40.
- **Files.** `crates/udx-app/src/shaped_cache.rs`, `crates/udx-app/src/render/plain.rs` (use the cache); tests
  `crates/udx-app/tests/shaped_cache.rs`; `docs/decisions/0020-shaped-text-cache.md` (completed).
- **Consumes.** GPUI's `WindowTextSystem::shape_text` and `WrappedLine::paint` (public at the pin; A 3.13).
  **Produces.** `ShapedTextCache` and `ShapeKey` (A 3.13), its budget shared with the prepared-message cache's memory
  target.
- **Tests:** `cache_hit_skips_shaping` (a counting text-system test double); `width_change_invalidates`;
  `theme_change_invalidates`; `budget_evicts_lru`; `memory_after_full_scroll_under_budget` (perf build, development
  Mac with the maintainer's approval; recorded, not a CI test).
- **Done when.** The common commands; the scroll scenarios re-run on the development Mac with the cache, the numbers
  added to record 0020. If the kit was adopted, the task is re-scoped as D4 says (the kit's cached form carries the
  scroll target) before it starts.
- **Platform notes.** Unit tests on three CI platforms; measurements on the development Mac with the maintainer's
  approval. Nothing changes OS state.
- **Record.** 0020.

## Wave 6: records and the gate

### Task 40: Threat model, `SECURITY.md` and design updates

- **Role and budget.** Sonnet builder, Opus reviewer. Document, 0.6M.
- **Needs.** The security tasks merged (T3 to T28), T32 (P4's UI test), T33 (P37), T38 (P15). **Runs beside.**
  anything.
- **Files.** `docs/security/THREAT-MODEL.md`, `SECURITY.md`, `docs/design/specs/2026-11-16-unidex-rust-design.md`
  (sections 5.11 and 8 only, and only what the maintainer's recorded decisions and decision records 0015 to 0026
  imply), `docs/acceptance/CHECKLIST.md` (C1 and C3: milestone "M1", the "?" removed).
- **What it does.**
  - Threat model 9: each M1 control marked built or tested with the test's name; each of the 31 proposals moved into
    design 5.11 and its "Proposed" mark dropped.
  - Threat model section 7 gains the residuals M1 found, each with the test that pins it: P8-a (Linux and Windows
    cannot deny an absent protected path; reported after the command), P8-b (the re-run offer exists only where the
    backend reports denials), P10-a (a deny-listed file created during a command is unmasked until the next; the scan
    cap makes commands ask), P43-a (Linux: a pathname socket created inside the workspace from outside is reachable),
    networked commands' reach to host loopback services and the Landlock ABI 6 requirement for network grants (D28),
    P51-a (no memory limit on macOS), P51-b (no fill test on Windows), D2's sweep, D3's tool compatibility and the
    access entries on the user's folders, D15's case folding, P10's compatibility list from T28 and T12, P50's
    "cgroup" wording met by a PID namespace, P3's undo copy outside `.unidex/` before the vault and its deferred trash
    test.
  - Design 5.11 and section 8, per the maintainer's answers of 2026-11-17: the Linux row says the shell is off "where
    the full sandbox does not apply" (D1) instead of "the tarball"; the Windows mechanism is "an AppContainer with a job
    object" (D3) instead of "a restricted token"; the `.deb` is built with cargo-deb from M1 (D12), with decision 0014's
    sequencing noted as superseded on this point by record 0022 (decision 0014 itself is not edited). Decision records
    are not edited by this task.
- **Done when.** The privacy scan; a reviewer checks every row of A section 6 against the threat model's update. It
  must not overwrite the maintainer's decisions recorded in the design by the other session of 2026-11-17; it starts
  from the design as it stands on `main`.
- **Platform notes.** Documents only. Nothing changes OS state.

### Task 41: The M1 gate

- **Role and budget.** Opus drafter, Opus security reviewer (the gate's security review), Sonnet reviewer of the
  documents. Research document 1.2M plus the security review 0.5M: 1.7M.
- **Needs.** Everything above.
- **Files.** `docs/build/M1-GATE.md`; `docs/evals/m1-first-run.md` (results of the first evaluation run).
- **What it does.** Runs the gate's measurements (next section; the development Mac runs need the maintainer's
  approval), the first evaluation run (after the maintainer's approvals), the security review against section 6 of A,
  and writes the report in the M0 gate report's form, with the hands-on check owed to the maintainer listed, never
  assumed, and enforced human approval of governed changes (D19) listed as owed. Every frame figure carries the 120 Hz
  caveat, and every figure from the perf build says it differs from the shipped binary by three features (A 3.13).
- **Done when.** Every box of A 1.3 is ticked or reported as a miss with its evidence.
- **Platform notes.** Measurements on the development Mac with the maintainer's approval; CI for the rest. Nothing
  changes OS state.

## Total proposed budget

Per-task caps, in millions of tokens:

| Wave | Tasks and caps | Sum |
|---|---|---|
| 0 | T1a 1.0, T1b 1.0, T1c 0.9, T1d 1.3, T2a 1.3, T2b 1.0 | 6.5 |
| 1 | T3 1.0, T4 0.7, T5 1.3, T6 1.0, T7 1.0 | 5.0 |
| 2 | T8 1.0, T9 1.0, T10 1.3, T11 1.3, T12 1.3, T13 1.3, T14 1.3, T15 1.3, T16 1.0, T17 0.7 | 11.5 |
| 3 | T18 0.7, T19 1.3, T20 1.0, T21 1.0, T22 1.0, T23 1.3, T24 1.0, T25 0.7, T26 0.7, T27 1.0, T28 1.0 | 10.7 |
| 4 | T29 1.3, T30 1.3, T31 1.0, T32 1.3, T33 1.3, T34 1.0, T35 1.3 | 8.5 |
| 5 | T36 1.3, T37 1.3, T38 1.3, T39 1.3 | 5.2 |
| 6 | T40 0.6, T41 1.7 | 2.3 |
| | This architect pass (the gate report's line; the architect's, reviewer's and revision's own usage is recorded by the controller in the ledger, not here) | 0.5 |
| | **Total of caps** | **50.2** |

Arithmetic: 6.5 + 5.0 + 11.5 + 10.7 + 8.5 + 5.2 + 2.3 = 49.7M for 45 tasks, plus 0.5M, 50.2M. The wave sums: wave 0,
1.0 + 1.0 + 0.9 + 1.3 + 1.3 + 1.0 = 6.5; wave 1, 1.0 + 0.7 + 1.3 + 1.0 + 1.0 = 5.0; wave 2, six tasks at 1.3 (T10 to
T15, 7.8) plus three at 1.0 (T8, T9, T16, 3.0) plus T17 at 0.7 = 11.5; wave 3, two at 1.3 (T19, T23, 2.6) plus six at
1.0 (T20, T21, T22, T24, T27, T28, 6.0) plus three at 0.7 (T18, T25, T26, 2.1) = 10.7; wave 4, five at 1.3 (T29, T30,
T32, T33, T35, 6.5) plus two at 1.0 (T31, T34, 2.0) = 8.5; wave 5, four at 1.3 = 5.2; wave 6, 0.6 + 1.7 = 2.3.

Against the gate report's proposed M1 budget of 21.9M (check-in at 11M): the caps sum to 50.2M, 28.3M more. The gate
report's figure was a reading of design 6 before the maintainer's answers, and every task it listed has a counterpart
here at the same cap: the seven core tasks (the engine loop T19, streaming into GPUI T29, the three sandboxes T10 to
T12, permission cards T13, the chat window T30; 9.1M), the eight code units (the two providers T8 and T9, file tools
T20, shell and web tools T21, the session log T16, `unidex chat` T24, the mock provider and golden tests T7, the
evaluation harness T28; 8.0M), the two CI tasks (nightlies T33, the automation-socket harness T32;
2.6M), the threat model update T40 and the gate report (1.2M), the security review (0.5M) and the architect pass
(0.5M). The 28.3M on top is:

| Addition | Tasks | Caps |
|---|---|---|
| The first task, split in four | T1a, T1b, T1c, T1d | 4.2 |
| Security work from the 31 accepted proposals that design 6 does not list as tasks | T5 file guard, T14 context and trust, T15 egress, T6 secrets and child environment, T4 display, opener and addresses, T17 spend ledger, T22 `web_fetch` as its own task, T18 the prompt protocol, T23 engine integration and the review-focus tests | 9.3 |
| The contracts, in two | T2a, T2b | 2.3 |
| Core utilities, config, doctor and the view-model | T3, T25, T26, T27 | 3.4 |
| The scroll target and question 23 | T36, T37, T38, T39 | 5.2 |
| Performance instrumentation and CI perf and size checks | T34, T35 | 2.3 |
| The automation socket as its own task | T31 | 1.0 |
| The gate at a realistic cap (1.7M instead of 0.6M plus 0.5M) | T41 | 0.6 |
| **Total** | | **28.3** |

4.2 + 9.3 + 2.3 + 3.4 + 5.2 + 2.3 + 1.0 + 0.6 = 28.3M; 21.9 + 28.3 = 50.2M.

**Forecast.** Caps are not forecasts (section f). The forecast is a range, and merges are not counted as savings.

- The ratio M0 recorded: the cap rule applied to M0's Tasks 1 to 15 gives 18.2M (1.0 + 1.0 + 1.0 + 1.3 + 0.6 + 1.2 +
  0.6 + 1.2 + 1.3 + 1.3 + 1.3 + 2.6 + 1.3 + 1.3 + 1.2), and they recorded 11.7M (section f's rows, Task 16 excluded:
  10,962k + 749k), a ratio of 0.64. Recorded figures are lower bounds, so the true ratio is somewhat higher.
- **Low end, about 35.2M.** Assumption: M1's tasks use their caps as M0's did. The 48.0M of caps other than the gate
  and the architect line, times 0.64, is 30.7M; plus the gate at its cap, 1.7M; plus the architect line, 0.5M; plus the
  controller at 50k per task, 2.3M (45 tasks; M0 recorded none of the controller's own usage, gate report section f,
  so this is an assumption). 30.7 + 1.7 + 0.5 + 2.3 = 35.2M.
- **High end, about 51.8M.** Assumption: novel security code with test authors, and Linux and Windows work verified
  only in CI, uses more of its caps than M0's mix of documents and trials, here 0.85; the gate costs what M0's did;
  the controller uses 150k per task. 48.0 x 0.85 = 40.8M; plus the gate at M0's 3.7M; plus 0.5M; plus 6.8M (45 x
  150k). 40.8 + 3.7 + 0.5 + 6.8 = 51.8M.

**The milestone cap: the maintainer's choice.** The architect proposed a cap of 45M. That cap is below the high end of
this plan's own forecast: the high end is 51.8M, 6.8M over 45M, and the low end is 35.2M, 9.8M under it. So if spend
tracks the high end, M1's scope as planned does not fit under 45M, and something has to give at a check-in. The
maintainer chooses one of three options. In each, every task stops at its own cap, the check-ins are at 11M (the gate
report's point), 22M (the old cap) and 33M, and at each check-in the controller reports spend so far, the controller's
own usage included, and projects the milestone's total from it.

- **(a) A cap at the high end: 52M** (51.8M rounded up). The whole forecast range fits: 16.8M above the low end and
  0.2M above the high end. Nothing is planned to stop, but there is almost no room past the high end: one fix round of
  M0's size (140k to 614k) beyond the 0.85 assumption would use it up, and a check-in projecting past 52M goes to the
  maintainer as under (b).
- **(b) The 45M cap as proposed.** It holds 9.8M above the low end as contingency for tasks that stop at their caps
  and for fix rounds the 20 % margins do not pay for, and it is 6.8M short of the high end. Reaching the high end stops
  the milestone at a check-in for a decision: the first check-in whose projection passes 45M stops the milestone (no
  new task starts, running tasks finish) until the maintainer chooses to raise the cap (to (a)), to move scope out (as
  (c), or the T12 item below), or to end M1 with the work done. If spend tracks the high end, the projection passes
  45M as soon as a check-in measures that pace, so the stop comes at a check-in, not at the cap.
- **(c) A lower cap than (a), 46M, with named scope moved out of M1:** T36 and T39 (the shaped-text cache and its
  measurement), T37 (the widget-kit measurement), and the six merges below. That removes 5.4M of caps (2.6 + 1.3 +
  1.5) and nine tasks (two, one, and one for each merge), leaving 36 tasks and 42.6M of caps other than the gate and
  the architect line. Forecast after the move: low (48.0 - 5.4) x 0.64 + 1.7 + 0.5 + 36 x 0.05 = 31.3M; high (48.0 -
  5.4) x 0.85 + 3.7 + 0.5 + 36 x 0.15 = 45.8M. The 46M cap is that high end rounded up: 0.2M above it and 14.7M above
  the new low end. Its costs are those of each item below; it keeps every answer the maintainer gave on 2026-11-17
  except gate question 4's (the persistent shaped-text cache in M1), which it reopens.

No set of scope outside the security work brings the high end much under 45M: with T12 added to (c), the range is
30.4M to 44.6M (below), which a 45M cap would hold with 0.4M to spare, at T12's costs, which include reopening the
maintainer's answer to gate question 1 for Windows.

**How a saving is counted.** An item's saving in caps is the caps it removes. Its saving in forecast follows the
forecast's two assumptions: the caps removed times the ratio (0.64 at the low end, 0.85 at the high end), plus the
controller's allowance (50k at the low end, 150k at the high end) for each task that no longer runs. Moving every item
below saves 6.7M of caps and ten tasks: 4.8M of forecast at the low end and 7.2M at the high end (4.3M and 5.7M
without the controller term), leaving a range of 30.4M to 44.6M ((48.0 - 6.7) x 0.64 + 1.7 + 0.5 + 35 x 0.05 and
(48.0 - 6.7) x 0.85 + 3.7 + 0.5 + 35 x 0.15).

Scope the maintainer could move out of M1, with what each saves and what it costs:

- **The persistent shaped-text cache and its measurement (T36, T39):** saves 2.6M of caps and two tasks: 1.8M of
  forecast at the low end (2.6 x 0.64 + 2 x 0.05) and 2.5M at the high end (2.6 x 0.85 + 2 x 0.15). Costs: the scroll
  target is likely missed at the M1 gate (decision 0008 found the trial at the limit), and gate question 4's answer
  ("M1 builds a persistent shaped-text cache") is reopened.
- **The widget-kit measurement (T37):** saves 1.3M of caps and one task: 0.9M of forecast at the low end (1.3 x 0.64 +
  0.05) and 1.3M at the high end (1.3 x 0.85 + 0.15). Costs: question 23 is answered without numbers (our own renderer
  by default).
- **The Windows sandbox (T12):** saves 1.3M of caps and one task: 0.9M to 1.3M of forecast, as for T37. Costs, all of
  them:
  - It reopens two of the maintainer's answers of 2026-11-17: gate question 1 for Windows (the shell sandbox on all
    three platforms in M1; A 1.1 item 4), and D3, the acceptance of an AppContainer with a job object and its
    access-list footprint. The shell is off on Windows through M1 (fail closed, P7), which needs a design 8 exception
    the maintainer accepts; the Windows backend then reports `Unavailable` with a "not built in M1" reason instead of
    being built (a small change the controller assigns).
  - D3's footprint is not built: no access entries, records or AppContainer profiles; `unidex sandbox clean` and
    `unidex trust --revoke` have nothing to remove on Windows either; record 0016 is not written.
  - The doctor task (T25) changes: on Windows its sandbox row reports the shell off with that reason, its "Windows
    sandbox access entries" row reports none, and T12 leaves its Needs.
  - The Windows rows of the coverage tables change from tested to "shell off on Windows": in A section 6 the Windows
    tests of P1, P8, P10, P23, P43, P48, P50 and P51 are not written, and P7's
    `p7_forced_unavailable_shell_absent_and_refuses` pins the shell's absence instead; the Windows column of A 3.1.5's
    probe table and A 3.6's Windows items become M2 work; T40's residual list loses D3's tool compatibility and access
    entries and gains "no shell on Windows in M1".
  - The Windows shell runs of T21, T23 and T28 (and the evaluation suite's shell tasks there) are skipped with that
    reason.
- **Merges, which save review overhead only, not work:** T4 into T3, T26 into T27, T25 into T24, T17 into T16, T18
  into T19, T31 into T32. Each saves about 0.25M of caps (one reviewer allocation and one reserve, less the larger
  review of the merged diff) and one task, about 1.5M of caps and six tasks for all six: 1.3M of forecast at the low
  end (1.5 x 0.64 + 6 x 0.05) and 2.2M at the high end (1.5 x 0.85 + 6 x 0.15), at the cost of larger pull requests in
  crates already shared. They move no scope.
- **Nothing in the security set** can move without the maintainer reopening question 2.

## Order of work

The waves above, in order; within a wave, the controller runs at most two compiling tasks at once, prefers the tasks
other tasks wait on (T5 in wave 1; T10, T13, T14, T15 in wave 2; T19 and T23 in wave 3; T29 and T30 in wave 4), and
runs document and review work beside them. Question 23 is put to the maintainer as soon as T37's record is ready, so
the answer arrives before T38. The gate's prerequisites (a hosted key with a spend cap; a local model server with one
small model) are asked for when T28 merges, so they are in place by T41. Approval to open the app on the development
Mac is asked before T29, T34, T36, T37 and T39.

## What M1's gate will measure, and how

| Measure | How | Where |
|---|---|---|
| Start-up | The perf build (A 3.13: `--features perf`, run with `--state-root`); `cargo xtask perf-launch` takes a clock reading before spawn and reads the first presented frame showing the chat window's session list and composer from the recorder; one warm-up, ten launches, median and range; a first launch after a build reported separately | Development Mac, 100 Hz display |
| Idle | 30 s after the home screen, 60 s sampled with `top -l 61 -s 1` (IDLEW and CPU per process), five runs; the app's own idle counter must read zero; compared with the empty-window floor T34 measured | Development Mac |
| Idle memory | `footprint -p` summed over the app's processes at the end of each idle window; and after a full scroll of the 5,000-message thread | Development Mac |
| Scrolling | Perf build scenarios at 2,000 px/s (10 s) and 10,000 px/s (5 s) from five start points, two launches; presented-frame intervals from GPUI's profiler; counts over 1.5 and 2 times the refresh interval, the median, 95th and 99th percentiles and the longest | Development Mac |
| First token | Perf build with the scripted mock provider: first delta 200 ms and 2 s after the prompt; engine read time to the first presented frame containing it; five turns in a new thread and five in the 5,000-message thread | Development Mac |
| Opening the long thread | Perf build: the open action to the first frame showing the thread | Development Mac |
| Download size | Every pull request: `perf-checks (size)`, the compressed release binaries against 36,000,000 bytes; every nightly: the macOS arm64 `.dmg` against 40,000,000 bytes | CI |
| Core benchmarks, idle counter | `perf-checks`, the `ui` jobs' idle check | CI, every change |
| Security | Every test of A section 6 green on its platforms; a security review of the merged code against the 31 proposals; the threat model updated (T40) | CI and review |
| Acceptance | C1 and C3 tests green | CI |
| First evaluation run | `udx-evals` M1 suite: mock in CI; one hosted model and one small local model on the development Mac after the maintainer's approvals; pass rate, invalid tool calls, turns, cost, asks (and P14's ask rates, which set the cap defaults) | CI and development Mac |
| Packages | Nightly from the gate's commit green on three platforms with the release check | CI (nightly) |
| Hands-on | The maintainer's check on macOS | The maintainer |

Every frame figure in the report carries the caveat that a 120 Hz laptop leaves 8.3 ms per frame instead of 10, and
every perf-build figure says the build differs from the shipped binary by three features.

## Risks most likely to blow the budget (most likely first)

1. **The Linux and Windows sandboxes are CI-only work on unknown runner behaviour** (T11, T12). Whether the runner
   restricts user namespaces, which Landlock ABI its kernel offers (below 6, network grants are off and the networked
   escape matrix cannot run there), the AppArmor profile syntax, cgroup delegation, AppContainer access entries and
   their propagation time, a lowbox token's custom capability, and which tools run inside an AppContainer can each cost
   a CI round of ten or more minutes, and M0's trials show fix rounds of 140k to 614k tokens. Mitigation: each task's
   "verify first" step runs on CI before the build; the controller stops a task at its cap rather than funding another
   round.
2. **Fix rounds from security reviews across many security tasks** (T5, T10 to T23). M0's reviews caught real holes in
   every security-relevant task (section f, "What the reviews caught"); a fix round is not funded by the 20 % margin.
   Mitigation: test authors write the threat model's tests first, so most findings are caught before review.
3. **The scroll target** (T36, T38, T39). If shaping is not the main cause, the cache does not meet "at most 1 in
   1,000" and further work (row layout, tool rows) is unplanned; on a 120 Hz display the margin shrinks further.
   Mitigation: T36 measures before T39 is built, and T39 is re-scoped from its numbers.
4. **The gate's own cost** (T41). M0's gate cost 3.7M against 0.6M planned; 1.7M is capped here, and the forecast's
   high end assumes 3.7M.
5. **The controller's own usage**, never recorded in M0; the forecast assumes 50k to 150k per task, and the check-ins
   report it.
6. **GPUI churn in the app tasks** (T29 to T32, T34): pre-1.0 APIs, the test-support and profiler features, the
   Wayland hard check and three-platform UI jobs; mitigated by the cookbook patterns and the pinned source.
7. **Contract churn**: a signature in A that proves wrong in a later task costs a controller amendment and rework in
   every task built on it. T2a's Opus builder, the crate-map check and the review are where this is cheapest to catch.
8. **Governance by process** (D19): a governed change relies on the extra review and the controller's report, not on
   an enforced approval, until a second account exists.
9. **Hosted macOS runner availability** (M0's repeated capacity messages) delays merges, re-runs and the per-change size
   check.
10. **Question 23 answered late** delays T38 and T39; T30's minimal renderer keeps the rest moving.
11. **Usage-limit stops** (gate report, "Pacing"): large Opus agents run one at a time, re-reviews wait for fix rounds.
