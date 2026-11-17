# M0 Foundations, Trials and Decisions: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the repository, its gates and its records, and answer every unproven technical question with a
throwaway trial, so that milestone M1 can be planned from facts.

**Architecture:** A Cargo workspace with only the crates M0 needs (`udx-core`, `udx-cli`, `udx-test-support`,
`udx-cookbook`, `xtask`). Trials live in a separate `spikes/` workspace that is deleted at the gate; what survives a
trial is a written decision record, never its code.

**Tech Stack:** Rust stable (pinned), `clap`, `directories`, `thiserror`, `regex`, `serde`, `assert_cmd`, `tempfile`,
GitHub Actions, `cargo-deny`, `cargo-audit`, gitleaks, GPUI (source chosen in Task 9).

**Spec:** `docs/design/specs/2026-11-16-unidex-rust-design.md` (approved 2026-11-16). Executors read both.

**Scope:** The spec covers twelve milestones. Each gets its own plan, written when the previous gate passes, because
M0's answers (GPUI pins, PTY crate, browser embedding, measured cost per task) decide the details of M1 onward. This
plan is M0 only.

## Global Constraints

- Public repository, MIT licence. No GPL, AGPL or LGPL dependency; `cargo-deny` enforces it.
- No personal information in any file, commit message, pull request or agent report: no real email addresses,
  home-folder paths, host names or private network addresses. Fixtures use invented names and `example.com`.
- `cargo xtask privacy --staged` must pass before every commit once Task 2 lands. Before that, the orchestrator scans
  by hand.
- Commit messages end with the session's attribution trailer. The gate allows that address.
- Nothing is installed on the development machine, and nothing is pushed for the first time, without the maintainer's
  say-so. After the first push, the orchestrator pushes each task's pull request once its local gate passes.
- Rust stable, exact version pinned in `rust-toolchain.toml`. Edition 2024. Workspace version `0.0.1`.
- Anything started through cargo keeps its files under `target/test-root` (Task 3). A test must never read or write a
  real user's folders.
- Agents may read permissively licensed code as reference and must not copy from anything else. GPUI is permissive;
  most of the editor it comes from, including its terminal view, is not.
- One task is one branch in its own worktree and one pull request. At most two agents compile at the same time.
- Trial code is throwaway: no tests required, never merged into the main workspace, deleted in Task 16.
- Stop-and-report thresholds for M0 (plan call, provisional until measured): 500k tokens per attempt for a code or
  document task, 1M for a trial. Two failed Sonnet attempts escalate to Opus; two more stop and go to the maintainer.
- Every task appends one line to `docs/build/LEDGER.md` when it finishes: tokens used, attempts, outcome.

## Review Focus

Conditions the spec implies that are most likely to bite, each pinned to a test in the task that owns the code:

1. A personal term in a **file name** or a **commit message**, not in file contents. Expected: flagged. (Task 2)
2. A **binary or non-UTF-8 file** in a commit. Expected: scanned without a crash; text around invalid bytes is still
   checked. (Task 2)
3. **Look-alikes that must pass**: the attribution trailer, `example.com` addresses, `git@github.com` remotes,
   `127.0.0.1`, CI runner home paths. Expected: clean, or every commit fails and people bypass the gate. (Task 2)
4. A **fresh clone with no local term list**. Expected: generic rules still run, exit code reflects findings only,
   and `--require-local` fails loudly. (Task 2)
5. **Symlinked temp folders** (the default on macOS) and a **symlink that escapes** the test root. Expected: the first
   is accepted, the second refused. (Task 3)

---

## File map

```
Cargo.toml  rust-toolchain.toml  deny.toml  .gitleaks.toml  .gitignore  LICENSE  README.md  SECURITY.md
.cargo/config.toml                 alias xtask; UNIDEX_TEST_ROOT for every cargo-run process
privacy-allow.toml                 path-and-rule exceptions, each with a reason
.github/workflows/ci.yml           lint, test (3 platforms), deny, audit, privacy
.github/workflows/nightly.yml      trial packaging (Task 14)
crates/udx-core/src/{lib.rs,paths.rs}
crates/udx-cli/src/main.rs         crates/udx-cli/tests/version.rs
crates/udx-test-support/src/lib.rs
crates/udx-cookbook/               pinned GPUI examples that CI builds
xtask/src/{main.rs,privacy/{mod.rs,rules.rs,scan.rs,git.rs}}   xtask/tests/privacy.rs
docs/build/LEDGER.md               docs/build/M0-GATE.md
docs/decisions/NNNN-<slug>.md      one record per decision
docs/security/THREAT-MODEL.md
docs/acceptance/CHECKLIST.md
docs/baseline/electron-2.7.0.md
spikes/                            separate workspace, deleted in Task 16
```

---

### Task 1: Workspace skeleton and `unidex --version`

**Model:** Sonnet. **Needs first:** the maintainer's approval to install the Rust toolchain.

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `LICENSE`, `crates/udx-core/Cargo.toml`,
  `crates/udx-core/src/lib.rs`, `crates/udx-cli/Cargo.toml`, `crates/udx-cli/src/main.rs`
- Modify: `README.md`
- Test: `crates/udx-cli/tests/version.rs`

**Interfaces:**
- Produces: `udx_core::VERSION: &str` (the workspace version); binary `unidex` with `--version` and `--json`.

- [ ] **Step 1: Install the toolchain** (after approval). Install `rustup` with the minimal profile, then the current
  stable release. Write that exact version (for example `channel = "1.xx.y"`, never `"stable"`) into
  `rust-toolchain.toml` with components `rustfmt` and `clippy`.
  Run: `cargo --version`. Expected: prints the pinned version.
- [ ] **Step 2: Write the failing test** in `crates/udx-cli/tests/version.rs`:

```rust
use assert_cmd::Command;

#[test]
fn prints_plain_version() {
    Command::cargo_bin("unidex").unwrap().arg("--version")
        .assert().success().stdout("unidex 0.0.1\n");
}

#[test]
fn prints_json_version() {
    Command::cargo_bin("unidex").unwrap().args(["--version", "--json"])
        .assert().success().stdout("{\"name\":\"unidex\",\"version\":\"0.0.1\"}\n");
}

#[test]
fn unknown_flag_is_a_usage_error() {
    Command::cargo_bin("unidex").unwrap().arg("--nope").assert().code(2);
}
```

- [ ] **Step 3: Run it.** `cargo test -p udx-cli`. Expected: FAIL, the binary does not exist.
- [ ] **Step 4: Implement.** Workspace `Cargo.toml` with `[workspace.package]` (version `0.0.1`, edition `2024`,
  license `MIT`) and members `crates/*` and `xtask`. `udx_core::VERSION = env!("CARGO_PKG_VERSION")`. `unidex` parses
  with `clap`; `--version` handling is our own so `--json` can change its output.
- [ ] **Step 5: Run it.** `cargo test -p udx-cli`. Expected: 3 passed.
- [ ] **Step 6: Licence and readme.** `LICENSE` is MIT, copyright "UniDeX contributors". `README.md` says what the
  project is, that it is pre-release, and links the spec. `.gitignore` covers `target/`, `spikes/target/` and
  `privacy-terms.local.txt`.
- [ ] **Step 7: Commit.** `chore: workspace skeleton and unidex --version`

### Task 2: The privacy gate

**Model:** Sonnet test author, a different Sonnet builder, Opus reviewer.

**Files:**
- Create: `xtask/Cargo.toml`, `xtask/src/main.rs`, `xtask/src/privacy/{mod.rs,rules.rs,scan.rs,git.rs}`,
  `.cargo/config.toml`, `privacy-allow.toml`, `.gitleaks.toml`
- Test: `xtask/tests/privacy.rs`

**Interfaces:**
- Produces, in `xtask::privacy`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleId { Email, HomePath, PrivateAddress, PrivateHost, SessionLink, LocalTerm, LocalFileTracked }
pub struct Finding { pub path: String, pub line: usize, pub rule: RuleId, pub excerpt: String }
pub struct Terms { /* generic rules plus optional local terms */ }
impl Terms {
    pub fn generic() -> Terms;
    pub fn with_local(self, list_text: &str) -> Terms;   // one term per line; blank lines and # comments skipped
    pub fn has_local(&self) -> bool;
}
pub fn scan_text(label: &str, text: &str, terms: &Terms) -> Vec<Finding>;
pub fn scan_bytes(label: &str, bytes: &[u8], terms: &Terms) -> Vec<Finding>;  // lossy decode, never panics
pub fn scan_path_name(path: &str, terms: &Terms) -> Vec<Finding>;
```

- Command: `cargo xtask privacy [--staged | --all | --range <A..B>] [--stdin <label>] [--require-local] [--json]`.
  Exit 0 clean, 1 findings, 2 usage error or `--require-local` with no local list.
- `--range` scans every file each commit in the range touches and each commit's message.
- Command: `cargo xtask hooks install` writes the `pre-commit` and `commit-msg` hooks described in Step 6.
- Local list: `privacy-terms.local.txt` at the repo root, matched case-insensitively. If git tracks it or does not
  ignore it, that is a `LocalFileTracked` finding.
- `privacy-allow.toml`: entries of `path` (glob), `rule`, `reason`. An entry exempts that rule on that path only.
  `LocalTerm` can never be exempted.

**Rules (exact):**

| Rule | Flags | Allows |
|---|---|---|
| `Email` | `[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}` | ends `@users.noreply.github.com`; equals `noreply@anthropic.com`; domain `example.com`, `.org` or `.net`; equals `git@github.com` |
| `HomePath` | `/Users/<name>/`, `/home/<name>/`, `<drive>:\Users\<name>\` | name is `runner`, `you`, `me`, `user`, `username`, `name` or `example` |
| `PrivateAddress` | IPv4 in `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `100.64.0.0/10`, `169.254.0.0/16` | everything else, including `127.0.0.1` |
| `PrivateHost` | `<label>.ts.net` | |
| `SessionLink` | `claude.ai/code/session_…`, `claude.ai/artifact/…`, `claude.ai/code/artifact/…` | |

- [ ] **Step 1: Write the failing tests** in `xtask/tests/privacy.rs`. All names and addresses are invented.

```rust
use xtask::privacy::{scan_bytes, scan_path_name, scan_text, RuleId, Terms};

fn rules(text: &str) -> Vec<RuleId> {
    scan_text("f.md", text, &Terms::generic()).into_iter().map(|f| f.rule).collect()
}

#[test] fn flags_a_personal_email() {
    let f = scan_text("f.md", "first line\nwrite to jane.roe@corp.io", &Terms::generic());
    assert_eq!((f.len(), f[0].rule, f[0].line), (1, RuleId::Email, 2));
}
#[test] fn allows_look_alike_addresses() {
    for ok in ["Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>",
               "12345+someone@users.noreply.github.com", "dev@example.com",
               "git@github.com:org/repo.git"] {
        assert!(rules(ok).is_empty(), "{ok}");
    }
}
#[test] fn flags_home_paths_on_three_platforms() {
    for bad in ["/Users/jroe/code/x", "/home/jroe/.config/x", r"C:\Users\jroe\AppData\x"] {
        assert_eq!(rules(bad), vec![RuleId::HomePath], "{bad}");
    }
}
#[test] fn allows_runner_and_placeholder_homes() {
    for ok in ["/home/runner/work/x", "/Users/runner/x", "/Users/you/vault/x"] {
        assert!(rules(ok).is_empty(), "{ok}");
    }
}
#[test] fn flags_private_addresses_but_not_loopback_or_public() {
    for bad in ["10.1.2.3", "172.20.0.5", "192.168.1.10", "100.64.0.7", "169.254.0.9"] {
        assert_eq!(rules(bad), vec![RuleId::PrivateAddress], "{bad}");
    }
    for ok in ["127.0.0.1", "8.8.8.8", "172.32.0.1", "100.128.0.1"] {
        assert!(rules(ok).is_empty(), "{ok}");
    }
}
#[test] fn flags_private_hosts_and_session_links() {
    assert_eq!(rules("see box.tail0a1b2c.ts.net"), vec![RuleId::PrivateHost]);
    assert_eq!(rules("https://claude.ai/code/session_AbC123"), vec![RuleId::SessionLink]);
}
#[test] fn local_terms_match_any_case_and_skip_comments() {
    let t = Terms::generic().with_local("# a comment\n\nAcme Corp\n");
    assert!(t.has_local());
    assert_eq!(scan_text("f.md", "we met at ACME corp", &t)[0].rule, RuleId::LocalTerm);
    assert!(scan_text("f.md", "a comment", &t).is_empty());
}
#[test] fn a_term_in_a_file_name_is_flagged() {
    let t = Terms::generic().with_local("Acme Corp\nacme-corp\n");
    assert_eq!(scan_path_name("docs/acme-corp-notes.md", &t)[0].rule, RuleId::LocalTerm);
}
#[test] fn invalid_utf8_is_scanned_without_a_panic() {
    let mut bytes = vec![0xff, 0xfe, 0x00];
    bytes.extend_from_slice(b"/Users/jroe/secret");
    assert_eq!(scan_bytes("blob.bin", &bytes, &Terms::generic())[0].rule, RuleId::HomePath);
}
```

  Add command-level tests with `assert_cmd`, each in a temporary git repository:

  | Test | Setup | Expected |
  |---|---|---|
  | `clean_repo_exits_zero` | one harmless file, `--all` | exit 0 |
  | `finding_exits_one` | a file holding `/Users/jroe/x`, `--all` | exit 1, output names the file and `HomePath` |
  | `no_local_list_is_fine_unless_required` | no local list | `--all` exits 0; `--all --require-local` exits 2 |
  | `a_tracked_local_list_is_a_finding` | `privacy-terms.local.txt` committed | exit 1, `LocalFileTracked` |
  | `stdin_mode_scans_a_commit_message` | pipe `fix for jane.roe@corp.io`, `--stdin msg` | exit 1 |
  | `range_mode_scans_commit_messages` | second commit's message holds `10.1.2.3`, `--range HEAD~1..HEAD` | exit 1 |
  | `allow_file_exempts_one_rule_on_one_path_only` | allow `HomePath` on `a.md`; both `a.md` and `b.md` hold a home path and an invented email | exit 1 with exactly three findings |
- [ ] **Step 2: Run them.** `cargo test -p xtask`. Expected: FAIL, the module does not exist.
- [ ] **Step 3: Implement** the interfaces above. `.cargo/config.toml` gets `[alias] xtask = "run --package xtask --"`.
  `privacy-allow.toml` starts with two entries: `xtask/tests/**` and this plan file, for the four generic text rules,
  reason "invented test vectors". `.gitleaks.toml` extends the default rules.
- [ ] **Step 4: Run them.** `cargo test -p xtask`. Expected: all pass.
- [ ] **Step 5: Run the gate on the repository.** `cargo xtask privacy --all`. Expected: exit 0.
- [ ] **Step 6: Install the hook.** `cargo xtask hooks install` writes a `pre-commit` hook that runs
  `cargo xtask privacy --staged` and a `commit-msg` hook that pipes the message through `--stdin`. Verify by trying to
  commit a file holding an invented home path. Expected: the commit is refused. Remove the file.
- [ ] **Step 7: Commit.** `feat: privacy gate with hooks`

### Task 3: Safe folders for anything cargo starts

**Model:** Sonnet test author, a different Sonnet builder, Opus reviewer.

**Files:**
- Create: `crates/udx-core/src/paths.rs`, `crates/udx-test-support/Cargo.toml`,
  `crates/udx-test-support/src/lib.rs`
- Modify: `crates/udx-core/src/lib.rs`, `.cargo/config.toml`
- Test: `crates/udx-core/tests/paths.rs`

**Interfaces:**
- Produces, in `udx_core::paths`:

```rust
pub trait Env { fn var(&self, key: &str) -> Option<String>; }
pub struct ProcessEnv;                     // reads the real environment
pub struct AppPaths { pub config_dir: PathBuf, pub data_dir: PathBuf, pub cache_dir: PathBuf }
#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("no home folder found")] NoHome,
    #[error("{dir} is outside the test root {root}")] OutsideTestRoot { dir: PathBuf, root: PathBuf },
}
impl AppPaths { pub fn resolve(env: &dyn Env) -> Result<AppPaths, PathError>; }
```

- Produces, in `udx_test_support`: `TestEnv::new() -> TestEnv` (a fresh folder inside the process's test root, with
  the variables below set), `TestEnv::root(&self) -> &Path`, and `impl Env for TestEnv`.

**Behaviour:**
- `UNIDEX_CONFIG_DIR`, `UNIDEX_DATA_DIR`, `UNIDEX_CACHE_DIR` override each folder.
- Otherwise, with `UNIDEX_TEST_ROOT` set: `<root>/config`, `<root>/data`, `<root>/cache`.
- Otherwise: the OS folders from `directories::ProjectDirs::from("", "", "unidex-preview")`.
- With `UNIDEX_TEST_ROOT` set, every folder must lie inside the root after symlinks are resolved on both sides (resolve
  the longest existing ancestor of a folder that does not exist yet). If not: `OutsideTestRoot`.
- `.cargo/config.toml` sets `UNIDEX_TEST_ROOT` to `target/test-root` (relative to the workspace) for every process
  cargo starts. Only an installed binary ever uses the OS folders.

- [ ] **Step 1: Write the failing tests** in `crates/udx-core/tests/paths.rs`. `MapEnv` is a local `HashMap` wrapper
  implementing `Env`, built with `MapEnv::of(pairs: impl IntoIterator<Item = (&str, &Path)>)`; `tmp()` returns a
  `tempfile::TempDir`; `real(p)` is `std::fs::canonicalize`.

```rust
#[test] fn defaults_live_inside_the_test_root() {
    let root = tmp();
    let p = AppPaths::resolve(&MapEnv::of([("UNIDEX_TEST_ROOT", root.path())])).unwrap();
    assert_eq!(p.config_dir, real(root.path()).join("config"));
    assert_eq!(p.data_dir, real(root.path()).join("data"));
    assert_eq!(p.cache_dir, real(root.path()).join("cache"));
}
#[test] fn an_override_inside_the_root_is_accepted() {
    let root = tmp();
    let inside = root.path().join("elsewhere");
    let p = AppPaths::resolve(&MapEnv::of([("UNIDEX_TEST_ROOT", root.path()), ("UNIDEX_DATA_DIR", inside.as_path())])).unwrap();
    assert_eq!(p.data_dir, real(root.path()).join("elsewhere"));
}
#[test] fn an_override_outside_the_root_is_refused() {
    let (root, other) = (tmp(), tmp());
    let e = AppPaths::resolve(&MapEnv::of([("UNIDEX_TEST_ROOT", root.path()), ("UNIDEX_DATA_DIR", other.path())]));
    assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })));
}
#[cfg(unix)] #[test] fn a_symlink_that_escapes_the_root_is_refused() {
    let (root, other) = (tmp(), tmp());
    std::os::unix::fs::symlink(other.path(), root.path().join("link")).unwrap();
    let via = root.path().join("link").join("data");
    let e = AppPaths::resolve(&MapEnv::of([("UNIDEX_TEST_ROOT", root.path()), ("UNIDEX_DATA_DIR", via.as_path())]));
    assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })));
}
#[cfg(unix)] #[test] fn a_root_reached_through_a_symlink_is_accepted() {
    let (holder, target) = (tmp(), tmp());
    let alias = holder.path().join("alias");
    std::os::unix::fs::symlink(target.path(), &alias).unwrap();
    assert!(AppPaths::resolve(&MapEnv::of([("UNIDEX_TEST_ROOT", alias.as_path())])).is_ok());
}
#[test] fn without_a_test_root_the_os_folders_are_used() {
    let p = AppPaths::resolve(&MapEnv::of([] as [(&str, &std::path::Path); 0])).unwrap();
    assert!(p.config_dir.to_string_lossy().contains("unidex-preview"));
}
#[test] fn cargo_gives_every_test_process_a_test_root() {
    assert!(std::env::var("UNIDEX_TEST_ROOT").is_ok());
    assert!(AppPaths::resolve(&ProcessEnv).is_ok());
}
#[test] fn test_env_folders_sit_inside_the_process_root() {
    let env = udx_test_support::TestEnv::new();
    let p = AppPaths::resolve(&env).unwrap();
    assert!(p.data_dir.starts_with(real(env.root())));
}
```

- [ ] **Step 2: Run them.** `cargo test -p udx-core`. Expected: FAIL, `paths` does not exist.
- [ ] **Step 3: Implement** `paths.rs`, `udx-test-support`, and the `[env]` entry
  `UNIDEX_TEST_ROOT = { value = "target/test-root", relative = true }`. `resolve` reads no folder contents and creates
  nothing.
- [ ] **Step 4: Run them.** `cargo test --workspace`. Expected: all pass.
- [ ] **Step 5: Commit.** `feat: app folders with a test-root guard`

### Task 4: CI on three platforms

**Model:** Sonnet. **Needs first:** the maintainer's go-ahead for the first push.

**Files:**
- Create: `.github/workflows/ci.yml`, `deny.toml`

**Interfaces:**
- Produces: required checks `lint`, `test (macos)`, `test (linux)`, `test (windows)`, `deny`, `audit`, `mutants`,
  `privacy`.

- [ ] **Step 1: Write `deny.toml`.** Allowed licences: MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause,
  BSD-3-Clause, ISC, Unicode-3.0, Zlib, CC0-1.0, MPL-2.0. Everything else is denied. Advisories: deny. Sources: only
  crates.io, plus any git source a decision record names.
- [ ] **Step 2: Write `ci.yml`.** Jobs:
  - `lint` (Linux): `cargo fmt --all --check`, then `cargo clippy --workspace --all-targets --locked -- -D warnings`.
  - `test`: matrix over the newest Apple silicon macOS, Ubuntu LTS and Windows Server runner labels available, written
    as fixed labels and never `-latest`; `cargo test --workspace --locked`.
  - `deny`: `cargo deny check`. `audit`: `cargo audit`.
  - `mutants`: `cargo mutants -p udx-core -p xtask` on pull requests that touch those crates. A surviving mutant
    fails the job unless it is listed, with a reason, in `.cargo/mutants.toml`.
  - `privacy`: `cargo xtask privacy --all`, then `cargo xtask privacy --range <base>..<head>` on pull requests, then
    gitleaks over the same range.
  Every action is pinned to a full commit SHA with its version in a trailing comment. Permissions: `contents: read`.
- [ ] **Step 3: Open the pull request and watch it.** `gh pr checks --watch`. Expected: eight checks, all green.
- [ ] **Step 4: Prove the gate bites.** On a scratch branch, push one commit adding a file that holds an invented home
  path. Expected: `privacy` fails and names the file. Close the pull request and delete the branch.
- [ ] **Step 5: Commit and merge.** `ci: three-platform checks with licence, advisory and privacy gates`

### Task 5: The ledger and the decision log

**Model:** Sonnet.

**Files:**
- Create: `docs/build/LEDGER.md`, `docs/decisions/0000-template.md`, and records `0001` to `0005`

- [ ] **Step 1: Write `LEDGER.md`.** A header that explains it, then one table: `date | milestone | task | role |
  model | attempts | tokens | outcome | pull request`. Any session can restart cold by reading this file, the decision
  log and the current plan.
- [ ] **Step 2: Write the template.** Sections: Context, Options considered, Decision, Who decided, Consequences,
  Revisit when.
- [ ] **Step 3: Record the five decisions already made**, from spec section 4: `0001-own-engine-acp-addons`,
  `0002-native-ui-gpui`, `0003-own-vault-with-import`, `0004-embedded-chromium`, `0005-platform-tiers`.
- [ ] **Step 4: Check.** `cargo xtask privacy --all`. Expected: exit 0. Each record names who decided as "the
  maintainer" or "design call".
- [ ] **Step 5: Commit.** `docs: task ledger and decision log`

### Task 6: Threat model

**Model:** Opus. Reference: spec section 5.11 and the Electron app's `SECURITY.md`.

**Files:**
- Create: `docs/security/THREAT-MODEL.md`, `SECURITY.md`

- [ ] **Step 1: Write the threat model.** One section per boundary in spec 5.11 (ten rows). Each section states: what
  is protected, who attacks and how (at least three concrete examples), the control, the milestone that delivers it,
  and the test that will prove it. Add a section listing every lesson in the Electron app's `SECURITY.md` and whether
  it still applies, with the reason.
- [ ] **Step 2: Write `SECURITY.md`.** How to report a problem (GitHub private advisories, no email address), what is
  in scope, and a link to the threat model.
- [ ] **Step 3: Review.** A second Opus agent, given only the two files and spec 5.11, lists attacks the model
  misses. Each is added or answered in writing.
- [ ] **Step 4: Check and commit.** `cargo xtask privacy --all` exits 0. `docs: threat model and security policy`

### Task 7: Acceptance checklist and the parity table

**Model:** Sonnet, reviewed by Opus. Source: the Electron app's `docs/app-smoke.md` (28 sections),
`docs/acceptance.md` and `app/tests/e2e/COVERAGE.md` (346 items).

**Files:**
- Create: `docs/acceptance/CHECKLIST.md`
- Modify: `docs/design/specs/2026-11-16-unidex-rust-design.md` (section 7 only)

- [ ] **Step 1: Port the checklist.** One row per source item: `id | source section | behaviour | fate | milestone |
  how verified`. Fate is `carry`, `change` or `drop`; a `change` or `drop` gives its reason in one line. How verified
  is `automated`, `manual` or `not applicable`.
- [ ] **Step 2: Check the count.** The file's totals line must account for all 346 source items. Items about host
  plugins, the status line and the setup wizard's downloads are `drop`, per the spec's non-goals.
- [ ] **Step 3: Rewrite spec section 7** as one row per source section, with counts of carry, change and drop.
- [ ] **Step 4: Review.** Opus samples 40 rows against the source and reports any wrong fate.
- [ ] **Step 5: Check and commit.** Privacy gate exits 0. `docs: acceptance checklist ported from the Electron app`

### Task 8: Add-on vendor terms

**Model:** Opus, with web access. Official vendor pages only.

**Files:**
- Create: `docs/decisions/0006-addon-vendor-terms.md`

- [ ] **Step 1: For Claude Code and for Codex**, record from the vendor's own current documentation: whether a
  third-party app may drive the user's own installed, signed-in CLI through an ACP adapter; which billing pool that
  use draws on; who maintains the adapter and its licence. Give each source's address and the date read. Quote at most
  one short sentence per source.
- [ ] **Step 2: Conclude** for each agent: ship as an add-on, ship with a stated limit, or leave off the list.
- [ ] **Step 3: Check and commit.** The maintainer reads the record before M5 is planned.
  `docs: decision 0006, add-on vendor terms`

### Task 9: GPUI source, pins and the cookbook

**Model:** Opus.

**Files:**
- Create: `docs/decisions/0007-gpui-source-and-pins.md`, `crates/udx-cookbook/Cargo.toml`,
  `crates/udx-cookbook/examples/hello_window.rs`, `crates/udx-cookbook/README.md`
- Modify: `.github/workflows/ci.yml` (build the cookbook examples on all three platforms)

**Interfaces:**
- Produces: the exact GPUI and widget-kit dependency lines every later crate copies, and the upgrade procedure.

- [ ] **Step 1: Compare the candidates**: the `gpui` crate on crates.io, the `gpui-kit` umbrella crate with its pinned
  GPUI snapshot, and a git dependency on a fixed upstream commit. For each: licence, date of the pinned commit, whether
  it builds on all three CI platforms, and what an upgrade involves.
- [ ] **Step 2: Decide and record**, with exact versions written as `=x.y.z` or a full commit hash.
- [ ] **Step 3: Write `hello_window`**: a window titled "UniDeX cookbook" with one label and one button that counts
  clicks.
- [ ] **Step 4: Verify.** `cargo run -p udx-cookbook --example hello_window` opens the window on macOS (hands-on).
  CI builds the example on all three platforms. `cargo deny check` passes with the new dependencies.
- [ ] **Step 5: Make worktree builds affordable** (spec 11.3). Set up a shared compiler cache (`sccache`, installed
  with the maintainer's approval) and a unified dependency set (`cargo-hakari`). Record in `0007` the cold and warm
  build times of the cookbook in a fresh worktree; the warm figure is what later task budgets assume.
- [ ] **Step 6: Commit.** `feat: cookbook crate on pinned GPUI; decision 0007`

### Task 10: Trial, streaming chat view and terminal

**Model:** Opus. **Code is throwaway**, in `spikes/chat-term/`.

**Files:**
- Create: `spikes/Cargo.toml`, `spikes/chat-term/`, `docs/decisions/0008-chat-and-terminal-trial.md`,
  `docs/decisions/0009-pty-crate.md`

- [ ] **Step 1: Build the trial.** A virtualised list of 5,000 markdown messages with a stream appending 200 tokens a
  second to the last one, and a terminal pane (PTY plus `alacritty_terminal` plus our own grid element).
- [ ] **Step 2: Measure on macOS by hand.** Frame pacing while streaming and while scrolling; first-token delay;
  terminal correctness with a full-screen program, resizing and 24-bit colour.
- [ ] **Step 3: Build on Linux and Windows in CI.** Record what compiled and what needed platform-specific code.
- [ ] **Step 4: Try both PTY options** (`portable-pty` and the tty module of `alacritty_terminal`) on all three CI
  platforms. Record the choice in `0009`.
- [ ] **Step 5: Write `0008`**: what worked, what did not, numbers, and the patterns to copy into the cookbook.
  Commit the records only. `docs: decisions 0008 and 0009`

### Task 11: Trial, platform integration

**Model:** Opus. **Code is throwaway**, in `spikes/platform/`.

**Files:**
- Create: `spikes/platform/`, `docs/decisions/0010-platform-integration-trial.md`, `docs/decisions/0011-tray-crate.md`

- [ ] **Step 1: Build one window that exercises**: text entry with a CJK input method and dead keys; a tray icon with
  a popover; the application menu; a second window; open and save dialogs; a `unidex://` link.
- [ ] **Step 2: Check on macOS by hand**, including the system screen reader reading a button and a text field.
- [ ] **Step 3: Check on Linux and Windows in CI** as far as a runner allows; state plainly what could not be
  checked without hardware.
- [ ] **Step 4: Write `0010`** with a table of feature by platform (works, broken, unknown), and `0011` naming the
  tray crate. If screen readers do not work, say so; spec section 8 already lists it as an exception.
  Commit the records only. `docs: decisions 0010 and 0011`

### Task 12: Trial, Chromium inside a GPUI window

**Model:** Opus. **Code is throwaway**, in `spikes/browser/`. This is the riskiest trial; run it early.

**Files:**
- Create: `spikes/browser/`, `docs/decisions/0012-embedded-browser-trial.md`

- [ ] **Step 1: Build the trial.** A helper process runs CEF with off-screen rendering. The app draws its frames as an
  element, forwards pointer, keyboard and text-entry events, and sends two DevTools commands (navigate, evaluate).
- [ ] **Step 2: Measure on macOS by hand.** Frame rate while scrolling a long page; typing in a form, including a
  CJK input method; a popover drawn over the page; memory per tab; size of the Chromium component on disk and
  compressed.
- [ ] **Step 3: Build on Linux and Windows in CI.** Record what worked. For Windows, also assess the OS web view
  behind the same `BrowserView` trait. For Linux, record whether Chromium's sandbox runs in a `.deb` install.
- [ ] **Step 4: Decide per platform** in `0012`: embed with CEF, embed with the OS web view (Windows only), or fall
  back to an external window. Set the browser component's size, memory and frame-rate targets from the measurements.
  Commit the record only. `docs: decision 0012`

### Task 13: Trial, rendering without hardware

**Model:** Opus. **Code is throwaway**, in `spikes/render-ci/`.

**Files:**
- Create: `spikes/render-ci/`, `docs/decisions/0013-rendering-in-ci-and-vms.md`

- [ ] **Step 1: In CI**, start the cookbook window on the Linux runner (virtual display, software renderer) and on
  the Windows runner, capture one frame, and upload it as a build artifact.
- [ ] **Step 2: Sketch the automation socket**: one action sent, the element tree read back, one frame captured, on
  whichever platforms Step 1 rendered.
- [ ] **Step 3: Virtual machines** (only with the maintainer's approval to install the software): does the cookbook
  window render in a Linux and a Windows virtual machine on the development machine?
- [ ] **Step 4: Write `0013`**: what "CI-verified" can honestly cover per platform, and whether virtual machines
  become a hands-on step. Commit the record only. `docs: decision 0013`

### Task 14: Packaging tool and the nightly pipeline

**Model:** Sonnet, reviewed by Opus.

**Files:**
- Create: `.github/workflows/nightly.yml`, `docs/decisions/0014-packaging-and-identity.md`

- [ ] **Step 1: Package the cookbook window** as an unsigned trial app on all three platforms with each candidate
  (cargo-packager, Velopack, cargo-dist). Compare: update mechanism, signing flow per platform, extra tooling needed,
  Linux formats (`.deb` and a tarball are required).
- [ ] **Step 2: Decide and record** the tool, and propose the app's bundle identifier and display name for the
  maintainer to confirm. Neither may contain personal information.
- [ ] **Step 3: Write `nightly.yml`** to build the three packages and attach them to the workflow run as artifacts.
  It publishes nothing and creates no release.
- [ ] **Step 4: Verify.** A manual run produces three artifacts. Download the macOS one and open it by hand.
- [ ] **Step 5: Commit.** `ci: nightly trial packages; decision 0014`

### Task 15: Baseline of the Electron app

**Model:** Opus.

**Files:**
- Create: `docs/baseline/electron-2.7.0.md`

- [ ] **Step 1: Measure the Electron app 2.7.0** on the reference machine, using that project's own test build and a
  generated fixture vault. Never the maintainer's real vault, and never by sending Apple Events to a running app.
  Metrics: cold start to first frame, idle wake-ups over 60 seconds, idle memory, frame pacing while scrolling a
  5,000-message thread, download size.
- [ ] **Step 2: Write the method** for each metric so the same measurement can be repeated on the Rust app.
- [ ] **Step 3: Compare with spec section 9** and propose any target that should change, with the reason.
- [ ] **Step 4: Check and commit.** Privacy gate exits 0 (no machine names, no folder paths).
  `docs: Electron 2.7.0 performance baseline`

### Task 16: The M0 gate

**Model:** the orchestrating session.

**Files:**
- Create: `docs/build/M0-GATE.md`
- Modify: the spec (sections 5.7, 5.8, 8, 9, 13), `README.md`
- Delete: `spikes/`

- [ ] **Step 1: Fold the decisions into the spec.** Each of records `0006` to `0014` changes at most the sections
  named above. A decision that contradicts the spec is raised with the maintainer, not applied silently.
- [ ] **Step 2: Copy the patterns worth keeping** from the trials into `udx-cookbook` as small examples that CI
  builds. Then delete `spikes/`.
- [ ] **Step 3: Write `M0-GATE.md`**: the decisions in one table; what stayed unknown; tokens used per task from the
  ledger; and proposed token budgets per task type and for M1 as a whole.
- [ ] **Step 4: Run the full gate.** `cargo fmt --all --check && cargo clippy --workspace --all-targets --locked --
  -D warnings && cargo test --workspace --locked && cargo deny check && cargo xtask privacy --all`. Expected: all
  pass locally and on all three platforms in CI.
- [ ] **Step 5: Hand over.** The maintainer reads `M0-GATE.md`, checks the cookbook window and the trial package on
  macOS by hand, and signs off. Only then is the M1 plan written.

---

## Order and parallel work

- Tasks 1 to 4 run in order; everything else needs them.
- Tasks 5, 6, 7 and 8 produce documents and can run together.
- Task 9 comes before Tasks 10 to 14. Task 12 starts first among the trials.
- At most two of Tasks 10 to 14 compile at the same time.
- Task 15 is independent. Task 16 is last.
