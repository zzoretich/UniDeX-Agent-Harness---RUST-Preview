# 0025: Governed paths: flag and review

Status: accepted on 2026-11-17
Date: 2026-11-17

## Context

The project's own safety checks (the tool that hosts them, the allow files, the workflows, the dependency policy and
the build tool's configuration) are ordinary files in the repository, so a pull request could weaken them. The gate
report asked for required review of those files through a code-owners file. That cannot be enforced today: the hosting
service does not let the account that opens a pull request approve it, and every pull request is opened from the
maintainer's account, so a required approval could never be given.

## Options considered

| Option | Trade-off |
|---|---|
| A. Required review through a code-owners file and a ruleset requirement | The intended end state, but it blocks every pull request with no one able to approve it while there is one account. |
| B. Flag and review: a check on the base branch flags a pull request that touches a governed file; a process rule adds an extra security review | Works with one account. Not enforced by the service: a merge is not blocked, so it rests on the process rule. |
| C. Do nothing for governed files | A weakened check would pass unnoticed. |

## Decision

Option B, "flag and review". The workflow `governance.yml` runs on `pull_request_target` (types `opened`,
`synchronize`, `reopened`, `ready_for_review` and `edited`, the last so a change of the base branch is checked again),
so the base branch's definition runs and a pull request cannot edit the check away. It builds the base branch's `xtask`,
reads the pull request's changed files through the API with a read-only token, as data, and runs the built
`xtask governed` on them. The check is named `governed-paths (flag)`. It never checks out, builds or runs the pull
request's code, holds only `contents: read` and `pull-requests: read`, and uses no secret.

The subcommand prints one warning annotation per governed file, writes a job summary table, and exits 0 whether or not
it flagged anything; it exits nonzero only on its own error (an unreadable list or summary, a malformed argument). The
subcommand never sees the API: the list step of the workflow fails the job if the API call fails. The runner shows only
a limited number of annotations per step, so the summary table is the complete list.

How the list is made, and what the subcommand does with it:

- Every changed file is listed under its name and, for a rename or a copy, under its previous name too, each name ended
  by a NUL byte (so a name holding a newline stays one name). A governed file renamed or moved out of the governed set
  is therefore flagged under its old name.
- The workflow passes the pull request's changed-file count (from the event) and the number of entries the API listed.
  When fewer were listed, or when the number listed reaches 3000 (the hosting service's documented listing limit, not
  verified offline, and the event's count may be capped the same way), the tool prints a warning annotation and a
  summary line saying the list is incomplete and the pull request must be treated as governed, and still exits 0.
- The workflow passes the base and head commit ids from the event; the tool refuses an id that is not 40 or 64
  hexadecimal characters and prints the two ids in the summary as "Run for base X and head Y (from the event)": they name the commits the run was
  started for, not ones the file list was proven to match.
- A name is matched as given and after case folding (upper-casing, then lower-casing, so characters that fold to ASCII
  letters are caught), with letter case ignored (patterns are anchored at the repository root, except the rows that begin `**/`, which apply at
  any depth). Any name holding a non-ASCII character is flagged, with its own reason
  ("non-ASCII name: may shadow a governed path on a case-folding file system"), because such a name can alias a governed
  file on a case-folding checkout.
- File names are data. In annotations `%`, carriage return, newline, `:` and `,` are escaped in properties, and `%`,
  carriage return and newline in messages; a backslash, a non-ASCII character and a control character are written as
  `\u{hex}` (a backslash as `\u{5c}`), so no two different names print the same. In the summary a name is made safe for
  one table cell: the table cannot be broken, no raw HTML can be added, no bracket link or image and no asterisk or
  tilde emphasis can be written, and line breaks and control characters are replaced. Underscore emphasis, at-mentions
  and bare web addresses can still render; they cannot add a row or leave the cell.
- Local use: the tool also reads `git diff --name-status -z --find-renames` output (`--name-status`). Before a merge,
  from a checkout of the base branch, run
  `git diff --name-status -z --find-renames BASE...HEAD > changed.status` and then
  `cargo xtask governed --files changed.status --name-status`, naming the real base and head. The tool accepts only
  git's status letters (`A C D M R T U X`, with a score for renames and copies) and fails on anything else, so a plain
  list given by mistake is an error, not a clean result. The output has the same
  annotations and, with `GITHUB_STEP_SUMMARY` unset, no summary. This run is not capped at 3000 files and does not
  depend on which events fired.

The governed list (one table in `xtask/src/governed.rs`; patterns anchored at the repository root, `*` never crossing a
folder separator, letter case ignored; the any-depth rows begin `**/`):

| Pattern | Why |
|---|---|
| `xtask/**` | The tool that hosts the privacy gate, the hooks and this check, with their tests. Any redirect of the gate's own modules has to edit a file here. |
| `privacy-allow.toml` | Every exemption from the privacy rules lives here. |
| `.gitleaks.toml` | The secret scanner's rules and its exemptions. |
| `.gitleaksignore` | The secret scanner reads findings to ignore from this file. |
| `.github/**` | Workflows, composite actions, dependency-update configuration and a code-owners file all steer CI. |
| `deny.toml` | The dependency policy: licences, advisories and sources. |
| `.deny.toml` | A second spelling of the dependency policy. |
| `**/.cargo` | A file or link named `.cargo` at any depth can stand for a build configuration folder. |
| `**/.cargo/**` | Build-tool configuration at any depth can redirect sources or run programs. |
| `**/.gitattributes` | Attributes at any depth can hide a file's changes from a review or change what the privacy gate reads. |
| `.gitmodules` | A submodule brings code in from outside the repository. |
| `Cargo.toml` | The workspace manifest can patch or replace any dependency. |
| `rust-toolchain.toml` | The compiler and cargo for every check come from this file. |
| `rust-toolchain` | The older spelling of the toolchain file. |
| `CODEOWNERS` | The code-owners file decides who must review. |
| `docs/CODEOWNERS` | The code-owners file decides who must review. |
| `crates/udx-core/src/paths.rs` | The path guard that confines every test to its test root. |
| `crates/udx-core/src/lib.rs` | A path attribute here could redirect the governed path guard module. |
| `docs/decisions/0025-governed-paths-review.md` | The record that defines the governed set and the review rule. |

`Cargo.lock` is deliberately not governed: every dependency update would flag, and the licence and advisory checks
cover it. The older step "Flag changes to files that govern the checks" in `ci.yml` stays until it is reconciled with
this list (owned by the tasks that edit that file); it runs from the pull request's own branch, so this record and the
base-branch check are the authoritative ones.

Process rule: every flagged pull request gets an extra security review by an Opus reviewer, limited to the governed
files, and the controller reports each such change to the maintainer. Before every merge the controller reads the
governance run of the pull request, identified by its workflow file (`governance.yml`) and its `pull_request_target`
event, not by a check name (another workflow could create a job of the same name). A run that is flagged, failed,
cancelled, missing, stale (its head id differs from the pull request's current head, or the pull request's base branch is not `main`) or reported an incomplete list counts as
flagged. The controller also runs the local check above on the base branch's tool for the same base and head.

## Who decided

The maintainer, on 2026-11-17, answering the gate report's first-task item: "flag and review". The governed list in the
brief was `xtask/src/privacy/**`, `xtask/src/hooks.rs`, `xtask/src/governed.rs`, the privacy-gate tests (`xtask/tests/privacy*.rs`), `xtask/tests/secrets.rs`, `xtask/tests/hooks.rs`, `xtask/tests/github_committer.rs`, `xtask/tests/governed.rs`,
`privacy-allow.toml`, `.gitleaks.toml`, `.github/workflows/**`, `deny.toml` and `.cargo/**`. The widening to the whole
`.github/` folder and to a `.cargo` folder at any depth, and, after two reviews, the rest of the table above (the whole
`xtask/` folder, the added files, case folding and the non-ASCII flag, the previous names, the incomplete-list check, the
base-change event and the commit ids, and the process rule's wording), were the controller's rulings during the task,
not the brief's text.

## Consequences

- Easier: a change to a safety check is visible in the annotations and the job summary of the pull request, and the
  check cannot be edited away by the change it judges.
- Harder: the flag does not block a merge, so the extra review rests on the process rule above. The flag has false
  positives by design (any file in `xtask/`, any non-ASCII name).
- Owed for the gate report: enforced human approval, through a ruleset requirement (by a code-owners file or
  otherwise), once a second account exists (outside contributors, or a separate account for the build agents).
- Not verified until after merge, because a `pull_request_target` workflow runs only from the base branch and cannot
  judge its own pull request. The controller observes scratch pull requests and records them in the ledger: one
  touching `deny.toml` shows the annotation and the summary and the check passes; one that edits `governance.yml` to
  print nothing is still flagged, because the base branch's definition runs; one that renames a governed file is
  flagged under its old name. All are closed and their branches deleted. Also recorded from those runs, as information
  only: whether a `pull_request_target` run attaches to the pull request's head commit as a check the ruleset could
  require.

## Revisit when

A second account exists that can approve a pull request opened by the first: then replace the flag by a required
review of the governed files, and keep the annotation as the pointer to them.
