# 0025: Governed paths: flag and review

Status: accepted on 2026-11-17
Date: 2026-11-17

## Context

The project's own safety checks (the privacy gate and its allow files, the secret scanner's configuration, the
workflows, the dependency policy and the build tool's configuration) are ordinary files in the repository, so a pull
request could weaken them. The gate report asked for required review of those files through a code-owners file. That
cannot be enforced today: the hosting service does not let the account that opens a pull request approve it, and every
pull request is opened from the maintainer's account, so a required approval could never be given.

## Options considered

| Option | Trade-off |
|---|---|
| A. Required review through a code-owners file and a ruleset requirement | The intended end state, but it blocks every pull request with no one able to approve it while there is one account. |
| B. Flag and review: a check on the base branch flags a pull request that touches a governed file; a process rule adds an extra security review | Works with one account. Not enforced by the service: a merge is not blocked, so it rests on the process rule. |
| C. Do nothing for governed files | A weakened check would pass unnoticed. |

## Decision

Option B, "flag and review". The workflow `governance.yml` runs on `pull_request_target`, so the base branch's
definition runs and a pull request cannot edit the check away. It builds the base branch's `xtask`, reads the pull
request's changed files through the API with a read-only token, as data, and runs `cargo xtask governed --files <list>`.
The subcommand prints one warning annotation per governed file, writes a job summary table, and exits 0 whether or not
it flagged anything; it exits nonzero only on its own error (an unreadable list, a failed API call). The check is named
`governed-paths (flag)`. It never checks out, builds or runs the pull request's code, holds only `contents: read` and
`pull-requests: read`, and uses no secret.

Process rule: every flagged pull request gets an extra security review by an Opus reviewer, limited to the governed
files, and the controller reports each such change to the maintainer.

The governed list (one table in `xtask/src/governed.rs`, patterns anchored at the repository root, `*` never crossing a
folder separator):

| Pattern | Why |
|---|---|
| `xtask/src/privacy/**` | The privacy gate's code decides what counts as personal information. |
| `xtask/src/hooks.rs` | The commit hooks run the privacy gate before a commit. |
| `xtask/src/governed.rs` | This check's own code and its governed list. |
| `xtask/tests/privacy*.rs` | The privacy gate's tests define what it must catch. |
| `xtask/tests/secrets.rs` | The secret-scanner tests define what it must catch. |
| `xtask/tests/hooks.rs` | The hook tests define what the commit hooks must do. |
| `xtask/tests/github_committer.rs` | The committer-identity tests guard the commit author check. |
| `xtask/tests/governed.rs` | This check's own tests and its governed list. |
| `privacy-allow.toml` | Every exemption from the privacy rules lives here. |
| `.gitleaks.toml` | Every exemption from the secret scanner lives here. |
| `.github/**` | Widened: the whole folder. Workflows, composite actions, dependency-update configuration and a future code-owners file all steer CI. |
| `deny.toml` | The dependency policy: licences, advisories and sources. |
| `**/.cargo/**` | Widened: a `.cargo` folder at any depth, because the build tool honours configuration in sub-folders. |

Two widenings go beyond the first list, because a flag is cheap and a miss is not: a `.cargo` folder at any depth, and
everything under `.github/` (so `.github/CODEOWNERS` and a nested composite action are flagged).

File names are data. Annotations escape `%`, carriage return, newline, `:` and `,` in properties and `%`, carriage
return and newline in messages. In the summary table a name is made safe for one cell (`|`, backtick, `<`, `>`, `&`,
backslash and line breaks are replaced by character references), so a name cannot inject a workflow command or markup.
The list is passed as a file of NUL-ended names, so a name holding a newline stays one name.

## Who decided

The maintainer, on 2026-11-17, answering the gate report's first-task item: "flag and review". The two widenings were
ruled by the controller in the task's brief.

## Consequences

- Easier: a change to a safety check is visible in the annotations and the job summary of the pull request, and the
  check cannot be edited away by the change it judges.
- Harder: the flag does not block a merge, so the extra review rests on the process rule above. The hosting service's
  list of a pull request's files stops at 3000 files; a pull request that large is not fully listed, which a reviewer
  must treat as flagged.
- Owed for the gate report: enforced human approval, through a ruleset requirement (by a code-owners file or
  otherwise), once a second account exists (outside contributors, or a separate account for the build agents).
- Not verified until after merge, because a `pull_request_target` workflow runs only from the base branch and cannot
  judge its own pull request. The controller observes two scratch pull requests and records them in the ledger: one
  touching `deny.toml` shows the annotation and the summary and the check passes; one that edits `governance.yml` to
  print nothing is still flagged, because the base branch's definition runs. Both are closed and their branches
  deleted. Also recorded from that run, as information only: whether a `pull_request_target` run attaches to the pull
  request's head commit as a check the ruleset could require.

## Revisit when

A second account exists that can approve a pull request opened by the first: then replace the flag by a required
review of the governed files, and keep the annotation as the pointer to them.
