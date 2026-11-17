# UniDeX Rust Preview: design

Status: approved by the maintainer · 2026-11-16 · draft 2, revised after an independent design review · revised at
the M0 gate, 2026-11-17: sections 5.7, 5.8, 8, 9, 13 updated from decisions 0006 to 0014, and section 7 corrected to
match the acceptance checklist; open questions in docs/build/M0-GATE.md
Source product: UniDeX Agent Harness 2.7.0 (Electron), MIT, referred to below as "the Electron app".

## 1. Intent

Build a new UniDeX from scratch in Rust, in this repository. It must be faster and smoother than the Electron app, run on
macOS, Linux and Windows, and work with any model: hosted frontier models, local models, or a mix. It must not depend on
any vendor's command-line tool or login to function.

Why a rewrite and not a conversion: in the Electron app every agent turn that uses tools is the `claude` or `codex` CLI
run as a child process. About half of its Node runtime adapts to those two tools (hooks, transcript formats, plugin
folders). Its UI is about 54,000 lines of hand-written DOM code on a stand-in for the Obsidian API, with blocking calls
inside render paths and several 300 to 500 ms polling loops. None of that translates. What carries over is the product:
its features, its data formats, its design tokens, its security lessons and its acceptance checklist.

## 2. Goals, non-goals, anti-goals

### Goals

- G1. An agent engine of our own: tool calling, permissions, sandboxing, context management, streaming, sub-agents.
- G2. Any model. Hosted (Anthropic, OpenAI, Google, any OpenAI-compatible endpoint) and local (Ollama, llama.cpp server,
  LM Studio, vLLM), assignable per role so one setup can mix them.
- G3. Engine quality is measured, not assumed: an evaluation suite runs across a matrix of models (section 10.2).
- G4. Claude Code, Codex and other agents usable as optional add-on engines through the Agent Client Protocol (ACP).
  Removing them breaks nothing. Open at the M0 gate: see `docs/build/M0-GATE.md`, question 6.
- G5. A native Rust UI on GPUI. No web view for the app's own screens.
- G6. Chromium embedded inside the app window for the Computer screen (section 5.8).
- G7. One codebase for macOS, Linux and Windows. A feature works on all three or is listed as an exception in section 8.
- G8. Measurable speed (section 9).
- G9. Feature parity with the Electron app 2.7.0 by the last milestone, with every drop or change listed (section 7).
- G10. Every screen fully usable from the keyboard. Screen-reader support is a listed exception until GPUI provides it
  (section 8). (Design call.)
- G11. Built and tested by orchestrated Opus and Sonnet agents (section 11).
- G12. No personal information in the repository, ever (section 12).

### Non-goals

- Replacing the Electron app or migrating its users. It stays the stable line.
- Plugins, hooks or status lines installed into Claude Code or Codex. The Electron app keeps that role. (Design call.)
- Mobile, web or hosted multi-user versions.
- Sharing a live vault with the Electron app. (Decided by the maintainer; section 4.3.)
- An always-running background daemon (section 5.9).
- Parsing past Claude Code or Codex transcripts, or the Electron app's session records. History covers sessions run in
  this app, native or add-on. (Design call.)
- Accepting the Electron app's `aos` command verbs. `unidex` has its own. (Design call.)

### Anti-goals (a failure even if it works)

- Needing a particular vendor's CLI, account or model to start or to run a turn.
- Input or output on the UI thread, or any polling loop where an event exists.
- Telemetry or any network call the user did not configure.
- Code translated line by line from the Electron app's host-adaptation layer.
- A real user's data touched by a test.
- Turning off Chromium's sandbox, or any other protection, to make a package or a test work.
- Code copied from a source whose licence does not allow it (section 11.4).

## 3. Constraints

- Public repository, MIT licence. Dependencies must be compatible (checked by `cargo-deny`).
- Rust stable. GPUI is pre-1.0 and is consumed as a snapshot of a specific upstream commit. At review time the widget
  kit is published as `gpui-kit` and pins such a snapshot. M0 records the exact GPUI source, the pinned versions and the
  upgrade procedure; after that an upgrade is its own change, never mixed with feature work.
- The `agent-client-protocol` crate is moving fast (several major versions in 2026). Pin a major version and run a
  conformance test for each pin.
- Frontier-model subscriptions are assumed usable only through the vendor's own tools, so the own engine uses API keys
  or local models, and subscriptions are reached through add-on engines (G4). The vendor terms for each add-on are
  checked and written down in M0, not later.
- Local builds are assumed to run on a 16 GB machine. A GPUI workspace is heavy to compile (section 11.3).
- No Linux or Windows machine is available for hands-on checks (section 8).
- The Rust toolchain is not yet installed on the development machine. Installing it needs the maintainer's approval.

## 4. Approaches

### 4.1 Engine (decided by the maintainer)

| Option | Trade-off |
|---|---|
| **Own engine, CLIs as optional add-ons (chosen)** | Independent by default; subscriptions and cross-review between Claude Code and Codex still work; costs one ACP client. |
| Own engine only | Cleanest; frontier models by API key only; loses cross-review on subscriptions. |
| Keep driving the CLIs, add an own engine beside them | Not independent. Rejected. |

### 4.2 App shell and UI (decided by the maintainer)

| Option | Trade-off |
|---|---|
| **Native Rust UI on GPUI (chosen)** | Rust throughout, GPU-drawn, largest smoothness gain; every screen is rebuilt; framework is pre-1.0. |
| Tauri shell with the existing web UI adapted | Fastest to parity and lowest risk; screens stay TypeScript in a web view. |
| egui, Iced, Slint | Weaker fit for a text-heavy app. Rejected. |
| Rust compiled to WebAssembly in a web view | Costs of both columns. Rejected. |
| Rust core swapped under the Electron app | Not a new application. Rejected. |

### 4.3 Data (decided by the maintainer)

| Option | Trade-off |
|---|---|
| **Own vault with a repeatable, read-only import (chosen)** | Independent in data as well as models; preview software never writes to the real vault. |
| Share the Electron app's live vault | One memory from day one, but duties fire twice, state sits in the other app's formats, and the tie is permanent. |

### 4.4 Browser (embedding decided by the maintainer; method is a design call, confirmed in M0)

| Option | Trade-off |
|---|---|
| **Chromium rendered off-screen in a helper process, drawn as an element in the app window (chosen)** | App UI can draw over the page; web content stays out of the app's process; input, text entry, cursors and popups must be forwarded by hand. |
| Chromium as a native child view | Less forwarding work, but it sits above everything GPUI draws, so ask cards, popovers and the command palette cannot appear over a page. |
| The system web view | Not Chromium on macOS or Linux, and cannot be driven the way the `computer_*` tools need. Rejected. |
| An external browser window | Simplest, but not embedded. Kept only as the fallback for a platform where embedding fails. |

The core has no UI code in it, so the shell choice stays reversible.

## 5. Architecture

One Cargo workspace. Each crate has a single purpose and a published interface; crates above depend only on crates
below.

```
udx-app        GPUI application: shell, screens, theme, automation socket (test builds only)
udx-ui-model   screen state and actions, with no GPUI types
udx-cli        `unidex` binary: every capability, headless, with --json
------------------------------------------------------------------
udx-persona    Chief of Staff: identity, state, duties, proposals
udx-scheduler  routines and duties; OS wake-up hooks
udx-browser    embedded Chromium and its control protocol
udx-term       PTY, terminal emulation, terminal model
udx-acp        add-on engines over the Agent Client Protocol
udx-engine     the agent loop, permissions, context, sessions
udx-sandbox    OS-level confinement for commands the engine runs
udx-tools      built-in tools
udx-mcp        MCP client and MCP server
udx-providers  model providers, model profiles and role routing
udx-store      vault, memory, index, file watching, import
udx-core       types, events, config, paths, errors
------------------------------------------------------------------
udx-evals          agent tasks run across a model matrix
udx-test-support   mock provider, fake ACP agent, temp vaults
udx-cookbook       small GPUI examples that CI builds; the agents' reference
xtask              privacy gate, packaging, release tasks
```

### 5.1 Two traits that define the product

- `Provider`: streaming chat with tool calls, a model catalog with capabilities (tool calling, vision, context size),
  and token usage. Implemented by Anthropic, OpenAI, Google, a generic OpenAI-compatible client, and Ollama.
- `Engine`: "something that runs a turn in a workspace and emits events". Implemented by the native engine
  (`udx-engine`, which uses a `Provider`) and by the ACP engine (`udx-acp`, which drives an external agent). Sessions,
  the UI and the CLI see only `Engine` events, so a thread can be served by either kind.

Roles (`main`, `background`, `reviewer`, `embeddings`) each map to a provider and model, with an ordered fallback list.
That mapping is how a setup mixes frontier and local models. A fallback happens only between turns, never inside one.

### 5.2 Native engine

- Turn loop with parallel tool calls, cancellation and resumable sessions.
- Session log: one append-only, versioned JSONL event file per thread, in our own schema, under `.unidex/sessions/`.
- `unidex chat` and `unidex run` work with no vault at all; memory features are then simply off.
- Permissions: allow, ask or deny per tool, path and command, with modes; every decision is logged. In an unattended
  run (a routine, a duty, `run-due`) an "ask" is a "deny".
- Internal events replace host hooks: context injection before a turn, memory capture after it, spend accounting.
- Spend ledger with per-call and per-day caps.

Making one loop work well across very different models is the hard part, and it is designed in, not patched on:

- **Model profiles.** Each model family gets a profile: which edit format it handles, whether it needs literal search in
  place of regular expressions, how many tools it can juggle, its prompt style. The Electron app's local-model loop
  already learned some of this and is the starting reference.
- **Tool calling without native support.** A model with no tool-calling API gets a prompt-based protocol. Chat-only is
  the last resort, and the UI says which mode a model is in.
- **Schema sanitising.** Tool definitions are adjusted per provider, because each accepts a different subset.
- **Reasoning state.** Opaque reasoning blocks a provider returns are kept and sent back unchanged.
- **Cache-friendly context.** Injected context goes where it does not invalidate a provider's prompt cache.
- **Context management.** Token accounting per provider; trimming and summarising when a window fills. For local
  servers the engine sets the context size it needs and checks that the prompt was not silently cut.
- **Loop guards.** Repeated identical calls, runaway turn counts and invalid-call streaks stop the turn with a clear
  message.
- **Retries.** Rate limits and transient errors back off and retry without duplicating a tool's side effects.

### 5.3 Tools

Read, write and edit files; glob; grep; shell; web fetch; todo; sub-agent; skill; memory; notify; plus any tool an MCP
server provides. File tools are scoped to the workspace.

Skills use the open `SKILL.md` format: listed by name and description, loaded on demand, and offered as slash commands.
A sub-agent definition is a markdown file with frontmatter (name, description, allowed tools, role). Both live in the
vault and in a per-user folder, and can be imported from other tools' folders but are never mirrored back.

### 5.4 Add-on engines (ACP)

`udx-acp` is an ACP client built on the official `agent-client-protocol` crate. It spawns an agent's ACP adapter, maps
its permission requests to our permission UI and its updates to `Engine` events. Claude Code and Codex are the first
two; any ACP agent should work. Cross-review and handoff are features on top of `Engine`: the reviewer is simply a
different engine or role from the builder.

- ACP lets an agent ask the client to read files and run terminals. Those requests pass through the same permissions
  and sandbox as the native engine's tools.
- The app never bundles or installs an add-on or a runtime for one. It detects an installed agent and its adapter, shows
  the status and the exact install steps, and runs an adapter only from the path it found or the user set.
- An app started from a dock or launcher does not inherit the shell's `PATH`. The app resolves it from the login shell
  once at start, as the Electron app learned to.
- If a vendor's terms rule its agent out, that agent is left off the list; the ACP engine ships regardless.

### 5.5 Store

The Rust app has its own vault. It does not open the Electron app's vault in place.

- Markdown is the source of truth and keeps the Electron app's formats and folder layout for what a person reads and
  edits: the `MEMORY.md` index, `brain/memory/<type>/*.md` with frontmatter, patterns, daily notes, notifications,
  persona files and `workspaces/<name>/workspace.md`. Import is therefore a copy, with nothing converted.
- Everything else is ours and lives in the vault's `.unidex/` folder: a format version, configuration (`config.toml`),
  the session bootstrap and working memory, routine and duty run state, session logs, the import manifest and the index.
  The index is a rebuildable cache (SQLite: full-text recall, embeddings, session index) and is never the only copy of
  anything. It records which embedding model and size produced it and rebuilds when they change.
- A per-machine file in the OS config folder records which vault is in use. The Electron app's settings are not read.
- A new vault is created from our own template.
- `unidex import <path>` reads an existing vault and never writes to it. It copies memory, patterns, notes, daily notes
  and persona identity; it records each workspace by its folder path instead of copying project folders; it brings over
  prompt routines and lists, without importing, any routine that calls the Electron app's runtime. It can be re-run. The
  manifest holds a hash of every file as last imported, which is how a re-run tells new, changed and conflicting files
  apart. Nothing is deleted, and conflicts are reported for the user to settle.
- Changes on disk arrive through file watching, not polling. Writes are atomic.

### 5.6 MCP

`udx-mcp` uses the official Rust SDK (`rmcp`). As a client it connects the engine to MCP servers (the Connectors
screen). As a server (`unidex serve`) it exposes memory, recall and session tools so an outside agent can use the vault.
The server speaks over standard input and output only by default; any network transport requires a token.

### 5.7 Terminals

A PTY layer (ConPTY on Windows), `alacritty_terminal` for emulation, and our own GPUI element that draws the grid. The
PTY is `alacritty_terminal`'s own `tty` module and event loop on all three platforms, pinned exactly with the emulator,
behind a session type of our own in `udx-term` so the backend can be replaced; `portable-pty` is the fallback (decision
0009; design call). Each terminal gets an I/O thread owned by `alacritty_terminal`, whose events the core forwards to
GPUI. On Unix that crate installs a process-wide SIGCHLD handler, so the engine's own child processes are waited on by
their process ids, and dropping a session blocks until the child is reaped, so it happens off the UI thread. In the M0
trial the grid element drew a 200,000-line burst on macOS with no slow frame (decision 0008). Two checks are open on
Linux and Windows: see `docs/build/M0-GATE.md`, question 9. The "chat over a live terminal" view of the Electron app is
not rebuilt: add-on engines speak ACP, so their turns are structured events already.

### 5.8 Embedded browser

The Computer screen shows Chromium inside the app window and offers the same `computer_*` tools as the Electron app.

- A `BrowserView` trait hides the engine. The first implementation is CEF (the Chromium Embedded Framework) rendering
  off-screen in a helper process. The app draws each frame as an element, forwards pointer, keyboard, text entry,
  cursor and popup events, and drives pages over the Chrome DevTools Protocol. The M0 trial settled how frames travel
  (decision 0012; design call): the app drives the helper over the helper's inherited standard input and output, the
  helper writes each frame as CPU pixels into shared memory that the app creates and the helper inherits, and the app
  draws the newest frame as a GPUI image. In the trial DevTools ran through CEF's in-process interface, with no port.
  On macOS this showed real Chromium in a GPUI window at about 60 frames a second, with typing, input methods and an
  app card drawn above a live page (scripted checks on the development Mac; the maintainer's hands-on check is owed).
- The helper process keeps web content out of the app's own process, survives a page crash, and avoids the conflict
  between CEF's and GPUI's application objects on macOS.
- Chromium is large. It is not in the installer: it is fetched the first time the Computer screen is opened, as a
  version-pinned component checked against a known hash. The app works fully without it. Open at the M0 gate: see
  `docs/build/M0-GATE.md`, questions 18 and 19.
- Chromium's own sandbox stays on, on every platform. A package format that cannot run it (AppImage has known trouble
  here) does not ship the browser. Open at the M0 gate: see `docs/build/M0-GATE.md`, questions 19, 20 and 22.
- Open at the M0 gate: in the trial Chromium contacted its vendor's servers by itself; see `docs/build/M0-GATE.md`,
  question 21.
- A bundled browser must be kept current. The app shows the engine version, warns when it is more than 60 days old, and
  browser updates follow the release policy written in M4. CEF follows Chromium's stable releases about every four
  weeks, so each update is a recurring release task: new pins and hashes, a rebuilt and signed helper on every platform,
  and the checks run again (decision 0012).
- On Windows the M0 trial chose CEF as well, run by CEF's bootstrap executable with the helper built as a library, the
  arrangement Chromium's Windows sandbox needs. The evidence is headless CI runs under an elevated account, with nothing
  drawn and no input tried (decision 0012; see `docs/build/M0-GATE.md`, question 20). The OS-provided web view can sit
  under app-drawn UI only after a GPUI change, so it is a fallback before an external window, not the first choice
  (decision 0012; design call).
- If embedding fails on a platform in the M0 trial, that platform falls back to an external browser window and is
  listed as an exception in section 8. The other platforms are not held back.

### 5.9 Scheduling without a daemon

Routines and duties run in the app while it is open. For when it is closed, `udx-scheduler` registers one OS wake-up
(launchd, a systemd user timer, Windows Task Scheduler) that runs `unidex run-due`. App and CLI share the store through
file locks and SQLite's write-ahead log. Only this app's own vault is scheduled, so a routine can never fire in both
this app and the Electron app.

### 5.10 Application

- GPUI with the pinned widget kit; the Electron app's `--udx-*` tokens become a Rust theme, light and dark. Open at the
  M0 gate: see `docs/build/M0-GATE.md`, question 23.
- Screen state and actions live in `udx-ui-model`, which has no GPUI types. Screens in `udx-app` render that state and
  send actions. Most screen behaviour can therefore be tested on every platform without drawing anything, and less of
  the app is tied to the framework.
- The core runs on Tokio; the app forwards core event streams into GPUI. The UI thread never waits on I/O.
- Long lists (a thread with thousands of messages) are virtualised, and streamed markdown is parsed incrementally.
- GPUI has no tray support, so the tray popover uses a separate crate; the trial in M0 covers it on each platform.
- `unidex://` links, multiple windows, and a quit gate that refuses to quit over unfinished work carry over.
- Secrets live in the OS keychain, with environment variables as a fallback where no keychain exists.

### 5.11 Security

The Electron app leaned on Claude Code's permissions and Codex's sandbox and still needed about 370 lines of documented
controls. This app executes model-chosen tools itself, so it owns all of that. A threat model is written in M0 and
`SECURITY.md` is kept current; each milestone gate includes a security review.

| Boundary | Controls |
|---|---|
| Model output to tools | Permissions (5.2). Shell commands run inside `udx-sandbox` (seatbelt on macOS, Landlock and seccomp on Linux, a restricted token on Windows), delivered in M1 and informed by existing permissively licensed sandbox code. |
| Files | Workspace scoping; a deny list for credential and key files; paths resolved before checks, covering letter case, Unicode forms, symlinks, hard links and Windows junctions. |
| Untrusted content | Web pages, MCP results, browser content and files from outside the workspace are labelled as untrusted in the model's context. |
| Memory | Memory is injected into every turn, so a poisoned memory persists. A memory write from a turn that touched untrusted content becomes a draft for review, not a saved memory. |
| Secrets | Keychain storage; values redacted before anything reaches a session log; API keys removed from the environment of every child process. |
| Network | `web_fetch` refuses loopback, link-local and private addresses unless the user allows them. |
| MCP servers | Treated as untrusted: per-server approval of tools; results labelled. |
| ACP agents | Their file and terminal requests pass through our permissions and sandbox (5.4). |
| Browser | Helper process, Chromium sandbox on, dedicated profile, and the Electron app's ask gating for sensitive actions. |
| Supply chain and updates | Committed lockfile, `cargo-deny`, `cargo-audit`, pinned CI actions, signed updates. |

## 6. Milestones

Each milestone ends with a gate (section 10.4) and the maintainer's sign-off before the next one starts. Risk is taken
early: every unproven technology is tried in M0, and a real window exists from M1.

| | Milestone | Delivers |
|---|---|---|
| M0 | Foundations, trials and decisions | Workspace skeleton; CI on three platforms; privacy gate; threat model; the acceptance checklist ported and section 7 completed against it; GPUI source and pins recorded; add-on vendor terms recorded; packaging tool chosen. Throwaway trials on all three platforms: a streaming chat view and terminal; text entry including CJK input and dead keys; tray, menus, multiple windows, file dialogs; a screen reader; Chromium embedded in a GPUI window; whether CI runners, and virtual machines on the development machine, can render the app. Baseline measurements of the Electron app. The cookbook crate, task ledger and decision log. |
| M1 | Vertical slice | One hosted and one local provider, the engine loop, core tools, the shell sandbox, permission cards and a session log, in a real GPUI chat window and in `unidex chat`. The first evaluation run. Nightly builds, signed on macOS. |
| M2 | Engine breadth | All providers, model profiles, context management, sub-agents, skills, MCP client. Gate: the evaluation suite across the model matrix. |
| M3 | Memory | Store, recall, context injection, session wrap-up, feedback rules, `unidex import`, MCP server. |
| M4 | The app | Shell (rail, command palette, status bar, settings), Sessions and History, terminal deck, diff pane, themes, tray, links, local-model management, auto-update. First tagged preview. |
| M5 | Add-on engines | ACP engine, Claude Code and Codex, cross-review, handoff. |
| M6 | Chief of Staff | Persona, duties, routines, notifications, proposals, Pulse. |
| M7 | Browser | Embedded Chromium and the Computer screen. |
| M8 | Workspace screens | Spaces, Files, To-Do, Runs. |
| M9 | Knowledge screens | Brain hub including the Map, and the notes editor. |
| M10 | Agent screens | Agents, Agent Teams, Skills, Connectors. |
| M11 | Release | Installers for every platform, onboarding, docs, a full acceptance sweep. |

## 7. Parity with the Electron app

`docs/acceptance/CHECKLIST.md` decides the fate of each of the Electron app's 346 acceptance items; the table below
sums it per source section.

Design calls that change or drop features:

- The setup wizard that downloads Node, uv and the CLIs is dropped: the app is one binary, and onboarding offers a
  provider, a key or a local model (M11).
- Host plugins, hooks, the status line and edits to host instruction files are dropped (non-goal; none).
- The "chat over a live terminal" view is replaced by ACP sessions (M5).
- Cost tracking comes from provider usage data, not from host transcripts (M2).
- The Brain Map is drawn from links our own index finds; the external graph builder is not carried (M9). (Design call.)
- Agents, skills and teams are UniDeX's own; they can be imported from other tools' folders and are never mirrored
  back (M10).
- Connectors are MCP servers for our engine (M10).
- The Computer pane is embedded Chromium (M7).
- The tray popover, command palette, quick capture and tour carry over (M4 to M11).
- Chief of Staff: carried over; runs on any role's model (M6).

| source section                                         | items | carry | change | drop | milestones         |
|--------------------------------------------------------|-------|-------|--------|------|--------------------|
| Install paths                                          | 3     | 1     | 2      | 0    | M4, M5, M6         |
| The app                                                | 12    | 4     | 3      | 5    | M4, M11            |
| Files                                                  | 27    | 25    | 2      | 0    | M8                 |
| Settings                                               | 16    | 4     | 10     | 2    | M4, M5, M6, M10    |
| Config (both hosts)                                    | 5     | 0     | 3      | 2    | M1, M6             |
| Pulse                                                  | 17    | 9     | 7      | 1    | M4, M6, M9         |
| To-Do                                                  | 8     | 8     | 0      | 0    | M8                 |
| Proposals                                              | 9     | 6     | 3      | 0    | M6                 |
| Notifications                                          | 9     | 7     | 2      | 0    | M6                 |
| Agent teams (both hosts)                               | 4     | 1     | 3      | 0    | M10                |
| Agent Teams tab (both hosts)                           | 11    | 8     | 3      | 0    | M10                |
| Status line (both hosts)                               | 7     | 1     | 1      | 5    | M4, M10            |
| Spaces / Runs                                          | 23    | 13    | 9      | 1    | M8                 |
| Brain                                                  | 28    | 20    | 7      | 1    | M9                 |
| Routines                                               | 11    | 6     | 3      | 2    | M6                 |
| Skills (both hosts)                                    | 10    | 2     | 3      | 5    | M10                |
| Connectors (both hosts)                                | 17    | 4     | 6      | 7    | M10                |
| Agents (both hosts)                                    | 8     | 0     | 3      | 5    | M10                |
| Cross-review and handoff (both hosts; each run spends) | 3     | 0     | 3      | 0    | M5                 |
| Codex host                                             | 22    | 0     | 8      | 14   | M3, M6, M8, M9     |
| Vault chat (per provider)                              | 6     | 0     | 6      | 0    | M4                 |
| Sessions (per host; each turn spends)                  | 17    | 7     | 10     | 0    | M4, M5, M7         |
| Code                                                   | 27    | 12    | 10     | 5    | M4, M5, M8, M10    |
| Computer pane (per host; each turn spends)             | 29    | 20    | 7      | 2    | M7                 |
| Launch sequence                                        | 6     | 0     | 4      | 2    | M6, M11            |
| Guided tour                                            | 6     | 5     | 1      | 0    | M11                |
| Telemetry switch                                       | 2     | 0     | 0      | 2    | none               |
| Review readiness                                       | 3     | 1     | 2      | 0    | M4, M6             |
| **total**                                              | 346   | 164   | 121    | 61   |                    |

## 8. Platforms

Support is stated in tiers, so a claim never outruns its evidence.

- **Tier 1, macOS (Apple silicon):** hands-on check by the maintainer at every gate.
- **Tier 2, Linux (x64) and Windows (x64):** verified by CI only. A build is labelled "CI-verified, not yet run on real
  hardware" until a tester confirms it. Intel Macs, ARM Linux and ARM Windows come later. (Design call.)

What "CI-verified" means is made as strong as it can be without hardware: the build, the unit tests, the view-model
tests, and end-to-end tests that drive the real app through its automation socket (section 10.3). M0 also checks whether
virtual machines on the development machine can render the app; if they can, they become a hands-on step for Tier 2.
Open at the M0 gate: see `docs/build/M0-GATE.md`, question 24 (virtual machines were researched, not run).

The M0 trial showed what "CI-verified" covers on the hosted runners, which have no GPU, and the label claims this and no
more (decision 0013; design call):

- **Linux (x64):** the app builds and its tests pass; its window opens and renders on X11 under a virtual display
  through both of GPUI's Linux paths, Vulkan and OpenGL, on Mesa's software rasterisers; a frame is captured and
  checked automatically for the expected colours, layout and the presence of text; the automation socket clicks a
  control, reads GPUI's accessibility tree and the app's element list, and captures a frame (on X11). Wayland rendered
  under a headless compositor in the trial, but that check could not fail the job, so Wayland is not stated as checked
  until the real harness makes it a hard check.
- **Windows (x64):** the app builds and its tests pass; its window renders on a Windows Server 2025 interactive session
  with a software Direct3D adapter (Direct3D 11 on Microsoft's software rasteriser, WARP); the composed window on
  screen is captured and checked; the automation socket, over a named pipe whose access list names only the current
  user (not verified against a second local user), clicks a control, reads GPUI's accessibility tree and captures a
  frame.
- **Not covered on either:** real GPU drivers; real desktop environments and window managers, and on Windows the
  client editions' shell; scaling other than 100 percent and multiple monitors; input methods and dead keys; input
  through the operating system rather than inside GPUI; real font configuration; the tray, notifications, file
  dialogs and portals; the screen readers themselves; performance and frame pacing; ARM.

| Concern | macOS | Linux | Windows |
|---|---|---|---|
| Rendering | GPUI | GPUI (X11 and Wayland) | GPUI |
| PTY | `alacritty_terminal`'s own PTY (decision 0009) | `alacritty_terminal`'s own PTY (decision 0009) | ConPTY, through `alacritty_terminal` (decision 0009) |
| Secrets | Keychain | Secret Service | Credential Manager |
| Wake-up for routines | launchd | systemd user timer | Task Scheduler |
| Tray | `tray-icon` (decision 0011) | separate crate; the backend is open at the M0 gate (`docs/build/M0-GATE.md`, question 11) | `tray-icon` (decision 0011); chosen, never run on Windows |
| Browser | CEF, off-screen (decision 0012) | CEF, off-screen; open at the M0 gate (`docs/build/M0-GATE.md`, question 19) | CEF, off-screen, under CEF's bootstrap; the OS web view as a fallback (decision 0012); chosen on headless CI evidence, nothing drawn (`docs/build/M0-GATE.md`, question 20) |
| Package | signed, notarized `.dmg`, built with cargo-packager (decision 0014, proposed) | `.deb` (cargo-packager, then cargo-deb once the browser's setuid helper arrives; decision 0014) and a tarball made separately; AppImage only without the browser; open at the M0 gate (`docs/build/M0-GATE.md`, question 19) | NSIS installer built with cargo-packager (decision 0014), signed if a certificate is supplied |

Known exceptions at design time:

- Screen readers: GPUI's support is unconfirmed. M0 tests it; until it works, this is an exception on all platforms.
  Open at the M0 gate: see `docs/build/M0-GATE.md`, question 12.
- Screenshot-based UI tests run on macOS first; GPUI's visual testing elsewhere is still in flux.
- Signing identities are supplied by the maintainer as CI secrets. Without them a platform's builds are unsigned and
  say so.
- The packaging tool is cargo-packager for the `.dmg`, the Windows installer and the `.deb`; the tarball is made
  separately; the `.deb` moves to cargo-deb when the browser's setuid helper arrives (decision 0014, proposed). The
  application identifier, display name, publisher, the nightly schedule and the signing identities are open at the M0
  gate: see `docs/build/M0-GATE.md`, question 5.
- Open at the M0 gate: further exceptions proposed by the M0 trials and the threat model; see
  `docs/build/M0-GATE.md`, questions 1, 2, 10 and 11.

## 9. Performance targets

Initial targets, to be confirmed against the Electron baseline measured in M0 (`docs/baseline/electron-2.7.0.md`, which
also proposes how each target is defined and measured). They are measured in two places, because shared CI runners have
no GPU and cannot time frames reliably:

- In CI, on every change: benchmarks of the core, download size, and a count of idle wake-ups. A regression fails the
  change.
- At each milestone gate, on the reference machine (an Apple silicon laptop with 16 GB): cold start, frame pacing and
  token latency. A miss blocks the gate. M0's measurements, the Electron baseline and the trials, were taken on an
  Apple silicon desktop with 16 GB and external displays at 100 Hz and 60 Hz, not on that laptop.

Open at the M0 gate: which machine is the reference, and how each target is defined; see `docs/build/M0-GATE.md`,
question 3.

The app, without the browser component:

- Cold start to first frame: under 400 ms.
- Idle CPU: no periodic wake-ups from our code. Idle memory: under 150 MB.
- Scrolling a 5,000-message thread: no dropped frames at the display's refresh rate. Open at the M0 gate: see
  `docs/build/M0-GATE.md`, question 4.
- First streamed token on screen: within 50 ms of arrival.
- Download size: under 40 MB.

The browser component is measured on its own (download size, memory per tab, frame rate of the embedded view), with
targets set from the M0 trial. Its memory does not count against the app's 150 MB. Open at the M0 gate: see
`docs/build/M0-GATE.md`, question 18.

## 10. Testing

### 10.1 Every change

- Unit tests in every crate. `cargo fmt`, `clippy -D warnings`, `cargo-deny`, `cargo-audit` and the privacy gate.
- Engine: a scripted mock provider and golden event-log tests; property tests for context trimming.
- Providers: one conformance suite run against every provider client using recorded HTTP fixtures. Live tests against
  real services are opt-in and never run on forks.
- ACP: a fake agent binary, and a conformance test per pinned protocol version.
- Store: temporary vaults only. A guard fails any test that resolves a path outside its temp folder.
- Screens: view-model tests on all platforms; `#[gpui::test]` view tests; screenshot baselines on macOS. Every pull
  request that changes a screen attaches a screenshot.
- Core crates are also checked by mutation testing, so a test suite that passes but asserts nothing is caught.

### 10.2 Engine quality

`udx-evals` holds 40 to 60 agent tasks (edit a file, fix a failing test, a multi-step change, a refusal case). It runs
them across a model matrix that includes frontier models and a small local model, and records pass rate, invalid tool
calls, turns and cost. Claude Code and Codex over ACP are the reference lines. The M2 gate requires agreed pass rates
per model class, and the results are published as a model compatibility table.

### 10.3 End to end

- Through the CLI, for everything the core does.
- Through the app, on all three CI platforms: test builds open an automation socket that can send actions, read the
  element tree and capture a frame. Release builds do not contain it. This replaces the role Playwright had for the
  Electron app, where it drove the real UI for almost every checklist item.

### 10.4 Milestone gate

CI green on three platforms; performance targets; security review; the milestone's acceptance items (ported from the
Electron app's checklist, each marked automated, manual or not applicable); then a hands-on check by the maintainer on
macOS.

## 11. How the agents build it

One orchestrating session holds the plan, dispatches work and merges, using sub-agents and scripted multi-agent
workflows. Nothing merges unreviewed.

### 11.1 Roles

| Role | Model | Work |
|---|---|---|
| Architect | Opus | Before a milestone: crate interfaces, event types and task specs with done-criteria. |
| Core builder | Opus | The novel parts: engine loop, context management, permissions, sandbox, GPUI shell foundations, terminal element, browser embedding, ACP mapping. |
| Builder | Sonnet | Well-specified units against a contract: provider clients, tools, store modules, CLI verbs, individual screens, docs. |
| Test author | Sonnet | Writes failing tests from the contract before the builder starts. Never the same agent as the builder. |
| Reviewer | Opus | Reviews each diff against its spec without the builder's summary. Security review at each gate. |

Opus reviewing Opus shares blind spots. Three things offset that: the separate test author, mutation testing, and the
evaluation suite, none of which depend on a reviewer's judgement. A review by a second provider at each milestone gate
is available if the maintainer turns it on.

### 11.2 Rules

- One task is one branch in its own worktree and one pull request, small enough to review in one read.
- Contracts first, so parallel tasks never edit the same files.
- A task is done when its listed commands pass: format, lint, tests, privacy gate.
- Two failed attempts by a Sonnet builder escalate to Opus. Two more stop the task and go to the maintainer.
- Every brief carries the privacy rule (section 12) and the provenance rule (11.4).

### 11.3 Keeping it fast and continuous

- Agents work from the pinned GPUI source and the cookbook crate, not from memory of the framework. CI builds the
  cookbook, so its examples cannot go stale.
- Compile cost: a shared compiler cache, a unified dependency set so worktrees reuse builds, `cargo check` before any
  full build, and at most two compiling agents at once locally. Specs and reviews run alongside.
- The work spans many sessions. A task ledger and a decision log live in the repository, so any session can restart
  cold from them. They follow the privacy rule like everything else.

### 11.4 Cost and provenance

- Every task has a hard token budget and every milestone a total. M0 measures the first real cost per task; budgets are
  set from that, and the maintainer approves each milestone's start. A task that hits its budget stops and reports.
- GPUI is permissively licensed, but much of the editor it comes from is not, including its terminal view. Agents may
  read permissively licensed code as reference and must not copy from anything else. Reviewers check where code came
  from, and `cargo-deny` checks dependency licences. A third-party notices file ships with every build.

## 12. Privacy

- A privacy gate (`cargo xtask privacy`) runs before every commit and in CI: a secret scanner over each commit, plus a
  scan for personal terms (email addresses, home-folder paths, host names, private network addresses).
- Recorded for M1 at the M0 gate: locally only the privacy scan runs; the secret scanner runs in CI only, which on a
  public repository is after publication. See `docs/build/M0-GATE.md`, section g, "The first task of M1".
- The personal term list is a local file that is never committed. CI uses generic patterns only.
- Fixtures use `example.com` hosts and invented names. No real session, vault or transcript content is ever a fixture.
- Commits use a no-reply address. Local instruction and workspace files are excluded from git.
- Pull requests, the task ledger, the decision log and agent reports are public and follow the same rule.

## 13. Left to the builder

- Which HTTP, SSE and provider client crates sit behind `Provider`.
- The SQLite schema and the embeddings store.
- Module layout inside each crate.
- Screen layouts follow the Electron app's design unless section 7 or `docs/acceptance/CHECKLIST.md` says otherwise.

Settled in M0, so no longer left open: the GPUI source and pin (the `gpui-pre` 0.3.12 snapshot, set once in the
workspace manifest; decision 0007), the PTY crate (`alacritty_terminal`'s own PTY; decision 0009) and the tray crate on
macOS and Windows (`tray-icon`; decision 0011). The packaging tool (cargo-packager, with cargo-deb for a `.deb` that
needs a setuid file; decision 0014) is proposed: it stands unless the maintainer reopens it, pending the maintainer's
confirmation of the application identity. The Linux tray backend is open
at the M0 gate: see `docs/build/M0-GATE.md`, question 11. Decision 0007 pinned the GPUI snapshot the widget kit requires
and deferred the kit itself; whether it is adopted is open at the M0 gate: see `docs/build/M0-GATE.md`, question 23.
The application identity and signing are open at the M0 gate: see `docs/build/M0-GATE.md`, question 5.

## 14. Main risks

| Risk | How the design answers it |
|---|---|
| GPUI is pre-1.0 and built for one editor's needs | Pinned snapshot, upgrades as their own change, `udx-ui-model` keeps screen logic framework-free, trials in M0. |
| Embedding Chromium in GPUI has no proven precedent | Trial in M0 on all three platforms, the `BrowserView` trait, and an external-window fallback per platform. |
| Linux and Windows are never touched by hand | Tiered support, automation-socket tests on CI, honest build labels, the virtual-machine check in M0. |
| Small local models are poor at tool calling | Model profiles, a prompt-based tool protocol, and published evaluation results per model. |
| Agent-built code that passes review but is wrong | Separate test author, mutation testing, evaluations, gates with a hands-on check. |
| Open-ended cost | Hard budgets per task and milestone, set from measured M0 figures, with approval at each milestone. |
