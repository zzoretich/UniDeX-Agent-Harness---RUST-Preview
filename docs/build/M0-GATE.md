# M0 gate report

Status: complete; signed off by the maintainer on 2026-11-17, who decided questions 1 to 5 (see "Maintainer's
decisions" below); questions 6 to 27 are open · 2026-11-17. It includes decision 0014 (packaging), the cookbook patterns
and the removal of the trial code.

This report is for the maintainer. It says what milestone M0 built, what its trials found, and what only the
maintainer can decide before M1 is planned. Every statement points to a record in this repository: a decision record
("decision 0012"), the design ("design 9"), the threat model ("threat model 7.1"), the acceptance checklist, the
Electron baseline, or the task ledger (`docs/build/LEDGER.md`). A few facts come from the build team's working notes,
which are not in the repository (cited as "working notes"); the controller can show the entry. A recommendation is
always marked as the build team's.

Words used throughout:

- **Tier 1 and Tier 2** (design 8): macOS gets a hands-on check by the maintainer at every gate; Linux and Windows are
  "verified by CI only".
- **A trial** was throwaway code in `spikes/` that answered one question and ended in a decision record. The trial code
  is removed at this gate and remains in history (section a).
- **Run by a program** means a script or the trial's own self-check drove the app and read the result back. Nobody
  operated any trial by hand. Where a person is needed, the step is listed in section d.

## Maintainer's decisions (2026-11-17)

The maintainer signed off milestone M0 and decided questions 1 to 5, the group due before M1 is planned. The hand
checks in section d were not done at sign-off: they are recorded as not done and stay owed, to be done before the first
gate that depends on them. Questions 6 to 27 are not answered and stay open, each before the milestone named in its
heading (questions 24 to 27 whenever convenient). The design, the threat model and decision 0014 were updated to match;
the question texts below are left as written, because they are the record of what was asked, and each of questions 1 to
5 carries a "Decided" line.

| Question | Decision |
|---|---|
| 1. Linux shell confinement | Option A, with C where the profile is absent. The `.deb` installs an AppArmor profile for the sandbox helper, so mount and network namespaces work. On installs without the profile (the tarball) the shell, stdio MCP servers and add-on adapters are off (fail closed), listed as a design 8 exception. |
| 2. Threat-model proposals for M1 | Option A. All 31 proposals accepted as written: P1 to P4, P6, P7, P43, P47, P49 to P51, P58, P8 to P10, P48, P13 to P15, P54, P59, P21 to P23, P57, P24, P37, P40, P41 for its M1 part only (the definition of an unattended run; the rest of P41 stays for M6), P60 and P39. The five conflicts in M1's area are settled in the threat model's favour: (a) features that are off on default Ubuntu and Windows with no auto-allowed command become design 8 exceptions under goal G7; (b) a repository is untrusted until the user trusts it (P13); (c) reads outside the workspace ask rather than being refused, and writes outside need a rule naming the folder (P60, threat model 4.2); (d) provider hosts and permission widening change only interactively (P22, P47), which qualifies "every capability, headless"; (e) spend caps arrive in M1, not M2 (P40). |
| 3. Reference machine and definitions | Machine option A: the reference machine is an Apple silicon desktop with 16 GB and an external display at 100 Hz. Design 9 carries the caveat that passing at 100 Hz may not mean passing on a 120 Hz laptop display (8.3 ms per frame instead of 10) and that thermal headroom differs. The baseline's definitions are adopted as this question lists them (and as `docs/baseline/electron-2.7.0.md`, "Comparison", states them), including the new target of opening a 5,000-message thread in under 100 ms to the first frame. |
| 4. Scrolling | Option C. The scroll target is the baseline's definition (at most 1 frame interval in 1,000 over 1.5 times the refresh interval and none over twice it, from presented frames, at 2,000 and 10,000 px/s), and M1 builds a persistent shaped-text cache as the way to meet it. Whether the cache sits around GPUI or needs a GPUI change is a design call for M1, to be recorded in a decision record when made. The chat's cache of prepared messages with eviction needs no decision. |
| 5. Packaging and identity | (i) Display name "UniDeX Rust Preview". (ii) The placeholder identifier `example.unidex.rust-preview` stays until the project has a domain or an organization, and must be replaced before anything is signed. (iii) "UniDeX contributors" stays as publisher, author, copyright and Debian maintainer, with no contact address. (iv) The daily nightly schedule stays. (v) Builds stay unsigned through M1: M1 delivers unsigned nightly builds, and signing is decided at the first tagged preview (M4). Condition (a), the two unhashed build-time fetches, still blocks the first signed build and any release. Condition (b), where the signing secrets live, is not decided and moves with signing to M4. The packaging tool choice (decision 0014) stands. |

## a. Where M0 stands

**On `main`**: every M0 task (pull requests #1 and #3 to #13; #2 was a throwaway proof that the privacy check fails a
bad commit; this gate is pull request #14):

- The workspace skeleton: `unidex --version`, the crates `udx-core`, `udx-cli`, `udx-test-support` and
  `udx-cookbook`, and `xtask`.
- The privacy gate (`cargo xtask privacy`) with commit hooks, and a path guard: for anything cargo starts, the three
  application folders (config, data and cache) that `udx-core`'s path resolution returns must lie inside a test root,
  not in the real user folders. It covers only code that uses that resolution; what design 10.1 asks beyond it is in
  section g, "The first task of M1". Both are covered by mutation testing in CI.
- CI on macOS, Linux and Windows with eight required checks: lint, tests on three platforms, licences and sources
  (`cargo deny`), advisories (`cargo audit`), mutation testing and the privacy scan.
- The task ledger and the decision log (records 0001 to 0014), the threat model (revision 3) and `SECURITY.md`, the
  acceptance checklist (346 items), the Electron 2.7.0 baseline, and the cookbook crate.
- Packaging and the nightly workflow (Task 14, decision 0014): packages of the cookbook window built in CI on all
  three platforms. The nightly workflow's daily schedule has been live since Task 14 merged on 2026-11-17. A scheduled
  run builds only when the newest commit is at most 25 hours old; the workflow attaches the packages to the workflow
  run and creates no release (on a public repository any signed-in user of the hosting service can download a run's
  artifacts); its artifacts are unsigned, named so and kept 7 days.
- The four trials (chat and terminal, platform integration, embedded browser, rendering in CI) and their temporary
  workflows are removed at this gate. They remain in history at commit c6880ec, the last commit on `main` that contains
  them. The patterns worth keeping are copied into the cookbook and listed in `crates/udx-cookbook/README.md`. The
  hands-on procedures for the platform trial and the browser trial are in `docs/acceptance/hands-on/`.

**In progress:** nothing. This gate's own pull request (#14) runs the required checks again when it is opened.

**CI:** the eight required checks were green on each task's last commit before it merged. While the trials existed, one
trial job was red by design: the embedded browser's Linux helper job failed on purpose because Chromium's GPU process
ran unconfined there (decision 0012; question 19). It went with the trial workflows. Hosted macOS runners were
repeatedly unavailable (a capacity message from the CI provider); that delayed several merges, Task 14's included, but
changed no result.

**Already decided by the maintainer during M0** (working notes): the plan, with installing the Rust toolchain and the
named cargo tools and the first push; that `cargo deny` fails an "unmaintained" notice only for a crate the workspace
uses directly (decision 0007); that the 0BSD licence is allowed (decision 0007); running the Electron baseline early;
downloading the CEF distribution for the browser trial and installing CMake and Ninja (decision 0012). Installing
virtual machine software was not approved and was not asked yet (question 24).

**What M0 did on the development Mac that the maintainer should know** (working notes):

- The browser trial's runs contacted Chromium's vendor's servers from this machine, and Chromium's component updater
  downloaded files into the trial's throwaway profiles (since deleted). On a strict reading that went beyond the one
  download the maintainer approved. No further launch was made after this was found (question 21).
- The browser trial left a short test string on the system clipboard. Its build folder, inside the trial's own
  worktree and ignored by git, holds about 4.2 GB; it can be removed once nobody needs the browser trial (section d).
- The Electron baseline's 67 launches each updated the development Electron binary's own preferences file in the home
  folder's preferences, because macOS ignores a substituted home folder for preferences. No vault, app data or agent
  configuration was touched (baseline, "Isolation").
- For one run the platform trial's app bundle existed for a few minutes and was deleted unopened; whether the system
  indexed it was not checked. The undo command in the platform trial's checklist covers it (decision 0010; section d
  says where the checklist now lives).

## b. What M0 found

Evidence levels used below: **program, dev Mac** (scripted runs on the development Mac, read back by the program);
**CI** (GitHub's hosted runners; which platforms is named); **source** (read in the pinned crates' source, nothing
ran); **not verified**.

| Trial or study | Question | Answer | Evidence | Record |
|---|---|---|---|---|
| GPUI source | Which GPUI, pinned how? | The `gpui-pre` 0.3.12 snapshot from crates.io, pinned exactly once in the workspace; the widget kit is deferred (question 23) | Built and launched on the dev Mac (nobody looked at the window); CI builds the cookbook on all three platforms; source compared with upstream file by file | 0007 |
| Chat view | Can GPUI carry a 5,000-message thread with a reply streaming in? | Yes on macOS, with one target missed: streaming and idle are well inside the targets; scrolling is not "no dropped frames" (up to 2 % of first-visit frames late); memory reaches the limit after a full scroll | Program, dev Mac (desktop, 100 Hz, busy machine); CI on three platforms: builds and windowless self-tests only; nothing drawn on Linux or Windows | 0008 |
| Terminal | Our grid element over `alacritty_terminal`? | Works on macOS: 13 of 13 correctness checks, a 200,000-line burst with no slow frame | Program, dev Mac; CI headless checks on three platforms; nothing drawn on Linux or Windows | 0008 |
| PTY crate | `portable-pty` or `alacritty_terminal`'s own PTY? | `alacritty_terminal`'s own PTY | Program, dev Mac; CI headless checks on three platforms. Two checks open: raw keystrokes on Windows (both crates), `vi` resize on Linux (intermittent) | 0009 |
| Platform integration | Text entry, input methods, dead keys, menus, windows, dialogs, links, quit gate, accessibility | macOS works with listed gaps; Linux and Windows have the code but no link delivery, no second-launch hand-off and no native menu bar; Wayland cannot place a popover; Dock Quit bypasses a quit gate on macOS | macOS: 35 checks by program through the operating system's own entry points; Linux and Windows: source, plus CI build and headless tests. The 14 hands-on steps are owed | 0010 |
| Screen readers | Does GPUI publish an accessibility tree? | On macOS, yes, for labelled controls and text fields; plain and styled text are not published | Program, dev Mac, through the OS accessibility API; VoiceOver itself not used; Linux and Windows: source only | 0010 |
| Tray | Which tray crate? | `tray-icon` 0.26.1 on macOS and Windows; Linux backend waits on a licence decision | macOS: program (a synthetic click, not a person's); Windows: source and CI build, never run; Linux: type-checked in CI only | 0011 |
| Embedded browser | Can Chromium run inside a GPUI window, off-screen in a helper process? | macOS yes; Linux only from a `.deb`, and not until its GPU process is shown confined; Windows ran under CEF's bootstrap in CI, headless, under an elevated account, nothing drawn; Chromium contacts its vendor by itself on every start observed | macOS: program, dev Mac (34 of 34 input checks at scale 1 after a script fix; 32 of 34 at scale 2 before it; three of the 34 are weaker than their names); Linux and Windows: CI headless only, nothing drawn, no input; Windows under an elevated account | 0012 |
| Rendering in CI | Can CI render the app and check frames without a GPU? | Yes: Linux X11 on software Vulkan and OpenGL, Windows Server on software Direct3D; frames checked automatically; an automation socket drives the app. Wayland passed but its check could not fail the job | CI on Linux and Windows, five runs; the socket also run by program on the dev Mac. Virtual machines: researched, not run | 0013 |
| Vendor terms (study, not a trial) | May UniDeX drive Claude Code and Codex through ACP, and who pays? | Both only with API-key credentials until a vendor confirms otherwise | Primary sources read on 2026-11-16; a reviewer fetched every cited page. Not legal advice | 0006 |
| Electron baseline (study) | How fast is the Electron app 2.7.0? | Warm start 724 ms (a lower bound for the shipped app), 625 idle wake-ups a minute, 239 MiB idle, scrolls at the display's rate, 304 to 641 ms to show a reply in a long thread | Program, dev Mac, development Electron binary with a mock keychain | baseline |
| Packaging | Which tool makes the `.dmg`, the Windows installer and the `.deb`, and can CI build them nightly? | cargo-packager 0.11.8 for the `.dmg`, the Windows NSIS installer and, for now, the `.deb`; the tarball is a plain `tar`; the `.deb` moves to cargo-deb when the browser's setuid helper arrives (cargo-packager cannot keep a setuid file or run an install script). Velopack (no `.dmg`, AppImage only on Linux) and cargo-dist (no `.dmg` or `.deb`) rejected. The record was "proposed" when written; the maintainer accepted it on 2026-11-17 with the identity (question 5) | CI, three platforms: the `.dmg`, `.deb`, tarball and Windows installer built unsigned from the cookbook window. The `.deb` was installed and removed on a Linux runner, and the Windows installer installed and removed on a Windows runner (file, library and registry checks). Nothing was opened on a display and nothing was run by a person; signing, notarization, the updater and the browser's nested bundles were not tried, and the Windows `.msi` was built and its contents listed but not installed | 0014 |

## c. Decisions for the maintainer

27 decisions, in three groups. Each entry gives the question, why it arises, the options with their cost and risk, the
build team's recommendation, and what happens if there is no answer. Where a question would change one of the design
sections this gate updated (5.7, 5.8, 8, 9 and 13), or contradicts approved text elsewhere (goal G4, section 5.10), the
design carries a one-line pointer to it and its approved wording stands.

| Group | Questions |
|---|---|
| Before M1 is planned | 1 to 5 |
| Before the milestone that needs it | 6 to 23 (question 23 is the one due before M1's chat window is built) |
| Whenever convenient | 24 to 27 |

The next two paragraphs describe the design as it stood before the decisions of 2026-11-17, and are kept as the record
of what this report said when it was written; for the design as it is now, see "Maintainer's decisions" above.

Changes the design already took (each settles something it left open, or states a fact about M0 beside the approved
text without changing it): the PTY crate (5.7), how browser frames travel and CEF on Windows (5.8), what "CI-verified"
covers in decision 0013's words, including the "(on X11)" qualifier on the Linux automation socket's frame capture (8),
the tray crate on macOS and Windows (8, 13), the GPUI pin (13), the packaging tool for the `.dmg`, the Windows
installer and the `.deb`, with the tarball made separately and the `.deb` moving to a second tool when the browser's
setuid helper arrives (8, 13; decision 0014, accepted on 2026-11-17), and that M0 measured on a desktop, not the
reference laptop (9). Section 7 was corrected to match the checklist, and a pointer to question 6 was added at goal G4.
If any of these should not stand, say so.

Where a record disagrees with text the maintainer approved, the design was not changed and the disagreement is a
question: the widget kit (5.10; question 23; section 13 only states that the kit was deferred and points to the
question), the reference machine (9; question 3), and goal G4 and design 3 (question 6). Besides its status line, the
design carries pointers to questions in this report only in the five sections this gate updated (5.7, 5.8, 8, 9 and 13),
at goal G4 and on section 5.10's widget-kit line; section 12 also points to section g for M1's first task, which is not
a question. Design 3 has none, and neither do the other passages that threat model 7.1's conflicts concern (section 5's
crate list, 5.3, 5.4, 5.6, 5.10's secrets fallback, 5.11, 6 and 7; goal G7's conflict is reached only through section
8's pointer to questions 1, 2, 10 and 11): those conflicts are listed in threat model 7.1 and in questions 1, 2, 13, 19
and 22, without a pointer in the design. Decision 0014 disagrees with no approved text: the design's package row and its
statements on signing and updates stand with it. Its open points (identity, schedule, signing) are question 5, with a
one-line pointer.

### Must be settled before M1 is planned

#### 1. Which mechanism confines shell commands on Linux?

**Decided (2026-11-17).** Option A, with C where the profile is absent: the `.deb` installs an AppArmor profile for the
sandbox helper so namespaces work; on installs without it (the tarball) the shell, stdio MCP servers and add-on
adapters are off (fail closed), listed as a design 8 exception.


**Why.** Design 5.11 says Landlock and seccomp on Linux. The threat model's proposals for blocking a command's network
and its calls to other programs (P1, P43) need mount and network namespaces, which default Ubuntu restricts through
AppArmor (threat model 7, 7.1). Chromium meets the same restriction (decision 0012). The threat model says this must be
decided before M1 starts (4.1).

**Options.**
- A. The `.deb` installs an AppArmor profile for the sandbox helper, so namespaces work. Cost: packaging work and a
  profile to maintain; needs root at install. Risk: installs without it (the tarball) have no confined shell, and under
  the fail-closed rule (threat model 5) the shell, stdio MCP servers and add-on adapters are off there.
- B. Landlock and seccomp only, as the design says. Cost: none extra. Risk: P1 and P43 are not met; a confined command
  can still send data out and reach other programs through the OS.
- C. The shell is off wherever namespaces are not available (P7), listed as a design 8 exception. Cost: a gap on
  default installs. Risk: users of the tarball lose the shell entirely.

**Recommendation (build team).** A, with C for installs without the profile. It is the same arrangement the browser
needs (decision 0012), so one packaging answer serves both.

**If not answered.** M1's plan cannot specify the Linux sandbox. The Linux shell would stay off (fail closed) while
macOS goes first. The Windows mechanism is decided inside M1's sandbox design and needs nothing now (threat model 7).

#### 2. Which of the threat model's proposals does M1 build, and which design conflicts do you accept for it?

**Decided (2026-11-17).** Option A: all 31 proposals accepted as written (P41 for its M1 part only, the definition of
an unattended run), and the five conflicts settled in the threat model's favour; see "Maintainer's decisions" above.


**Why.** The threat model proposes 61 controls, each "undecided until the maintainer accepts it" (threat model,
markings), and ranks them in section 8. Twenty-nine are scheduled for M1: P1 to P4, P6, P7, P43, P47, P49 to P51, P58
(permissions and the shell); P8 to P10, P48 (files); P13 to P15, P54, P59 (untrusted content); P21 to P23, P57
(secrets); P24 (network); P37 (a release check on nightly builds); P40 (spend caps from M1); P41's definition of an
unattended run. Two more are cross-cutting defaults with no milestone stated: P60 (a new workspace starts with the
least privilege) and P39 (private files are owner-only). Five rows of threat model 7.1 conflict with the design in
M1's area: features off on default Ubuntu and Windows with no auto-allowed command, each needing a design 8 exception
(goal G7); a repository is untrusted until the user trusts it (P13, against design 5.11's wording); reads outside the
workspace ask rather than being refused (P60, against 5.3); provider hosts and permission widening change only
interactively (P22, P47, against 5's "every capability headless"); spend caps from M1, not M2 (P40).

The 31 proposals, each with a short name taken from its own text and what it stops in the words of threat model 8:

| Proposal | Short name | What it stops |
|---|---|---|
| P1 | No network for sandboxed commands by default | A sandboxed command sending data out or resolving names |
| P2 | Allowed commands run as parsed arguments, with no shell | An allowed command carrying a payload through shell syntax, wildcard expansion or an option a rule's wildcard matched |
| P3 | Built-in asks in every mode, denied unattended | Destructive commands under permissive modes |
| P4 | Ask cards lead with the action in the app's words | Ask cards that hide or misstate the command, or the data a request would carry out |
| P6 | Delimiters with a per-session random value in the prompt-based tool protocol | Forged tool calls under the prompt-based protocol |
| P7 | The shell is off unless the full sandbox applies | A shell that runs unconfined because the sandbox silently failed |
| P43 | Sandbox profiles deny inter-process communication | A sandboxed command asking the OS to run code outside the sandbox |
| P47 | Rule sources and precedence | Permissions widened by a workspace file, a generalised "allow always", rule precedence, or a built-in ask turned off for good |
| P49 | The CLI escapes control characters; its asks need a typed word | CLI asks spoofed by output or pre-filled input |
| P50 | Session-owned process groups | Processes outliving their grant, their session or the app |
| P51 | Per-command limits and a free-space floor | Fork bombs, memory balloons and full disks |
| P58 | Rate-limited asks; a turn stops after a set number of denials | Approval fatigue from flooded asks |
| P8 | Protected paths inside the workspace | Hooks and repository configuration planted inside the workspace |
| P9 | The file guard checks the opened handle, not a path string | Races, Windows path forms and link tricks in the file guard |
| P10 | An allow list for the sandbox's reads | The sandbox reading home files that no deny list names |
| P48 | Workspace edges | Persistence planted in startup files, launch agents or the app's own configuration |
| P13 | Workspace trust | A repository steering the model through its own files and third-party paths |
| P14 | Egress after exposure | Data leaving after a successful injection, to any host, the injecting one included, by a composed URL or a choice of links |
| P15 | The renderer never fetches a remote resource by itself | Remote images and links leaking data from rendered output |
| P54 | One opener for the whole app | The app opening `file:` and custom-scheme URLs from servers, agents or notes |
| P59 | A silently cut prompt stops the turn | Turns that continue after the prompt lost its labels |
| P21 | Stored secrets redacted from tool results | Secrets reaching the model through tool results |
| P22 | A key is bound to its provider's host | Keys sent to a provider host that was changed |
| P23 | A scrubbed environment for child processes; the secret store out of their reach | Commands using the user's agent sockets, tokens or secret store |
| P57 | No dumps, and no HTTP headers in logs | Keys in crash dumps and trace logs |
| P24 | `web_fetch` caps, and `https` for remote provider endpoints | Oversized responses and plain-HTTP provider endpoints |
| P37 | A release check on every packaged artifact | Test-only features or debugging switches in a release |
| P40 | Spend caps from M1 | Spend overruns, cap races and unpriced models, loopback proxies to paid providers included |
| P41 | Routine declarations; in M1 only the definition of an unattended run (the rest in M6) | A changed routine running unattended with its old grants, or reaching undeclared URLs |
| P60 | Least privilege by default for a new workspace | Permissive defaults in a new workspace |
| P39 | Owner-only files for secrets and private state | Private files readable by other local users |

**Options.**
- A. Accept these 31 as written and the five conflicts in the threat model's favour; update the design. Cost: M1 grows;
  every proposal has a test that M1 must pass. Risk: an architect may find one not buildable as written, which then
  comes back as a decision record.
- B. Accept the first rows of section 8 that land in M1 (P14, P1, P43, P13, P7, P47) and P40, and decide the rest at
  the M1 gate. Cost: less M1 work. Risk: the rest are built later, or retrofitted.
- C. Decide each proposal with the M1 architect's specs. Cost: a longer planning round.

**Recommendation (build team).** A. Section 8's order puts P14, P1, P43 and P13 at the top, and all of them land in M1.

**If not answered.** The proposals stay "Proposed", M1 builds to the design only, and the security review at the M1 gate
measures against the design. The other 30 proposals (one withdrawn, one demoted to a backstop) are decided before their
own milestones (questions 13, 22). P38 (protected tags and a release environment that needs your approval) is scheduled
for M0 in the threat model; M0 did not build its check (question 26).

#### 3. Which machine is the reference, and how is each speed target defined?

**Decided (2026-11-17).** Machine option A, with the 120 Hz and thermal caveat written into design 9; the baseline's
definitions adopted as listed below, including the 100 ms target for opening a 5,000-message thread.


**Why.** Design 9 names an Apple silicon laptop with 16 GB as the reference machine. Every M0 measurement was taken on
an Apple silicon desktop with 16 GB and external displays at 100 Hz (and 60 Hz in the browser trial), under other load
(baseline, decisions 0008 and 0012). A 120 Hz laptop display leaves 8.3 ms per frame instead of 10 (decision 0008). The
baseline also proposes how each target is defined, because "cold start", "no periodic wake-ups" and "no dropped frames"
cannot be checked as written. The first-token target has not been measured through a real provider (decision 0008).

**Options for the machine.**
- A. Name the desktop and its 100 Hz display as the reference. Cost: none. Risk: passing at 100 Hz may not mean passing
  on a 120 Hz laptop; thermal headroom differs.
- B. Keep the laptop and repeat the baseline's start-up, scroll and token figures on it before the first gate that uses
  them (baseline). Cost: access to such a laptop and a measurement session. Risk: none beyond time.

**Options for the definitions** (all from the baseline, "Comparison"): start-up is spawn to the first presented frame
showing the home screen's navigation, median of ten warm launches, with a first launch reported separately; idle is at
most 60 wake-ups and 0.1 CPU-seconds a minute (placeholders until a floor is measured on an empty native window);
memory is the summed physical footprint 90 seconds after the home screen is up; scrolling allows at most 1 frame
interval in 1,000 over 1.5 times the refresh interval and none over twice it, from presented frames, at 2,000 and
10,000 px/s; the first token is also required in a 5,000-message thread, timed from the engine reading the provider's
bytes; download size is the compressed macOS arm64 disk image without the browser; and a new target: open a
5,000-message thread in under 100 ms to the first frame. Accept them, change them, or keep design 9 as written.

**Recommendation (build team).** Adopt the baseline's definitions. For the machine, B if a laptop is available,
otherwise A with the 120 Hz caveat written into design 9.

**If not answered.** The M1 gate measures on the desktop against targets whose events and statistics are not defined,
so "a miss blocks the gate" becomes a judgement call.

#### 4. What does "no dropped frames" mean for the chat, and how is it met?

**Decided (2026-11-17).** Option C: the baseline's definition is the target, and M1 builds a persistent shaped-text
cache to meet it; whether it sits around GPUI or needs a GPUI change is an M1 design call, recorded in a decision
record when made.


**Why.** In the chat trial, scrolling 5,000 messages on a first visit had up to 20 of 1,000 frame intervals over 1.5
times the refresh interval, worse than Electron's 2 of 9,998 (decision 0008). The likely cause is that GPUI keeps
shaped text only from one frame to the next, so rows new to the view are shaped again; it was not isolated. A miss
blocks a gate (design 9), and M1 has a real chat window.

**Options.**
- A. Build a persistent shaped-text cache (around GPUI, or as a GPUI change carried as a fork). Cost: unknown size of
  work; a fork is carried across every GPUI upgrade (decision 0007). Risk: the cause is likely, not proven.
- B. Adopt the baseline's definition (question 3) as the target. Cost: none. Risk: the trial still misses it (2 % late
  against at most 0.1 %).
- C. Both: B as the target, A as the way to meet it.

**Recommendation (build team).** C.

Whatever the answer, one thing follows from decision 0008 and needs no decision: the chat needs a cache of prepared
messages with eviction (memory reached 145 to 149 MiB after scrolling the whole thread, at the 150 MB limit). The widget
kit for message bodies is a separate decision (question 23).

**If not answered.** The target stays as written, and M1's chat window would likely miss it at its gate, as the trial
did.

#### 5. Packaging and identity: the app's name, its schedule, and signing

**Decided (2026-11-17).** (i) "UniDeX Rust Preview". (ii) The placeholder identifier stays until the project has a
domain or an organization, and is replaced before anything is signed. (iii) "UniDeX contributors", no address. (iv) The
daily schedule stays. (v) Unsigned through M1; signing is decided at the first tagged preview (M4), where condition (b)
moves too; condition (a) still blocks the first signed build and any release.


**Why.** Decision 0014 chose cargo-packager and built unsigned packages of the cookbook window on all three platforms.
The record is "proposed": the tool is a design call from the trial and stands unless you reopen it, and the identity is
yours to confirm. The trial uses placeholders: the identifier `example.unidex.rust-preview.trial`, the display name
"UniDeX Rust Preview", and "UniDeX contributors" as publisher, author, copyright and Debian maintainer. None carries a
personal name, handle or domain. Five parts need you. Parts (i), (iii) and (iv) have no deadline stronger than "before a
build is released"; part (ii), the identifier, must be settled before the first signed preview, and part (v) before M1's
first signed nightly (design 6), so the whole question sits before M1 is planned.

**(i) The display name.** This is the value that sticks. On Windows it names the install folder and the uninstall
entry (the CI install confirmed both); on macOS it is the app bundle's name. Renaming it after a release leaves the old
install beside the new one. The identifier does not do this on Windows (decision 0014). The Debian package and the
program's file name are `unidex-rust-preview` whatever you choose.

- A. "UniDeX Rust Preview", the record's proposal: it matches the edition's name and differs from the Electron app's
  "UniDeX". Cost: none. Risk: it names the edition "Rust Preview", which is wrong once the app is no longer a
  preview, and a rename then leaves two installs.
- B. "UniDeX Preview". Cost: none. Risk: close to the Electron app's name.
- C. "UniDeX (Rust Preview)". Risk: parentheses are awkward in file names and paths (decision 0014).

**(ii) The bundle identifier.** On macOS it keys the app's preferences, Keychain items and privacy permissions, so
changing it later makes macOS treat the app as new (decision 0014).

- A. `example.unidex.rust-preview`: claims no domain, and `.example` can never be registered. Cost: none. Risk: not
  fit for a release you mean to keep; it must be replaced before the first signed preview, which resets preferences and
  permissions.
- B. `dev.unidex.rustpreview` (or another top-level domain): conventional and under the project's control once the
  project registers the matching domain. The project owns none today, and the record did not check that any is
  available. Risk if used before owning the domain: its owner could ship an app with the same identifier.
- C. `io.github.<project-organization>.unidex-rust-preview`: collision-safe by construction and usual for projects with
  no domain, but it needs a GitHub organization named for the project first. The repository sits under a personal
  account whose handle must not appear.

**(iii) Publisher, copyright and Debian maintainer.** All four fields hold "UniDeX contributors". A Debian maintainer
normally has an address; `dpkg` accepted this one without. Options: keep the string with no address; add a project
contact address (it would sit in every package, and the repository has none today: `SECURITY.md` offers no email);
or choose another string. Say which, and whether the copyright line stays.

**(iv) The nightly's schedule.** The nightly workflow already runs on a daily schedule (it has since Task 14 merged
on 2026-11-17), as well as on demand and on pull requests that touch it. A scheduled run builds only when the newest
commit is at most 25 hours old, creates no release, and attaches unsigned packages to the run for 7 days, where any
signed-in user of the hosting service can download them. That rule was exercised on the pull request path only when
the record was written (decision 0014).

- A. Keep the daily schedule. Cost: hosted runner time on three platforms each day that has a commit; hosted macOS
  runners were often unavailable in M0, so a scheduled run can fail or wait for that reason. Risk: unattended red runs.
- B. Run only on demand and on pull requests. Turning the schedule off means removing the `schedule:` key and its cron
  line from the workflow's triggers; the `decide` job's schedule branch is then dead code. Cost: nobody sees a break
  until someone starts a run. M1 promises nightly builds (design 6).

**(v) Signing identities.** Needed before the first signed nightly, which M1 delivers on macOS (design 6). Decision 0014
lists what you would supply as repository secrets: for macOS a Developer ID Application certificate (a base64 `.p12`)
and its password, the signing identity string, and notarization credentials (an Apple ID, an app-specific password and
a team id, or an App Store Connect API key); for Windows a code-signing certificate usable on the runner, or a cloud
signing service; for updates, if the cargo-packager updater is used, a minisign key pair. The secret names come from
the tool's source and have never been exercised. Three workflow changes must come first (decision 0014): secrets must
not be available to `pull_request` runs (the workflow runs on pull requests that change its paths); the `-vv` flag on
the Package step, which traces the whole configuration, must go; and the Package step is one matrix step, so each
platform's signing environment needs a condition on the operating system. Two more conditions come before any signing
secret is added to that step:

- (a) The two unverified build-time fetches (below) are hash-checked or supplied from verified copies. The `create-dmg`
  script runs inside the same Package step that would receive the certificate and the notarization credentials, so a
  tampered script could read them from its environment (its URL is pinned to a commit, which lowers the likelihood).
- (b) Where the secrets live and who approves their use is decided. Threat model P38 puts signing secrets only in a
  release environment that needs the maintainer's approval, and a scheduled nightly cannot satisfy that without a
  decision. The options: an approval-gated environment, which a schedule cannot use unattended; signing only on
  manually approved runs; or unsigned nightlies, with signed builds made separately.

Options for the signing identities:

- A. macOS first, as M1 promises: supply the Apple certificate and notarization credentials after (ii) is settled, so
  the signed app does not carry a placeholder identifier. Windows and updates wait for M4 or M11. Cost: an Apple
  developer account able to issue the certificate (its price is not in the record). Risk: low.
- B. All platforms at once. Cost: also a Windows certificate or signing service (cost not researched).
- C. Stay unsigned until the first tagged preview (M4). Cost: none. Risk: M1's deliverable changes (design 6), and
  macOS says what it says about an unsigned app (below).

**Also in decision 0014, open or not tried.**
- **A condition before the first signed build, and in any case before any release.** The packaging tool fetches two
  things at build time with no hash check: a small NSIS plugin whose DLL (203,264 bytes) ends up inside the Windows
  installer and runs on the user's machine at install time, and the `create-dmg` script that the macOS runner executes.
  The nightly fetches both on every run. Decision 0014 disclosed this after the Task 14 review found it undisclosed.
  Before the first signed build, and in any case before any release, each must be fetched with a hash check or
  supplied from a copy verified in advance (part v, condition a); the tool's source shows a way to use a
  pre-placed copy of each. (Its AppImage tooling is also unhashed, one more reason the AppImage is not used.) The
  tool's other downloads are hash-checked in the tool.
- **What macOS will say.** The unsigned app carries only the linker's signature, and `codesign --verify` and `spctl
  --assess` both fail on the CI-built copy. A downloaded copy may therefore be called "damaged", not "unidentified". The
  two fallbacks in the hand check (section d), removing the quarantine attribute from your copy and re-signing it with
  a local ad hoc signature, were not tried.
- **Nothing was opened on a display.** No package was opened on real hardware, and nobody knows whether the Linux or
  Windows program opens a window from its package; the installs were checked by file, library and registry entries.
- **The tool's age.** cargo-packager's last release is 2025-11-27 (its repository has commits in November 2026 and
  about 267,000 downloads); it is version 0.x; its configuration is one small file, so replacing it is bounded.
- **Updates were not tried.** The tool's updater is optional and not forced by the choice; it does not cover `.deb`
  installs or make deltas. Whether to use it is a decision for M4's auto-update work (decision 0014).

**Recommendation (build team).** (i) A. (ii) A until a domain or an organization exists, then B or C before the first
signed preview, and before (v). (iii) Keep "UniDeX contributors" and add an address only when the project has one.
(iv) A: M1 asks for nightlies, and the schedule rule needs a real scheduled run to be tested. (v) A, after (ii) and
after conditions (a) and (b). Confirm the display name with the same care as the identifier, because it is the harder
one to change. Condition (a) is not a decision: it blocks the first signed build, and in any case any release, and the
build team will keep it on the list. Where the secrets live, condition (b), is yours to decide.

**If not answered.** (i) and (ii) The placeholders stay in the unsigned nightlies; nothing is signed or released under
them, so the first signed build waits. (iii) The string stays as it is. (iv) The daily schedule keeps running. (v) No
signing; M1's signed macOS nightlies cannot be delivered, and no secret exists to be misplaced.

### Must be settled before the milestone that needs it

#### 6. Add-on engines: accept API-key use only, and change goal G4? (before M5)

**Why.** Goal G4 and design 3 say subscriptions are reached through the add-on engines. Decision 0006 found that both
vendors' current pages direct third-party tools to API keys, so both add-ons ship for API-key credentials only, and G4
holds for neither until a vendor confirms otherwise. For Claude Code the pages leave open whether UniDeX counts as a
product running Claude Code, under which plan use might be allowed. For Codex, OpenAI lets existing local or open-source
apps keep using the app server's login and does not say a new one may start.

**Options for Codex** (decision 0006, open point 3).
- A. API key only (the record's default). Cost: no ChatGPT-plan use for Codex in UniDeX. Risk: none from OpenAI's
  pages.
- B. Allow the user's existing Codex sign-in. Cost: needs OpenAI to confirm that a new local, open-source client may use
  app-server login, and UniDeX to stay free, local and open source. Risk: OpenAI's page speaks only of existing apps.
- C. Sign in with ChatGPT, OpenAI's route for third-party apps. Cost: a design change: UniDeX registers as a client,
  owns an OAuth sign-in and stores tokens, reversing the rule that it never handles a vendor login. Risk: plan use
  covers eligible Plus and Pro users only; a paid or hosted UniDeX needs OpenAI's approval.

**For Claude Code** (open points 1 and 2): ask Anthropic three questions (is UniDeX a product running Claude Code or a
product built on the Agent SDK; if the first, may it refuse plan credentials; if the second, could plan use be
approved), or stay API-key only without asking. The adapter's own guard lets an Enterprise plan pay; the record refuses
that until an Anthropic source allows it.

**Also in this decision.** OpenAI's usage policies gain a clause on 2026-11-20 requiring approval for services that let
third parties reach OpenAI models (decision 0006, open point 6). The record reads the Codex add-on as outside it; its
review noted the clause also bears on UniDeX's own engine calling OpenAI with the user's key (working notes). Re-read it
after that date. Whether the adapters' bundled copies of the vendor tools raise any terms issue is not stated in the
sources (open point 5).

**Recommendation (build team).** Accept decision 0006 as written (Codex option A, Claude API-key only), send Anthropic
the three questions before M5 is planned, and change G4 and design 3 now so the design does not promise what neither
add-on can deliver.

**If not answered.** Both add-ons are API-key only, G4 and design 3 stay inaccurate, and decision 0006 stays "proposed".

#### 7. How do add-on engines reach UniDeX's memory and browser tools? (before M5)

**Why.** Porting the checklist found no design text on how an add-on engine (Claude Code or Codex over ACP) gets the
vault's memory context and the `computer_*` browser tools; the review rated it high weight and said it must be settled
before M5 is planned (working notes, Task 7). The records name the question, not the options; the options below are the
build team's framing.

**Options.**
- A. Offer both through UniDeX's own MCP server (`unidex serve`, design 5.6), which the add-on connects to, with the
  threat model's limits on that server (P28, P61). Cost: browser tools added to the server. Risk: an outside agent then
  drives the signed-in browser through it.
- B. Inject memory into the add-on's prompt through ACP and offer no browser tools. Cost: small. Risk: add-on sessions
  lose a feature the native engine has.
- C. Neither: memory and the browser are native-engine features. Cost: none. Risk: checklist rows that assume them for
  add-ons change fate.

**Recommendation (build team).** A for memory now; the browser tools wait until the browser's own controls exist
(questions 21 and 22).

**If not answered.** M5's plan has to answer it before any add-on row can pass.

#### 8. What does UniDeX show for an add-on engine's cost? (before M5)

**Why.** The checklist review found no design text on usage and cost for add-on engines (working notes, Task 7). The
threat model notes their spend never appears in UniDeX's ledger (threat model 7, P40).

**Options.** A. Show usage only when an adapter reports it, labelled as the vendor's figure. B. Show none, and say
that the vendor bills it. C. Estimate it. Cost and risk: A depends on adapters; B is honest and cheap; C can mislead.

**Recommendation (build team).** B until an adapter reports usage, then A.

**If not answered.** No add-on cost is shown. Add-on spend stays outside UniDeX's spend caps either way (P40).

#### 9. The terminal's two open checks (before M4)

**Why.** In CI the 33-key raw keystroke round trip stopped at the key "z" on Windows with both PTY crates, and on
Linux `vi` sometimes did not see a resize (both crates in one run, one crate in the next), while other resize checks
passed (decision 0009). Neither separates the two crates; both are open for the real terminal.

**Options.** A. Investigate both in CI at the start of the terminal work. B. Wait for a Windows tester or a virtual
machine (question 24). C. Accept both as known risks and label Windows terminal input unverified.

**Recommendation (build team).** A, then B for a hands-on Windows check when possible.

**If not answered.** M4 starts the terminal with both open, and Windows terminal input stays unverified.

#### 10. Platform gaps found by the platform trial: exceptions or work? (before M4)

**Why.** Goal G7 says a feature works on all three platforms or is listed in design 8. Decision 0010 found: Linux and
Windows have no native menu bar (GPUI stores menus and shows nothing); Wayland cannot place a popover window; on macOS
Quit from the Dock and log-out bypass a quit gate, because GPUI's application delegate cannot refuse to terminate;
Linux file dialogs need the desktop portal; Windows input methods go through IMM32 only; and on Linux and Windows
opened links are never delivered and a second launch does not hand over to the running app. Separately, the checklist
review found that one Routines row lists launchd jobs, which exist only on macOS (working notes, Task 7).

**Options.**
- A. Accept decision 0010's exceptions in design 8 (the Linux tray menu-first, Linux file dialogs need the portal, no
  native menu bar on Linux and Windows with an in-window one drawn, macOS Dock Quit and log-out until handled, Windows
  IMM32 only), plus a cross-platform note for the launchd row, and plan the work the app owns: link delivery and a
  single-instance hand-off on Linux and Windows, quit handling on macOS, display mapping and window state (decision
  0010). Cost: that work, mostly in M4. Risk: low.
- B. Require native parity, for example native menus on Windows written by the project. Cost: high. Risk: work in
  platform code this project does not otherwise own.

**Recommendation (build team).** A.

**If not answered.** Design 8 lists none of them, and they show up at the M4 gate as failures against G7.

#### 11. The Linux tray: allow a licence, or write our own? (before M4)

**Why.** The tray crate's Linux backend that reports clicks (`ksni`) is under the Unlicense, which the licence policy
does not allow. The other backend brings GTK 3, LGPL system libraries, an unsound advisory that fails the policy, and no
click events (decision 0011). Changing the allow list is the maintainer's decision.

**Options.**
- A. Allow the Unlicense in `deny.toml`. Cost: one line. Risk: one more licence on the allow list.
- B. Write a small StatusNotifierItem over `zbus`, which GPUI's Linux tree already has. Cost: a few hundred lines
  including its menu (an estimate). Risk: more code to own and test.
- C. The AppIndicator backend. Rejected by decision 0011: it fails the advisory policy and cannot report a click.

**Recommendation (build team).** A, as decision 0011 recommends; B if you decline.

**If not answered.** The Linux tray is a design 8 exception until one of A or B lands (decision 0011).

#### 12. Narrow the screen-reader exception on macOS? (before M4, after your VoiceOver check)

**Why.** Goal G10 and design 8 list screen readers as an exception on every platform until GPUI supports them. On
macOS the app publishes a working accessibility tree for labelled controls and text fields, checked through the OS
accessibility API, but nobody has used VoiceOver with it (decision 0010). Step 13 of the platform checklist is that
check (section d).

**Options.**
- A. Once step 13 passes, narrow the exception to Linux and Windows, and to styled text, which publishes nothing. Cost:
  the app takes on decision 0010's accessibility rules (every control labelled, text fields publish text runs, styled
  text publishes its own nodes). Risk: low.
- B. Keep the exception everywhere. Cost: none. Risk: understates what works.
- C. Drop it. Rejected by decision 0010: it claims Linux and Windows without evidence.

**Recommendation (build team).** A, after step 13.

**If not answered.** The exception stays on all platforms.

#### 13. The remaining design conflicts in threat model 7.1 (before M2, M3, M5 and M6)

**Why.** Threat model 7.1 lists twelve conflicts with the design. Questions 1, 2, 19 and 22 cover eight; these four
remain:

| Milestone | Conflict | Design says | Threat model proposes |
|---|---|---|---|
| M2 | Skills and sub-agents arrive before their protection | Skills and sub-agents in M2, memory drafts in M3 (design 6) | Deny tool writes to skill and sub-agent files in M2 until drafts exist (P19) |
| M3 | `unidex serve` session tools | The server exposes memory, recall and session tools (5.6) | Session tools off by default (P28) |
| M5 | ACP mediation | An agent's requests pass through our permissions and sandbox (5.4) | Agents' own tools and project settings run outside those requests; gate the start (P44) and add a second layer (P29) |
| M6 | Secrets for unattended runs | Environment variables as a fallback where no keychain exists (5.10) | Scheduler files never hold secrets; a run without the keychain fails (P55) |

One open point of threat model 7 also belongs here and reaches no other question: unattended add-on engines, that is,
whether a routine may declare an ACP engine at all (L3), given that the engine's own tools sit outside UniDeX's
controls. The threat model gives it no milestone of its own; it ties it to P41's routine declarations (lesson L3's
row), which it schedules for M6, so it is due before M6 is planned.

**Options.** For each row: accept the threat model's position and update the design, or keep the design's wording
and record the remaining risk in threat model 7. For the open point: an add-on engine runs unattended only when a
routine declares it (P41), as the threat model's row for lesson L3 reads; or an add-on engine never runs unattended.

**Recommendation (build team).** Accept each before its milestone is planned; decide the open point with them,
before M6.

**If not answered.** The design's wording stands and the proposed control is not built. The open point stays open,
and M6's plan must answer it before a routine can name an add-on engine.

#### 14. How are the prompts and duties UniDeX seeds into a vault versioned? (before M6)

**Why.** The checklist found no design text on what happens to seeded prompts and duties when the app updates (working
notes, Task 7).

**Options.** A. Seeded files carry a version; an update offers the new version without overwriting the user's edits.
B. Never touch a seeded file after it is created. C. Overwrite on update. Cost and risk: A is more work; B leaves users
on old prompts; C destroys edits.

**Recommendation (build team).** A.

**If not answered.** M6's plan must answer it.

#### 15. May the user turn off local run recording? (before M6)

**Why.** The Electron app had a switch that stopped local run records; the design has none, so the checklist dropped
those items (checklist S3 and TS1; ledger, Task 7). This is a product question.

**Options.** A. Add the switch. B. No switch: runs are always recorded, locally only. Cost: A is small; B none.

**Recommendation (build team).** A: the Electron app offered it and the cost is small. No record argues either way.

**If not answered.** The rows stay dropped.

#### 16. Uninstall (before M6, when the OS wake-up arrives; and M11)

**Why.** Routines register one OS wake-up (design 5.9). The checklist found no row and no design text on uninstalling:
removing that wake-up, and keeping or deleting the vault (working notes, Task 7).

**Options.** A. A `unidex` command that removes the wake-up and the app's own configuration and keeps the vault unless
asked. B. Documented manual steps. C. The installer's own removal (for example a `.deb` removal script). Risk if
none: a removed app can leave a wake-up behind that calls a missing program.

**Recommendation (build team).** A, keeping the vault by default.

**If not answered.** There is no uninstall path.

#### 17. A read-only or staged write mode? (before M8)

**Why.** The Electron app could switch its Spaces, Brain and connector write actions off. The design has no such mode,
so the checklist dropped those items (SMR21, BR16, CN17; ledger, Task 7).

**Options.** A. Add a per-screen read-only switch in M8 to M10. B. Leave them dropped.

**Recommendation (build team).** B: the permission modes (design 5.2) already let the user deny writes. No record
argues either way.

**If not answered.** The rows stay dropped.

#### 18. The browser's size and memory against design 9 (before M7)

**Why.** Design 9 sets the app's targets "without the browser component" and says the browser's memory does not count
against the app's 150 MB. The trial (decision 0012) found the component is about 95 to 103 MB compressed and about 300
MB on disk for macOS alone, against 40 MB for the whole app; Linux and Windows were not measured. It also found that
drawing the browser adds 117 to 185 MB inside the app's own process while a page streams, which by design 9's wording
counts against the 150 MB; the trial's minimal app already used 124 MB idle. Decision 0012 proposes targets for the
component; its start-up target is not supported by the measurements yet (6 of 23 runs were slow for a reason not
found).

**Options** (decision 0012).
- A. The browser as a separately downloaded component with its own budget: 0012's targets (at most 100 MB compressed
  and 320 MB on disk for macOS arm64; Chromium at most 300 MB with a blank tab, 450 MB with a documentation page, 650
  MB while scrolling one; the app's frame path at most 200 MB above its idle footprint; frame rate, frame interval,
  UI-thread and key-to-frame limits at 60 Hz), with a stated per-tab and per-pane cost. Cost: design 9 grows a browser
  section. Risk: the 60 Hz targets were not tried at 120 Hz.
- B. Revise the application targets to include the browser. Cost: weaker targets for everyone. Risk: the app's own
  regressions hide behind the browser's numbers.

**Recommendation (build team).** A.

**If not answered.** Design 9 contradicts the measured cost, and the M7 gate would fail the app's memory target
whenever a page streams.

#### 19. The Linux browser: `.deb` only, and not until its GPU process is confined? (before M7)

**Why.** Design 5.8 keeps Chromium's sandbox on "on every platform" and fetches the browser on first use. On default
Ubuntu, Chromium's sandbox runs only with a setuid helper or an AppArmor profile installed as root, so the `.deb` must
install one of them (or the whole component) up front; the tarball and AppImage cannot run it. In CI, with either,
the renderers and the storage service were confined, but the GPU process ran without its seccomp filter on a runner
with no GPU. A flaw reached through it would act with the user's reach (decision 0012).

**Options.**
- A. Ship the browser on Linux only in the `.deb`, which installs the sandbox helper or profile at install time, and
  only after the GPU process is shown confined: on hardware with a GPU (a person with a Linux machine, since hosted
  runners have none and a self-hosted runner on a public repository is a security exposure, decision 0013), or by a
  supported Chromium setting (candidates, none tried: making a failed GPU sandbox fatal, starting it earlier, or
  rendering through Chromium's own software renderer). Cost: a Linux exception to "not in the installer"; a hardware
  check. Risk: the check may never be possible without a tester.
- B. The external browser window on Linux (design 5.8's fallback). Cost: no embedded browser on Linux. Risk: none.

**Recommendation (build team).** A as the plan, B until the evidence exists. Switches that turn the GPU sandbox off or
move the GPU work into the unsandboxed browser process are not candidates (decision 0012; design anti-goals).

**If not answered.** Per decision 0012, the browser does not ship on Linux and Linux uses the external window.

#### 20. The network service runs unsandboxed on Linux and Windows (before M7)

**Why.** In this CEF build Chromium starts its network service with no sandbox on Linux and Windows (on Windows it ran
unrestricted at the account's level); on macOS it is sandboxed. It parses what comes off the network, so a flaw in it
is outside Chromium's sandbox (decision 0012). This is Chromium's own choice, not a switch the trial passed. All
Windows evidence is headless and ran under the hosted runner's elevated administrator account; under a normal account
the browser process would run at a lower level, which was expected but not observed (decision 0012).

**Options.** A. Accept it on those platforms and record the risk in the threat model. B. Find a Chromium feature or
setting that sandboxes it (not tried) before shipping there. C. Treat it as blocking, like the Linux GPU process.

**Recommendation (build team).** B in M7, with A recorded if nothing is found.

**If not answered.** When the browser ships on Windows, this process runs unsandboxed, with nothing recording that you
accepted it.

#### 21. Chromium's background traffic: a condition for shipping, and may the trial run again? (before M7; part two now)

**Why.** On every start observed on the development Mac, before any page was opened, Chromium contacted its vendor's
servers (the component updater, network time and a search endpoint). Each contact discloses the machine's public
address, the Chromium version and the time of use, and the component updater downloaded files into the profile. Three
documented switches did not stop it (decision 0012). The design's anti-goals rule out network calls the user did not
configure. Decision 0012 makes a working control a condition for shipping the browser anywhere. Since this was found, no
further launch has been made on the development Mac (working notes), so the hands-on browser check (section d) waits for
your word.

**Options.**
- A. Accept decision 0012's condition: no browser ships until a control stops or routes the traffic. Candidates, none
  tried: disabling the responsible Chromium features by name, Chromium policies, a resolver rule or proxy that lets
  the browser resolve only the names it is sent to (what threat model P52 asks), a CEF build without the updater, an
  OS-level firewall per process. Cost: M7 work. Risk: none found may work.
- B. Ship with the traffic disclosed and an opt-out. Contradicts the anti-goal.
- C. Build CEF without the updater. Cost: our own Chromium build every four weeks (decision 0012).

And separately: may the browser trial be launched again on the development Mac, knowing each start contacts the
vendor, so you can do the hands-on check before a control exists?

**Recommendation (build team).** A, trying the resolver rule first. For the launch, your call; the build team does not
launch it without your word.

**If not answered.** The condition stands (decision 0012 is accepted with it) and no further launch happens.

#### 22. CEF's browser process and page clipboard writes (P33, P52) (before M7)

**Why.** CEF's browser process, which holds the signed-in profile, is outside Chromium's sandbox by design. The threat
model proposes confining it (P33) only where Chromium's sandbox still engages inside it (threat model 7.1). M0 did not
try it. CEF also has no clipboard hook: a page can write to the system clipboard on a click, and the agent's
forwarded click counts as one (decision 0012; threat model 4.9).

**Options.** A. Accept P33 as rewritten (try it in M7, drop it where it stops the renderer sandbox) and P52's fallback
(deny the page clipboard permission and list the remaining copy path in threat model 7). B. Accept the browser process
unconfined and page clipboard writes as they are, with the risk recorded.

**Recommendation (build team).** A.

**If not answered.** Neither is built: the browser process runs with the user's full reach and pages can write the
clipboard.

Not a decision, recorded here so it is not lost (decision 0012): the trial passed Chromium a switch that replaces the
macOS Keychain key protecting the profile's cookies and saved passwords with a fixed test key. The real browser must
not carry it forward. It needs Chromium's own Keychain item, read by a helper signed with a stable identity; macOS may
then ask the person to allow access, and that prompt names the helper, not the app.

#### 23. Is the widget kit adopted for chat message bodies? (before M1's chat window is built)

This is the one question in its group that is due before M1's chat window is built, earlier than the rest of the
group (section g).

**Why.** Design 5.10 says "GPUI with the pinned widget kit", and design 3 notes that the kit is published as
`gpui-kit`. Decision 0007 pinned the GPUI snapshot the kit requires and deferred the kit itself: its styled layer needs
the 0BSD licence (since allowed by the maintainer) and its tree carries more unmaintained notices. Decision 0008 then
measured it. It recommends not adopting the kit for the thread list or the terminal. For message bodies it recommends,
provisionally, the kit's unstyled layer (`gpui-base`) and its markdown `TextView`, for selection, copy, link handling,
code-block hooks and a parse that runs in the background while a reply streams, all of which GPUI lacks. The measured
cost was worse than our own renderer's in places: while resizing, 27 to 40 intervals over 1.5 times the refresh interval
against 12 to 23; a token p99 of about 21 ms against 16 to 18 ms; start-up 17 ms later. Memory after a full scroll was
smaller. Two limits: only the form that drops its parse when a row leaves the view was measured, not the cached form
recommended; and selection and links were not exercised. Before the recommendation is final decision 0008 lists four
measurements: the cached form in the scroll, jump, stream and resize scenarios; selection and copy within and across
messages in the virtualised list; link activation; and that a message whose parse lands a frame later does not shift the
rows around it. None has been made. Until this is answered, the design's wording and decision 0008 do not agree, so the
design was not changed; it carries a pointer.

**Options.**
- A. Adopt `gpui-base` and its markdown view for message bodies once the four measurements pass. Cost: 33 more crates, a
  parse that lands a frame later during a stream, a renderer we do not control (decision 0008), and a short
  measurement task first. Risk: the measurements may show it worse than our own renderer, which then wins.
- B. Do not adopt it: message bodies use our own renderer over GPUI's styled text (the `plain` form in the trial), and
  selection, copy, links and the streaming parse are built by us. Cost: that work, not estimated. Risk: it is larger
  than the kit's, though the renderer measured better.
- C. Decide at the M1 gate, building the chat window so the message-body renderer can be swapped. Cost: a boundary
  in M1's chat code. Risk: rework if the answer arrives late.

**Recommendation (build team).** C with A's measurements as an early M1 task: adopt the kit for message bodies only if
all four pass, otherwise B. The kit stays out of the thread list and the terminal either way (decision 0008).

**If not answered.** The kit is not a dependency (decision 0007), design 5.10's wording stands, and M1's chat window
uses our own renderer, the form the trial measured; in that measurement it missed the scroll target and reached the
memory limit (decision 0008; question 4).

### Whenever convenient

#### 24. A Linux virtual machine on the development Mac

**Why.** Design 8 says M0 checks whether virtual machines can render the app and, if so, they become a hands-on step
for Tier 2. M0 did not run one: installing the software needs your approval (decision 0013). A virtual machine is the
only way short of hardware to try a real desktop, input methods, the tray, notifications, dialogs, scaling, real fonts
and the screen readers. It does not close the hardware gap: the GPU is virtual, the guest is Arm, not the x64 that
Tier 2 names, and its speed says nothing about real machines.

**What you would approve** (decision 0013, checked 2026-11-17): UTM 5.0.9, free under Apache-2.0, the `UTM.dmg` asset of
the project's GitHub release, 304,702,085 bytes; an app dragged into Applications. Not verified: whether it is signed
and notarized (checkable with `codesign` and `spctl` before the first run), whether it needs a kernel extension or
administrator rights, the exact guest image and its size, and where it stores machines. About 30 GB of disk in all (an
estimate) and one guest at a time on 16 GB. Whether GPUI can use the guest's accelerated OpenGL device is untested; if
not, the guest renders in software as CI does.

**Options.** A. Approve UTM for an arm64 Linux guest now, as a trial, used at gates for the hands-on Linux items and
labelled as an Arm check. B. Decline: Tier 2 stays CI-only. C. Decide at M4 together with a Windows guest (Parallels is
the only route Microsoft authorizes and needs a yearly licence plus Windows 11 Pro; UTM's DirectX driver is a month old
and experimental).

**Recommendation (build team).** A, at your convenience and before the first gate that wants hands-on Linux items (it
is a trial and nothing else waits on it); Windows at M4 (decision 0013).

**If not answered.** No virtual machine, and design 8's sentence about virtual machines stays unfulfilled.

#### 25. A faster frame path for the browser needs a GPUI change

**Why.** The browser's frames are copied twice on the CPU and uploaded on the UI thread. Drawing CEF's shared textures
directly would save about 2 to 3 ms of the UI thread and 3 to 4 ms of other threads per large frame, but needs an
external-texture element in GPUI on every platform: a fork of the pinned snapshot, carried across every upgrade. The
same fork could expose what the Windows OS web view needs to sit under app UI (decision 0012).

**Options.** A. Not now; revisit if the browser targets tighten (high refresh or full-screen panes). B. Fork now.

**Recommendation (build team).** A, as decision 0012 recommends.

**If not answered.** A.

#### 26. Turn on private vulnerability reporting, and other repository settings

**Why.** `SECURITY.md` tells reporters to use GitHub's private vulnerability reporting and offers no email address.
That is a repository setting only the maintainer can turn on and keep on (threat model 7). If you accept P38 (question
2), protected tags and a release environment that needs your approval are also repository settings; the threat model
scheduled P38's check for M0 and M0 did not build it.

**Options.** Turn it on (a few minutes), or leave it off, in which case a reporter who follows `SECURITY.md` finds no
way to report privately.

**Recommendation (build team).** Turn it on, unless it already is. It takes minutes and nothing waits on it, so it sits
in this group; the sooner it is on, the sooner `SECURITY.md` is true.

**If not answered.** Unchanged.

#### 27. Delete one early CI run's logs? (optional)

**Why.** The public log of one Windows job in the first rendering run (run 44128026417, job 134197065060) prints a
security descriptor that includes the identifier of the hosted runner's built-in administrator account. That account
belongs to a throwaway runner, not a person. Later runs do not print it (decision 0013).

**Options.** Delete that run's logs, or keep them.

**Recommendation (build team).** Keep them: the identifier is not personal, and decision 0013 cites that run for some
figures. Delete them if you prefer.

**If not answered.** Kept.

## d. Hands-on checks the maintainer owes

Not done at sign-off (2026-11-17): they are recorded as not done and stay owed, to be done before the first gate that
depends on them.

| Check | What to do | Instructions | About how long |
|---|---|---|---|
| The cookbook window | Run `cargo run -p udx-cookbook --example hello_window`; expect a window titled "UniDeX cookbook" with a label and a button that counts clicks | `crates/udx-cookbook/README.md`, "Run an example" | A few minutes, most of it the first build |
| The trial package | Take a green `nightly` run on the default branch (scheduled or started by hand), not a run from a fork. The latest artifacts from Task 14's pull request (13) exist until 2026-11-24; after that, if no scheduled run has built, start one by hand from the workflow's page (manual dispatch). Download the artifact `unidex-rust-preview-nightly-unsigned-macos` (about 3.2 MB), unzip it, open the `.dmg`, and drag the app to a scratch folder, not Applications. macOS will refuse to open an unsigned download: it will say it cannot verify the app is free of malware or that it is damaged. If it offers a choice, use System Settings, Privacy & Security, "Open Anyway". A window titled "UniDeX cookbook" with a Count button should open. Afterwards quit it, drag the app to the Trash, eject the disk image and delete the download. Report what macOS said: it is the one result CI cannot give | Decision 0014, "Maintainer's step". Two fallbacks if it only says "damaged" were never tried: `xattr -dr com.apple.quarantine` on your copy, then, if it still refuses, `codesign --force --deep --sign -` on your copy | The record gives none; the build team's guess is about 10 minutes (not measured) |
| The platform trial's 14 steps | Input methods, dead keys, press-and-hold, the tray, menus, a second window, open and save dialogs, VoiceOver, quit | `docs/acceptance/hands-on/platform-trial.md`; it needs a separate checkout of commit c6880ec, as that file explains | About 15 minutes plus a first build of a few minutes |
| The embedded browser | Run the trial app with the real browser on a test page, then type in a form, use an input method, scroll a long page and watch the app's overlay card draw over the page | `docs/acceptance/hands-on/browser-trial.md`; it also needs a separate checkout of commit c6880ec, as that file explains | The build team's earlier guess was 10 to 15 minutes of looking once the app is built, plus a build time that was not estimated |

Three things to know before starting:

- **The trial code is gone from `main`.** The platform and browser procedures run the trials, which exist only in
  history at commit c6880ec. Each procedure says how to make that separate checkout, so the 14 steps and the browser
  check no longer depend on the code being in the working tree, and they take a few more steps than they did.
- **The platform checklist decides question 12.** Step 13 (VoiceOver) is what narrows the screen-reader exception on
  macOS.
- **The browser check waits for your word.** Every start of the trial browser contacts Chromium's vendor's servers and
  can download components, and no control exists yet (question 21), so it is not run until you say so.

## e. What stayed unknown

| Area | Not verified in M0 | What would verify it |
|---|---|---|
| GPUI | That windows look right on real Linux and Windows desktops and GPUs; the start-up cost of compiling shaders at run time; whether CI caches stay within the repository's quota; that the executor, Linux event loop and renderer substituted in the snapshot behave like Zed's own (decision 0007) | A Tier 2 tester or a virtual machine (question 24); a timed start-up; watching cache sizes; the snapshot comparison at each upgrade |
| Chat | Anything visual on Linux and Windows; glyph fallback, right-to-left text and wrapping as a person sees them on macOS; a 120 Hz display; a quiet machine; presentation time; a real provider's first token; rich rows (tool cards, images, diffs); selection across messages, link clicks, syntax highlighting; the widget kit's cached form (decision 0008) | The cookbook window and the gate's hands-on check; measurements on the reference machine (question 3); M1's mock-provider timing; the four kit measurements (question 23) |
| Terminal | Windows raw keystrokes; Linux `vi` resize; the terminal drawn on Linux and Windows; PowerShell and WSL; input methods in the terminal; many terminals for days (decisions 0008, 0009) | Question 9; a Windows tester or guest |
| Platform | Every step of the platform checklist; Linux and Windows beyond compiling; Windows input methods that use only the Text Services Framework; Wayland compositors without the text-input protocol; Narrator and Orca; restoring native window state; notifications from an unbundled app (decision 0010) | Section d; a tester or guest; M4's real screens |
| Tray | The Windows icon (never run); either Linux backend; which Linux desktops show the icon and what happens with no tray host; a person's click on macOS; whether GPUI's timers keep running while the tray menu is open on macOS; whether the unsound GTK code path is reachable (decisions 0010 and 0011) | Question 11; the platform checklist step 8; a tester |
| Browser | The maintainer's look on macOS; the full input set at scale 2 after the script fix; a real input source and its candidate window; the Linux GPU process on GPU hardware; a setting that sandboxes the network service; confining the browser process; containing clipboard writes; a control for the background traffic and what was downloaded; component size on Linux and Windows; anything drawn or typed on Linux and Windows; Windows under a normal account; codecs, camera, printing, drag and drop, page accessibility; 120 Hz; the slow start-ups; cold start; the Linux equivalent of the mock keychain (decision 0012) | Questions 18 to 22; section d; a Linux machine with a GPU |
| Rendering in CI | Client editions of Windows (the runner is Windows Server 2025); the flake rate beyond five runs; which font the Linux runner drew with; the Windows pipe against a second local user; input through the OS; scales other than 1; whether GPUI's test-support feature changes run-time behaviour; a 2 pixel capture offset on Linux; everything about virtual machines (decision 0013) | More runs; the real test harness in M1; question 24 |
| Speed | The reference laptop; the first token through a real provider; an idle floor for an empty native window; the signed Electron release's start-up; trackpad scrolling (baseline, decisions 0008 and 0012) | Question 3; M1's measurements |
| Vendor terms | Whether UniDeX counts as a product running Claude Code; how the Claude adapter reports a keyless Console sign-in; an isolated Codex credential store through the adapter; which pool a Codex personal access token bills; the bundled vendor binaries; OpenAI's clause after 2026-11-20 (decision 0006) | Question 6; Anthropic's and OpenAI's answers; an M5 check |
| Acceptance checklist | 13 rows carry a milestone chosen by judgement and marked "?" (the checklist's column notes): APP9 (M4), ST-IMPORT (M4), C1 and C3 (M1), P6 (M6), BR28 (M9), CX4 (M8), CH6 (M4), TM4 and TM20 (M8), TM22 (M4), LS3 (M6), LS6 (M11) | The architect of each row's milestone confirms or moves it when that milestone is planned; the maintainer decides only if one should move |
| Packaging | Any package opened on real hardware, and what macOS says about a downloaded copy (the app is not validly signed, so it may say "damaged"; both fallbacks untried); whether the Linux or Windows program opens a window from its package (the runners have no display; installs were checked by file, library and registry entries); signing, notarization, the updater and the secret names (read from the tool's source, never exercised); the nested helper bundles on macOS, the Windows bootstrap arrangement and the browser component's size in a package; that the `.deb` can carry the browser's setuid helper through cargo-deb (the tool measured it; cargo-packager's `.deb` cannot); the Windows `.msi`, never installed; download size against the 40 MB target (the program is a one-window example with the default release profile, so sizes measure the formats, not the product); the nightly's schedule rule, which only the pull request path had exercised when decision 0014 was written; the hash-checked fetching the record requires before the first signed build, and in any case before any release; the tool's last release date (2025-11-27) and its 0.x versioning (decision 0014) | The trial package hand check (section d); question 5; a first signed build; the browser packaged in M7; a Tier 2 tester or a virtual machine (question 24); a real scheduled run |
| Privacy gate and path guard | What the gate cannot see, its false positives, the missing local secret scan, coverage beyond the commit hooks, the hook installer's and the path guard's gaps (the final review of M0) | The first task of M1 (section g, "The first task of M1") |

Records that disagree:

- **The measurement machine and the reference machine.** The baseline's first paragraph calls its machine "the
  reference machine", while its "Conditions" and "Comparison" say it is a desktop and not the laptop design 9 names.
  The baseline is left as it is; question 3 settled it on 2026-11-17: the desktop is the reference machine.
- **One token figure.** `docs/build/LEDGER.md` gives Task 3's reviewer 216k; the working notes say "193k+"
  at the time the task finished. This report uses the figure in `docs/build/LEDGER.md`.

## f. What M0 cost

### Tokens per task and per role

Figures are tokens as the harness reported them, cumulative per agent: an agent that worked several turns (a build and
its fix rounds, or a review and its re-reviews) is one figure. They are context tokens, not billing units. "n/r" means
not recorded; a total marked "+" is a lower bound. Rows are in `docs/build/LEDGER.md`.

| Task | Builder | Test author | Reviewer | Fresh fix agent | Separate re-reviewer | Total | Reviews and fix rounds |
|---|---|---|---|---|---|---|---|
| 1 Workspace skeleton | 99k S | | 112k O | | | 211k | 1 review, no fixes |
| 2 Privacy gate | 209k S | 103k S | 201k O | | 217k O, 95k S | 825k | 1 review, 3 fix rounds |
| 3 Path guard | 129k S | 100k S | 216k O | | | 445k | 1 review, 2 fix rounds |
| 4 CI on three platforms | 175k S, plus about 20k S by Task 2's builder | 192k S (mutation tests) | 226k O, 235k O (mutation tests) | | 105k S, 114k S | 1,067k | Workflow: 1 review, 2 fix rounds; mutation tests: 1 review, 1 fix round |
| 5 Ledger and decision log | 101k S | | n/r S | | | 101k+ | 1 review, no fixes |
| 6 Threat model | 430k O | | 347k O | 229k O | n/r S | 1,006k+ | 1 adversarial review, 2 fix rounds |
| 7 Acceptance checklist | 230k S | | 264k+ O | | | 494k+ | 1 review, 1 fix round |
| 8 Add-on vendor terms | 416k O | | 281k+ O | | | 697k+ | 1 review, 1 fix round |
| 9 GPUI source and cookbook | 383k O | | 277k O | 140k S | n/r S | 800k+ | 1 review, 1 fix round |
| 10 Chat and terminal trial | 564k O | | 185k S | 180k+ S | 169k+ S | 1,098k+ | 1 review, 3 fix rounds |
| 11 Platform trial | 554k O | | 147k S | 138k S | n/r S | 839k+ | 1 review, 1 fix round |
| 12 Browser trial | about 600k O (phase A, approximate) and 599k O (phase B) | | 360k O | 614k O | n/r S | about 2,173k+ | 1 review, 1 fix round over three CI rounds |
| 13 Rendering trial | 475k+ O | | n/r S | n/r S | n/r S | 475k+ | 3 builder rounds, 1 review, 1 fix round |
| 14 Packaging | 355k S (3 CI rounds and 1 fix round) | | 273k O | | 121k S | 749k | 1 review, 1 fix round |
| 15 Electron baseline | 443k O | | 146k+ S | 142k S | | 731k+ | 1 review, 1 fix round |
| 16 The M0 gate, stage 1 | 570k O (drafter) | | 213k S | 200k S | 166k S | 1,149k | 1 review, 1 fix round |
| 16 The M0 gate, stage 2 and final review | 563k O (cookbook builder), 201k S (documents drafter) | | 225k S (code), 185k S (documents), 503k O (final whole-branch) | 163k S, 371k O | 132k S, 202k S | 2,545k | 1 review each of code and documents, 1 fix round and 1 re-review; then 1 final review, 1 fix wave, 1 re-review. Task 16 in all: 1,149 + 2,545 = 3,694k |

S is Sonnet, O is Opus. Incomplete figures: Task 5 (reviewer), 6, 9, 11 and 12 (each a re-reviewer), 10 (later fix
and re-review turns), 13 (only the first builder's first two rounds are recorded), and 12 (phase A is the agent's noted
context size, not a reported figure). The
controller's own usage is not recorded anywhere and is not included.

**Recorded total, Tasks 1 to 16 (Task 16 with both stages and the final review): about 15.4 million tokens** (a lower bound). The arithmetic:
the rows of Tasks 1 to 13 and 15 sum to 10,962k (211 + 825 + 445 + 1,067 + 101 + 1,006 + 494 + 697 + 800 + 1,098 + 839 +
2,173 + 475 + 731); Task 14 is 355 + 273 + 121 = 749k; Task 16 is 570 + 213 + 200 + 166 = 1,149k for stage 1 and 563 + 201 + 225 + 185 + 163 + 132 + 503 + 371 + 202 = 2,545k
for stage 2 and the final review, 3,694k in all; 10,962 + 749 + 3,694 = 15,405k.

By role: builders about 7.12M (46 %; Opus 5.60M, Sonnet 1.52M), reviewers about 4.40M (29 %), fresh fix agents
about 2.18M (14 %), separate re-reviewers 1.32M recorded (9 %), test authors 0.40M (3 %). The arithmetic, in thousands:
builders 5,427 + 355 + 570 + 764 (563 + 201) = 7,116 (Opus 4,464 + 570 + 563 = 5,597; Sonnet 963 + 355 + 201 = 1,519);
reviewers 2,997 + 273 + 213 + 913 (225 + 185 + 503) = 4,396; fresh fix agents 1,443 + 200 + 534 (163 + 371) = 2,177;
re-reviewers 700 + 121 + 166 + 334 (132 + 202) = 1,321; test authors 395. These sum to
15,405k. Task 14's fix round was its own builder, so it is inside the builder's 355k; Task 16's fix rounds were fresh
agents.

By kind of task: code with a separate test author (Tasks 2, 3) 0.45M to 0.83M; CI and packaging (Tasks 4, 14) 0.75M to
1.07M; documents with a Sonnet builder (Tasks 5, 7) 0.1M+ to 0.49M+; research documents with an Opus builder (Tasks 6,
8, 15) 0.70M+ to 1.01M+; the gate report (Task 16, both stages and the final review) 3.69M; GPUI integration (Task 9) 0.80M+; trials
(Tasks 10 to 13) 0.84M+ to 2.2M+.

The plan's stop thresholds (500k per attempt for a code or document task, 1M for a trial) held, with one exception: no
code or document agent passed 500k except three of the gate's (the stage 1 drafter at 570k, the stage 2 cookbook builder at 563k and the final whole-branch reviewer at 503k, all Task 16); the largest of the others
was the threat model's author at 430k. Several trial agents passed 500k (the largest 614k, cumulative over three CI
rounds), none reached 1M.

### What the reviews caught

Each of these would otherwise have shipped (working notes; the first is also a note in `docs/build/LEDGER.md`):

- A builder's report claimed a change that had not applied (Task 2). Since then reports are checked against the diff.
- The privacy scan's commit range skipped a root commit's files (Task 2).
- The path guard let a path escape through a symlink after a `..`, a hole in the controller's own algorithm; and an
  empty or inherited test root fell back to the real user folders (Task 3).
- CI would have turned `main` red after every merge made in GitHub's interface; and the mutation check passed when
  mutants survived if any had also timed out, through a ruling error by the controller (Task 4).
- The licence check skipped GPUI's whole dependency tree, and unsound advisories failed only for direct dependencies
  although a commit said otherwise (Task 9).
- Soft checks that failed silently: the CI step that turns warnings into annotations swallowed its first warning
  (Task 10).
- A result at display scale 2 (34 of 34 input checks) that no log supported; the logs support it at scale 1 only. The
  same review found memory targets contradicted by the logs, the mock keychain unmentioned and the privacy cost of
  Chromium's background traffic unstated (Task 12).
- The Claude adapter's guard does not stop a subscription paying through an environment token, so the record's
  enforcement rule did not hold; and an OpenAI policy clause the record called absent (Task 8).
- The threat model's main exfiltration control let data go back to the injecting host, its sandbox model ignored
  escapes through other programs, and add-on agents load a repository's own configuration (Task 6).
- A public CI log printed a runner account's identifier and the record did not say so; the Wayland step could not
  fail its job (Task 13).
- The Electron start-up figure from a development binary was framed as the shipped app's, and "cold start" was measured
  with a warm cache (Task 15).
- The nightly workflow fetches two tools at build time without a hash check (an installer plugin and a disk-image
  script), and the record did not say so until the review; the record also called the identifier the permanent value
  on Windows although the product name keys the install folder, and one size in it cited the wrong CI job (Task 14).

### Where effort was wasted

- Two stops at the maintainer's usage limit. The second stopped three agents mid-run; one held 17 uncommitted files.
- Large contexts: several builders reached 400k to 600k tokens, so fix rounds went to fresh agents that had to read
  everything again (Tasks 6, 9, 10, 11, 12, 13, 15), and the browser trial's second phase started a fresh agent.
- Two controller errors cost a fix round each (Tasks 3 and 4).
- Faults in checks, not in the work, cost CI rounds: a count that ended a step under strict shell settings (Task 13),
  a sandbox check that mistook forked processes for their parent (Task 12), a missing newline before annotations
  (Task 10), a Windows path-length limit (Task 12), an installer started from a bash step on Windows that hung to the
  two-hour limit (Task 14).
- Hosted macOS runners were often not available, which meant re-runs and waiting (Tasks 10, 11, 12, 14).
- Index conflicts in `docs/decisions/README.md` when trials merged in a different order (Tasks 11, 12, 13).
- The advisory policy was asked twice on one day: a sixth notice appeared hours after five were accepted by name.

### Proposed budgets (the build team's proposal)

Per task, all roles together, as a hard stop that reports to the controller. Each figure is the highest recorded total
for that kind of task in M0 (the table above) plus about 20 %, rounded to the nearest 50k. The margin is there because
several recorded totals are lower bounds; it does not pay for an extra fix round, which cost 140k to 614k each in M0, so
a task that needs one stops and reports. These are caps, not forecasts: most M0 tasks of each kind used less.

| Task type | Budget | Per role | Highest recorded in M0, and the arithmetic |
|---|---|---|---|
| Well-specified code unit (Sonnet test author and builder, Opus reviewer): a provider client, a tool, a CLI verb | 1.0M | test author 150k, builder 300k with its fix rounds, reviewers 500k with re-reviews, 50k reserve | Task 2: 825k (3 fix rounds); Task 3: 445k. 825k x 1.2 = 990k, so 1.0M |
| Core code (Opus builder): engine loop, sandbox, permission cards, the GPUI chat window | 1.3M | builder 600k, reviewer 350k, fix agent 300k, 50k reserve | Tasks 9, 10, 11: 800k+, 1,098k+, 839k+. 1,098k x 1.2 = 1,318k, so 1.3M. This rests on trials (Tasks 10 and 11) as the proxy for core code, since M0 had no core-code task; the one non-trial task, Task 9, alone would give 800k x 1.2 = 960k, so 950k |
| Document (Sonnet) | 600k | builder 250k, reviewer 300k, 50k reserve | Task 7: 494k+ (Task 5: 101k+). 494k x 1.2 = 593k, so 600k |
| Research document (Opus) | 1.2M | builder 500k, reviewer 350k, fix agent 250k, 100k reserve | Tasks 6, 8, 15: 1,006k+, 697k+, 731k+. 1,006k x 1.2 = 1,207k, so 1.2M |
| CI or packaging | 1.3M | builders and test author 450k, reviewers with re-reviews 700k, 150k reserve | Task 4: 1,067k with a mutation-test sub-task. 1,067k x 1.2 = 1,280k, so 1.3M |
| Trial | 1.3M; 2.6M for a high-risk one | ordinary: builder 600k, reviewer 250k, fix agent 300k, re-reviewer 100k, 50k reserve; high-risk: two builder phases of 600k, reviewer 400k, fix agents 650k, re-reviewers 150k, 200k reserve | Task 10: 1,098k+ (Tasks 11, 13: 839k+, 475k+); Task 12: about 2,173k+. 1,098k x 1.2 = 1,318k, so 1.3M; 2,173k x 1.2 = 2,608k, so 2.6M |

With Task 14 included the rule gives the same figures. Its total is 749k, below Task 4's 1,067k, which stays the
highest recorded for CI or packaging, so that row is still 1.3M. Task 16 is left out of the rule: it cost
1,149 + 2,545 = 3,694k in all (stage 1, then stage 2 and the final review), above the document budget, because a gate
report reads every record, and stage 2 also built the cookbook; it is the only task of its kind. That suggests the 0.6M
the M1 estimate gives the gate report is low: this gate cost more than six times that.

(Written before the decision of 2026-11-17 that M1's nightlies are unsigned; no figure below is changed.)

**M1 as a whole: 22 million tokens**, with a check-in at 11 million. Reasoning, to be replaced by the M1 architect's
real task list: design 6 puts one hosted and one local provider, the engine loop, core tools, the shell sandbox,
permission cards, a session log, a GPUI chat window, `unidex chat`, the first evaluation run and signed macOS nightlies
in M1. Read as tasks: an architect pass (0.5M); seven core tasks, the engine loop, streaming into GPUI, the sandbox on
each of three platforms, permission cards and the chat window (7 x 1.3M = 9.1M); eight well-specified tasks, two
provider clients, file tools, shell and web tools, the session log, `unidex chat`, the mock provider with golden tests,
the evaluation harness (8 x 1.0M = 8.0M); two CI tasks, signed nightlies and the automation-socket harness (2 x 1.3M =
2.6M); two documents, the threat model update and the gate report (2 x 0.6M = 1.2M); and the gate's security review
(0.5M). Total 0.5 + 9.1 + 8.0 + 2.6 + 1.2 + 0.5 = 21.9M, called 22M. It is a cap: M0's sixteen tasks used about 15.4M
recorded (a lower bound), and the earlier draft of these figures gave 18.3M. If you prefer a lower cap, the earlier
figures (700k for a code unit, 400k for a document, 900k for a research document and for a CI task) are below the
highest recorded M0 totals by 125k, 94k, 106k and 167k respectively, so tasks as large as those would stop and report.
The 22M total is also short if the M1 gate costs what this one did (3,694k against the 0.6M
allowed): by about 3.1M. Part of that was the cookbook's code and the final whole-branch review, so a gate that adds no
cookbook would cost less.

### Pacing

The work twice ran into the maintainer's usage limit: once early on 2026-11-17 while the first document reviews ran,
and once while three trials ran together. After the first stop, large Opus agents ran one at a time and a re-review
waited for a fix round to finish rather than run beside it; during the trials, at most two heavy agents ran at once.
At the second stop, the controller stopped all three running agents so none was cut off mid-run, then resumed each with
its context. Fix rounds for agents above about 400k tokens went to fresh agents
with a findings file. The build team proposes the same pacing for M1.

## g. What the build team recommends happen next

**First, before M1 is planned** (items 1 to 3 were decided on 2026-11-17; item 4 was not done):
1. Question 1 (the Linux sandbox) and question 2 (the M1 proposals and conflicts). M1's sandbox and permission specs
   cannot be written without them.
2. Questions 3 and 4 (the reference machine and target definitions; the scroll target). Every M1 gate measurement
   depends on them.
3. Question 5 (packaging and identity). Decided: M1's nightlies are unsigned, so the identifier (part ii) is replaced
   before anything is signed and the signing identities (part v) wait for the first tagged preview (M4); the display
   name (part i) is settled.
4. The cookbook window and the trial package checks (section d): the plan's own sign-off step. Not done at sign-off;
   they stay owed, to be done before the first gate that depends on them.
5. Question 23 (the widget kit), before M1's chat window is built; M1's architect can plan around it (option C).

**What M1's plan needs from M0's findings, beyond those answers** (decisions 0008, 0009 and 0013): the chat's message
cache with eviction; GPUI's frame readback exists only with its test-support feature, so test builds compile GPUI
differently from release builds, and the check that release builds have no automation socket runs on nightly
artifacts; GPUI's accessibility tree has no bounds or element ids, so the app supplies them for UI tests; GPUI reports
Windows' software renderer as hardware, so software rendering is detected by adapter name; GPUI's default UI font is
missing on Linux, so the app names or ships its fonts; the real harness makes Wayland a hard check; the engine's own
child processes are waited on by process id (the terminal's signal handler); and a re-read of OpenAI's usage policies
after 2026-11-20 if M1's hosted provider is OpenAI (question 6).

**Whenever convenient, nothing waits on them:** questions 24 to 27. Question 26 (private vulnerability reporting) takes
minutes and makes `SECURITY.md` true; question 24 (a virtual machine) is best answered before the first gate with
hands-on Linux items.

**Then, before each later milestone is planned:** questions 13 and 6 to 8 (M2, M3, M5); 9 to 12 (M4); 14 to 16 (M6);
17 (M8); 18 to 22 (M7).

**Before the first signed build, and in any case before any release** (M1's nightlies are unsigned, so this now comes
with signing, decided at the first tagged preview, M4): the two unhashed build-time fetches in the packaging tool, an
installer plugin and a disk-image script, are fetched with a hash check or supplied from a verified copy, and where the
signing secrets live and who approves their use (threat model P38) is decided (decision 0014; question 5, part v). The
build team keeps the first on its list as a blocker; the second is not decided and moves to M4 with signing.

**What can proceed regardless:** the platform trial's 14 steps and the browser check from `docs/acceptance/hands-on/`
(section d; the first decides question 12, the second waits for your word on question 21); the M1 architect's crate
interfaces and event types (design 11.1); the mock provider, recorded HTTP fixtures and golden event-log tests (design
10.1); and the provider clients, which need no answer above except question 6's re-read.

### The first task of M1: harden the privacy gate and the path guard

The final review of M0 found no critical issue in the privacy gate or the path guard and left the items below. They
are recorded here, not fixed at this gate, and the M1 plan must contain each of them as its first task. Line numbers
refer to the code as this gate leaves it.

What the gate cannot see:

- UTF-16 content, in every scan mode (`xtask/src/privacy/scan.rs:162-164`).
- Host names beyond its one pattern: local network names, `user@host` prompts, IPv6 private and link-local addresses,
  hardware addresses (`xtask/src/privacy/rules.rs:74-75`).
- A home-folder path inside a URL's query string (`xtask/src/privacy/rules.rs:121-133`).
- A byte-order mark or a UTF-16 encoding in the private term list, which silently disables terms while the "list
  required" check still passes (`xtask/src/privacy/scan.rs:43-55`, `xtask/src/privacy/mod.rs:155`).
- Non-UTF-8 file names in the whole-tree mode (`xtask/src/privacy/git.rs:45-50`).
- Submodule path names (`xtask/src/privacy/git.rs:94-95`).
- An empty range, reported as "clean" without the number of commits scanned (`xtask/src/privacy/mod.rs:403-404`).
- Allow-list values compared on a truncated excerpt (`xtask/src/privacy/mod.rs:140-142`).

False positives M1 will hit:

- The home-folder rule flags relative path segments such as a `home` or `users` module directory; anchor it to the
  start of a path (`xtask/src/privacy/rules.rs:53-54`).

What design 12 promises and the gate does not do:

- No local secret scan: provider-key patterns in `xtask`, or a local run of the secret scanner, before M1 brings
  provider keys into the workflow. Today the secret scanner runs only in CI (`.github/workflows/ci.yml:337-344`), which
  on a public repository is after publication.

Coverage beyond the commit hooks:

- A pre-push hook that scans the range being pushed, which covers rebases, cherry-picks, merges, bypassed hooks and
  commits made before the hooks were installed (`xtask/src/hooks.rs:44-50`).
- CI scanning pull request titles and bodies (`.github/workflows/ci.yml:270-344`).
- CI judging a pull request with the default branch's gate and allow file, so a pull request cannot weaken its own
  check (`.github/workflows/ci.yml:306-336`).
- Required review, through a code-owners file, on the gate, the allow file, the workflows, `deny.toml` and `.cargo/`
  (the repository has no such file).

The hook installer:

- With a global hooks path set, it writes the hooks there, and they then refuse commits in other repositories
  (`xtask/src/hooks.rs:24-38`).
- An existing hook that cannot be read, or is not UTF-8, is overwritten (`xtask/src/hooks.rs:51-60`).

The path guard:

- Tests for a relative override and for a missing root (`crates/udx-core/tests/paths.rs`).
- The error reported for some absolute-path failures (`crates/udx-core/src/paths.rs:73`, `:77` and `:96`).
- Values that are only whitespace (`crates/udx-core/src/paths.rs:63`).
- A notice when a test is skipped because it runs as root (`crates/udx-core/tests/paths.rs:316-323`).
- What design 10.1 asks beyond the guard, a per-test guard on every resolved path: `udx-core`'s path resolution as the
  single choke point for every user-folder path; the test environment helper required in tests (a test binary started
  outside cargo has neither variable); and the scheduler refusing to register an operating-system wake-up from a
  process that has a test root (`crates/udx-core/src/paths.rs`, `crates/udx-test-support/src/lib.rs`).

Architecture notes for M1's plan:

- Per-crate test helpers instead of one test-support crate that would depend on the crates it helps test, a
  dev-dependency cycle (`crates/udx-test-support`).
- Package a binary target, not an example: examples build with dev-dependencies, so a test-only GPUI feature could
  ship (`packaging/Packager.toml:28-30`).
- Avoid a direct dependency on `chrono`, or decide it explicitly: under the advisory scope rule it would fail the
  licence and advisory check (`deny.toml:39`; decision 0007).
- The nightly's concurrency group lets a scheduled run and a manual run cancel each other
  (`.github/workflows/nightly.yml:41-43`).
- The macOS cookbook step recompiles the platform crates, which costs CI time (`.github/workflows/ci.yml:137-143`).
- Test vectors that use real domains move to reserved ones (`xtask/tests/`).
