# Task ledger

This file records the work done on the project. One line is appended for each finished role-run (a builder, a test
author, a reviewer or a re-reviewer finishing a task). Nothing is edited after the fact except to correct an error.

Any session, human or agent, restarts cold from three places: this file, the decision records in `docs/decisions/`, and
the current plan under `docs/design/plans/`.

| date | milestone | task | role | model | attempts | tokens | outcome | pull request |
|---|---|---|---|---|---|---|---|---|
| 2026-11-16 | M0 | Task 1 workspace skeleton | builder | Sonnet | 1 | 99k | done | first pull request |
| 2026-11-16 | M0 | Task 1 | reviewer | Opus | 1 | 112k | approved, no critical or important findings | first pull request |
| 2026-11-16 | M0 | Task 2 privacy gate | test author | Sonnet | 1 | 103k | 16 failing tests | first pull request |
| 2026-11-16 | M0 | Task 2 | builder | Sonnet | 1 build and 3 fix rounds | 209k | done, 51 tests | first pull request |
| 2026-11-16 | M0 | Task 2 | reviewer | Opus | 1 | 201k | needs fixes: 1 critical, 7 important | first pull request |
| 2026-11-16 | M0 | Task 2 | re-reviewer | Opus | 2 rounds | 217k | 2 rounds with findings open | first pull request |
| 2026-11-16 | M0 | Task 2 | re-reviewer | Sonnet | 1 round | 95k | approved | first pull request |
| 2026-11-16 | M0 | Task 3 path guard | test author | Sonnet | 1 | 100k | 11 failing tests | first pull request |
| 2026-11-16 | M0 | Task 3 | builder | Sonnet | 1 build and 2 fix rounds | 129k | done, 24 tests | first pull request |
| 2026-11-16 | M0 | Task 3 | reviewer | Opus | 1 review and 2 re-reviews | 216k | needs fixes (1 critical, 1 important), then approved | first pull request |

## Notes

- Token figures are cumulative per agent, as reported by the harness.
- A builder's report once claimed a change that had not applied. The re-review caught it, so reports are always checked
  against the diff.
