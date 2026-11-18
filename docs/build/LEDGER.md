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
| 2026-11-16 | M0 | Task 4 CI on three platforms | builder | Sonnet | 1 build and 2 fix rounds | 175k | done, eight checks green | #1 |
| 2026-11-16 | M0 | Task 4 | reviewer | Opus | 1 review and 1 re-review | 226k | needs fixes: 1 critical, 1 important; then 1 new finding open | #1 |
| 2026-11-16 | M0 | Task 4 | re-reviewer | Sonnet | 1 round | 105k | approved against the CI run | #1 |
| 2026-11-17 | M0 | Task 4 mutation tests | test author | Sonnet | 1 and 1 fix round | 192k | 20 tests, 0 missed mutants | #1 |
| 2026-11-17 | M0 | Task 4 mutation tests | reviewer | Opus | 1 | 235k | needs fixes: 4 tests | #1 |
| 2026-11-17 | M0 | Task 4 mutation tests | builder (Task 2's builder) | Sonnet | 2 more turns | about 20k | done, 2 source changes | #1 |
| 2026-11-17 | M0 | Task 4 mutation tests | re-reviewer | Sonnet | 1 round | 114k | approved | #1 |
| 2026-11-16 | M0 | Task 5 ledger and decision log | builder | Sonnet | 1 | 101k | done | #3 |
| 2026-11-16 | M0 | Task 5 | reviewer | Sonnet | 1 | not recorded | approved, no critical or important findings | #3 |
| 2026-11-17 | M0 | Task 6 threat model | builder | Opus | 1 build and 1 fix round | 430k | done | #6 |
| 2026-11-17 | M0 | Task 6 | reviewer (adversarial) | Opus | 1 review and 1 re-review | 347k | needs fixes: 3 critical, 24 important; then 1 important open | #6 |
| 2026-11-17 | M0 | Task 6 | builder (fresh, fix round 2) | Opus | 1 | 229k | done, 61 proposals | #6 |
| 2026-11-17 | M0 | Task 6 | re-reviewer | Sonnet | 1 round | not recorded | approved | #6 |
| 2026-11-16 | M0 | Task 7 acceptance checklist | builder | Sonnet | 1 build and 1 fix round | 230k | done, 346 rows | #4 |
| 2026-11-16 | M0 | Task 7 | reviewer | Opus | 1 review and 1 re-review | at least 264k | needs fixes, then approved | #4 |
| 2026-11-17 | M0 | Task 8 add-on vendor terms | builder | Opus | 1 build and 1 fix round | 416k | done | #5 |
| 2026-11-17 | M0 | Task 8 | reviewer | Opus | 1 review and 1 re-review | at least 281k | needs fixes: 1 critical, 4 important; then approved | #5 |
| 2026-11-17 | M0 | Task 9 GPUI source and cookbook | builder | Opus | 1 | 383k | done; two questions went to the maintainer | #7 |
| 2026-11-17 | M0 | Task 9 | reviewer | Opus | 1 | 277k | needs fixes: 4 important | #7 |
| 2026-11-17 | M0 | Task 9 | builder (fresh, fix round) | Sonnet | 1 | 140k | done | #7 |
| 2026-11-17 | M0 | Task 9 | re-reviewer | Sonnet | 1 round | not recorded | approved | #7 |
| 2026-11-17 | M0 | Task 10 chat and terminal trial | builder | Opus | 1 | 564k | done, decisions 0008 and 0009 | #9 |
| 2026-11-17 | M0 | Task 10 | reviewer | Sonnet | 1 | 185k | needs fixes: 7 important | #9 |
| 2026-11-17 | M0 | Task 10 | builder (fresh, fix rounds) | Sonnet | 3 fix rounds | 180k for round 1; not recorded for rounds 2 and 3 | done | #9 |
| 2026-11-17 | M0 | Task 10 | re-reviewer | Sonnet | 3 rounds | 169k for round 1; not recorded for rounds 2 and 3 | approved | #9 |
| 2026-11-17 | M0 | Task 11 platform integration trial | builder | Opus | 1 | 554k | done, decisions 0010 and 0011 | #10 |
| 2026-11-17 | M0 | Task 11 | reviewer | Sonnet | 1 | 147k | needs fixes: 3 important | #10 |
| 2026-11-17 | M0 | Task 11 | builder (fresh, fix round) | Sonnet | 1, resumed after the usage-limit stop | 138k | done | #10 |
| 2026-11-17 | M0 | Task 11 | re-reviewer | Sonnet | 1 round | not recorded | approved | #10 |
| 2026-11-17 | M0 | Task 12 embedded browser trial | builder (phase A) | Opus | 1 | not recorded; context noted as near 600k | phase A done, stopped before the CEF download as planned | #12 |
| 2026-11-17 | M0 | Task 12 | builder (fresh, phase B) | Opus | 1, resumed after the usage-limit stop | 599k | done, decision 0012 | #12 |
| 2026-11-17 | M0 | Task 12 | reviewer | Opus | 1 | 360k | needs fixes: 7 important | #12 |
| 2026-11-17 | M0 | Task 12 | builder (fresh, fix round) | Opus | 1 fix round over 3 CI rounds | 614k | done; Linux trial job red on a recorded gap | #12 |
| 2026-11-17 | M0 | Task 12 | re-reviewer | Sonnet | 1 round | not recorded | approved | #12 |
| 2026-11-17 | M0 | Task 13 rendering in CI | builder | Opus | 3 rounds | 475k after round 2; not recorded for round 3 | done, decision 0013 | #11 |
| 2026-11-17 | M0 | Task 13 | reviewer | Sonnet | 1 | not recorded | needs fixes: 5 important | #11 |
| 2026-11-17 | M0 | Task 13 | builder (fresh, fix round) | Sonnet | 1 | not recorded | done | #11 |
| 2026-11-17 | M0 | Task 13 | re-reviewer | Sonnet | 1 round | not recorded | approved | #11 |
| 2026-11-17 | M0 | Task 15 Electron baseline | builder | Opus | 1 | 443k | done | #8 |
| 2026-11-17 | M0 | Task 15 | reviewer | Sonnet | 1 review and 1 re-review | at least 146k | needs fixes: 5 important; then approved | #8 |
| 2026-11-17 | M0 | Task 15 | builder (fresh, fix round) | Sonnet | 1 | 142k | done | #8 |
| 2026-11-17 | M0 | Task 14 packaging and the nightly workflow | builder | Sonnet | 3 CI rounds and 1 fix round | 355k | done, decision 0014 proposed | #13 |
| 2026-11-17 | M0 | Task 14 | reviewer | Opus | 1 | 273k | needs fixes: 3 important | #13 |
| 2026-11-17 | M0 | Task 14 | re-reviewer | Sonnet | 1 round | 121k | approved | #13 |
| 2026-11-17 | M0 | Task 16 the M0 gate, stage 1 | builder (drafter) | Opus | 1 | 570k | done | #14 |
| 2026-11-17 | M0 | Task 16 stage 1 | reviewer | Sonnet | 1 | 213k | needs fixes: 4 important | #14 |
| 2026-11-17 | M0 | Task 16 stage 1 | builder (fix round) | Sonnet | 1 | 200k | done | #14 |
| 2026-11-17 | M0 | Task 16 stage 1 | re-reviewer | Sonnet | 1 round | 166k | approved | #14 |
| 2026-11-17 | M0 | Task 16 the M0 gate, stage 2 | builder (cookbook builder) | Opus | 1 | 563k | done | #15 |
| 2026-11-17 | M0 | Task 16 | builder (documents drafter) | Sonnet | 1 | 201k | done | #15 |
| 2026-11-17 | M0 | Task 16 | reviewer (code) | Sonnet | 1 | 225k | needs fixes: 2 important | #15 |
| 2026-11-17 | M0 | Task 16 | reviewer (documents) | Sonnet | 1 | 185k | needs fixes: 2 important | #15 |
| 2026-11-17 | M0 | Task 16 | builder (fresh, fix round, code and documents) | Sonnet | 1 | 163k | done | #15 |
| 2026-11-17 | M0 | Task 16 | re-reviewer | Sonnet | 1 | 132k | approved | #15 |
| 2026-11-17 | M0 | Task 16 | reviewer (final whole-branch) | Opus | 1 | 503k | ready to merge with fixes: 10 important, none critical | #15 |
| 2026-11-17 | M0 | Task 16 | builder (fresh, final fix wave) | Opus | 1 | 371k | done | #15 |
| 2026-11-17 | M0 | Task 16 | re-reviewer (final) | Sonnet | 1 | 202k | approved | #15 |
| 2026-11-17 | M0 | M0 sign-off and gate decisions (questions 1 to 5) | maintainer's decision, recorded in the documents | not applicable | 1 | not applicable | M0 signed off; questions 1 to 5 decided and recorded in the gate report, design, threat model and decision 0014; the section d hand checks not done, still owed; questions 6 to 27 open | #17 |

## Notes

- Token figures are cumulative per agent, as reported by the harness.
- A builder's report once claimed a change that had not applied. The re-review caught it, so reports are always checked
  against the diff.
- From Task 4 on, the pull request is given by number. Tasks 1 to 4 landed in the first pull request, #1.
- "not recorded" means the harness's figure was not captured; "at least" marks a figure captured before the agent's
  last turn was reported.
- A role marked "fresh" is a new agent that took over from the original, usually a fix round when the original's
  context had grown large; it has its own row.
- Agents stopped at a usage limit and then resumed keep one cumulative figure across the stop.
| 2026-11-17 | M1 | Gate decisions written into the documents | builder | Sonnet | 2 | 229k | done; one fix round (1 important, 6 minor findings) | #17 |
| 2026-11-17 | M1 | Gate decisions written into the documents | reviewer | Sonnet | 2 | 173k | approved after one fix round | #17 |
| 2026-11-17 | M1 | Architecture and task breakdown | architect | Opus | 1 | 534k | done; 38 tasks, caps of 43.0M against the gate report's 21.9M | #18 |
| 2026-11-17 | M1 | Architecture and task breakdown | reviewer | Opus | 1 | 440k | approved with fixes: 4 critical, 16 important, 24 minor findings | #18 |
| 2026-11-18 | M1 | Architecture and plan, fix round 1 | fix author | Opus | 1 | 560k | all findings addressed; the task file became the plan, 45 tasks, caps of 50.2M | #18 |
| 2026-11-18 | M1 | Architecture and plan, fix rounds 2 and 3 | fix author | Opus | 2 | 607k | done; round 2 fixed 2 partly resolved and 5 new important findings, round 3 fixed 2 more | #18 |
| 2026-11-18 | M1 | Architecture and plan | re-reviewer | Opus | 3 | 598k | approved after three fix rounds; three clarifications carried to Tasks 2b, 6 and 24 | #18 |
