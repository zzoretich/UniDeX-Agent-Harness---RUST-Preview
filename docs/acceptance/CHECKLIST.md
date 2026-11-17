# Acceptance checklist

This file ports the Electron app's acceptance checklist into UniDeX Rust Preview. The source is the e2e coverage list
and smoke checklist of UniDeX Agent Harness 2.7.0 (Electron, MIT), which lists 346 items. Each milestone gate (design
section 10.4) takes its acceptance items from this file, so a row's fate decides whether a behaviour is built, built
differently, or left out.

Columns:

- `id`: the source item's id, unchanged. The source uses `R7` twice for two different items; `R7` is the proposal page
  item that sits between `R6` and `R8`, and `R7b` is the other, the instruction proposal item listed first. The source
  lists `CX1` to `CX15` as one row; they are split here in the order of the smoke checklist's Codex section.
- `source section`: the section of the source list the item belongs to.
- `behaviour`: what the user can do or see in this app, in one sentence. A `Change:` or `Drop:` tail gives the reason.
- `fate`: `carry` (kept as is), `change` (kept in a different form, with the reason) or `drop` (removed by the design's
  non-goals, a design call in section 7, or another named design section, with the reason).
- `milestone`: the design milestone (M0 to M11) that delivers it, or `none` for a drop. A trailing `?` marks a
  milestone that the design does not state and that was chosen by judgement, for the reviewer to confirm.
- `how verified`: `automated` (a test can assert it through the CLI, the view-model layer or the automation socket,
  design section 10), `manual` (needs a person: visual polish, OS dialogs, signing, a first run on a clean machine),
  or `not applicable` for a drop.

The source's release acceptance procedure (`docs/acceptance.md`) is the model for the M11 release sweep; its steps are
not rows here and are not counted in the 346.

## Totals


| Fate | Items |
|---|---|
| carry | 164 |
| change | 121 |
| drop | 61 |
| **total** | **346** |

| Milestone | Items |
|---|---|
| M0 | 0 |
| M1 | 2 |
| M2 | 0 |
| M3 | 1 |
| M4 | 45 |
| M5 | 20 |
| M6 | 49 |
| M7 | 28 |
| M8 | 61 |
| M9 | 33 |
| M10 | 36 |
| M11 | 10 |
| none | 61 |
| **total** | **346** |

## Install paths

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| I1 | Install paths | The app opens a vault on its home screen with no error logged. | carry | M6 | automated |
| I2 | Install paths | Settings shows the vault folder and the paths of any add-on engine programs as pickers, with a Probe button that re-resolves them. Change: The app needs no Node binary or Claude config folder; only the vault and add-on engine paths remain. | change | M5 | automated |
| I3 | Install paths | Settings shows which provider and model each role resolves to, with the reason, and a refresh button that re-reads it. Change: The source reads a runtime state file; the app reads its own role configuration. | change | M4 | automated |

## The app

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| APP1 | The app | A setup wizard checks for and downloads Node, uv, Ollama, and the two CLIs, verifying each pinned download. Drop: The app is one binary and never downloads or installs other tools (non-goal; section 7). Local model downloads are covered under local-model management. | drop | none | not applicable |
| APP2 | The app | First-run onboarding lets the user pick a provider and enter a key or choose a local model, name the Chief of Staff and its model, then open the app. Change: The wizard steps for host choice, CLAUDE.md diffs and Codex hook trust are gone; onboarding offers a provider, a key or a local model. | change | M11 | manual |
| APP3 | The app | Opening an existing install offers to update the vault's runtime, then runs the launch sequence. Drop: There is no separate runtime to update; the app is one binary and updates through its own updater. | drop | none | not applicable |
| APP4 | The app | Settings that an Obsidian-era vault kept are in effect on first start and copied into the app's data. Drop: The Electron app's settings are never read (section 5.5), and Obsidian is not part of this app. | drop | none | not applicable |
| APP5 | The app | Check for Updates checks for a new release, is greyed out with the reason when updates are off, and Restart to update appears once one is downloaded. | carry | M4 | manual |
| APP6 | The app | The tray icon opens a popover, closing the window hides it, and quitting really quits (with the quit gate over unfinished work). Change: The doctor row naming the app is not carried; there is no runtime to report it. | change | M4 | manual |
| APP7 | The app | The app follows the OS light or dark appearance under Match system, and the rail toggle, palette command, View menu and settings switch it, with the choice kept across restarts. | carry | M4 | manual |
| APP8 | The app | Every screen is legible in light and dark, each status colour always means the same thing, and the interface uses Inter with JetBrains Mono for code and numbers. | carry | M4 | manual |
| APP9 | The app | The app's name and mark appear consistently in the dock or taskbar, app menu, About box, window title and tray tooltip, and the tray mark follows the menu bar tint. | carry | M4? | manual |
| APP10 | The app | The app starts and is fully usable with no add-on engine installed, and onboarding says add-on engines are optional and can be added later. Change: There is no host choice to skip; add-on engines are optional by design. | change | M11 | manual |
| APP11 | The app | After install the wizard lists what each host left on this machine (rules, memories, servers, sessions) from a dry run and imports the ticked kinds. Drop: The non-goal excludes parsing past transcripts, and section 5.3 allows importing skills and agents (see SK1, AG1); the design includes no import of a host's rules, memories, servers or sessions. | drop | none | not applicable |
| APP12 | The app | An earlier install's host is named, and removing its wiring first runs the uninstall before the install continues. Drop: The app never installs into or removes anything from Claude Code or Codex. | drop | none | not applicable |

## Files

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| F1 | Files | Files sits after Spaces on the rail and opens on Icons at the vault top level, folders first with kind tabs, hiding dot-folders and build folders, with the item count at the foot. | carry | M8 | automated |
| F2 | Files | The app's own system folders are locked: their write actions are disabled with the reason, and the memory folder cannot be renamed, moved or trashed. Change: The locked paths are the `.unidex` folder and its index, not `brain/_index` and `brain/scripts`. | change | M8 | automated |
| F3 | Files | A double-click enters a folder or opens a note, with back, forward, a path bar and the up and down shortcuts. | carry | M8 | automated |
| F4 | Files | The List, Columns and Icons switcher works from its buttons and shortcuts, and the choice survives leaving the tab and restarting. | carry | M8 | automated |
| F5 | Files | List view sorts and reverses by clicking its column headers, with direction marks, and the arrow keys move in and out of folders. | carry | M8 | automated |
| F6 | Files | Columns view opens a column per folder and shows a preview with tags and links for a selected note. | carry | M8 | automated |
| F7 | Files | Selection works by click, modifier click, range click, select all, Escape, arrows and type-ahead, with the count at the foot. | carry | M8 | automated |
| F8 | Files | The preview pane toggles from its shortcut and the choice is remembered. | carry | M8 | automated |
| F9 | Files | Quick Look previews a note, a text file, an image or any other file with its Open action, steps with the arrows, and closes with Space or Escape. | carry | M8 | automated |
| F10 | Files | The context menu offers the right items for a file, a folder and empty space, including Reveal in the file manager. | carry | M8 | automated |
| F11 | Files | A workspace folder offers Open in Spaces. | carry | M8 | automated |
| F12 | Files | Search finds lines across notes and all text files, a line opens its note, and the find shortcuts focus it and Escape leaves it. | carry | M8 | automated |
| F13 | Files | Open file matches paths fuzzily, with Markdown files first. | carry | M8 | automated |
| F14 | Files | New note in the chosen folder creates, opens and selects it, and the form explains each refusal. | carry | M8 | automated |
| F15 | Files | Rename works in place with the stem selected and a name check, Move to keeps the bytes, and any open tab follows the file. | carry | M8 | automated |
| F16 | Files | Move to Trash asks first and says how many for a selection, sends files to the OS trash, and the toast points there with no Undo. | carry | M8 | automated |
| F17 | Files | When the Files window is narrower than 900 px the sidebar collapses behind its button. | carry | M8 | automated |
| F18 | Files | Files shows the same views, smart folders and tags with only an add-on engine or none installed, naming no engine anywhere. Change: The source variant is for a machine with only one of two named CLIs; the app has no host dependency, so the check becomes "no engine named". | change | M8 | automated |
| F19 | Files | Renaming or moving a file updates the links to it (an Update N links choice that is remembered), leaves the app's own system notes untouched, and reports the count. | carry | M8 | automated |
| F20 | Files | New folder (shortcut, plus button or menu) numbers duplicate names, and Duplicate copies a file or folder and selects the copies. | carry | M8 | automated |
| F21 | Files | Dragging a selection onto a folder, favorite or path-bar entry moves it, with a hint, and drops on system folders are refused with the reason. | carry | M8 | automated |
| F22 | Files | Undo (shortcut or toast) reverses moves and renames including their link updates, goes stale once the file moved elsewhere, and in a text field undoes typing. | carry | M8 | automated |
| F23 | Files | Icons shows images up to 4 MB as thumbnails, keeps a placeholder for larger or unreadable ones, and scrolls a large image folder without stalling. | carry | M8 | automated |
| F24 | Files | Smart folders list Recents, Changed today and Images, exclude system folders, and each opens a view with a back control. | carry | M8 | automated |
| F25 | Files | Tags lists the twelve most used with counts, and a tag lists its notes in the view in use. | carry | M8 | automated |
| F26 | Files | Favorites accept dragged folders, refuse files and duplicates with a reason, can be removed or restored to defaults, persist, and follow a rename. | carry | M8 | automated |
| F27 | Files | The search scope bar chooses vault or folder and names or contents, results navigate with the arrows, and a hit opens at its line or previews with Space. | carry | M8 | automated |

## Settings

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| S1 | Settings | A Settings gear sits at the rail's foot, stays visible in a short window, opens with Enter or Space, and is also in the palette. | carry | M4 | automated |
| S2 | Settings | Settings opens with "N changed from the defaults" matching `unidex config list`, and every row shows its key, its source, when it applies and daily caps as "today $x of $cap". Change: The config file is `config.toml` and the verb is `unidex config`. | change | M4 | automated |
| S3 | Settings | The master switches write their keys: Background AI asks before turning paid calls on, and the Chief of Staff switch pauses duties. Change: The design does not include a switch for local run recording, so that master switch is not carried; Background AI and the Chief of Staff switch remain. | change | M6 | automated |
| S4 | Settings | Settings has no free text box: number rows use minus and plus around presets, plus on a spend cap asks first, minus never asks, and both disable at the ends. | carry | M4 | automated |
| S5 | Settings | A value outside the presets shows as custom, opening the tab leaves it unchanged, and minus and plus step to the neighbouring presets. | carry | M4 | automated |
| S6 | Settings | List settings show as chips (the last required chip is locked), file-backed settings offer an edit-the-file button, and some settings link to their manager screens. Change: The files are the app's `config.toml` and vault files, and engine and install rows are plain text. | change | M10 | automated |
| S7 | Settings | Changing a setting that affects scheduled routines leaves a follow-up bar and a badge on Settings, and clicking it re-registers the OS wake-up and clears both. Change: The source runs a routines sync command; the app re-registers its single OS wake-up (section 5.9). | change | M6 | automated |
| S8 | Settings | The reset arrow returns a changed row to its default, engine and install rows are read-only, and a run-doctor button opens a terminal running `unidex doctor`. Change: There is no upgrade command; the app updates itself. | change | M4 | automated |
| S9 | Settings | Settings rows for an add-on engine that is not installed are dimmed with a hint saying how to add it, yet remain editable. Change: The source dims rows for a named missing CLI; the app dims rows for any missing add-on engine. | change | M5 | automated |
| S10 | Settings | A vault whose runtime predates the config command shows a prompt to upgrade it while the app's own settings keep working. Drop: There is no older runtime to support; the app is one binary. | drop | none | not applicable |
| S11 | Settings | The plugin settings section and the settings window's runtime tab show the same rows, and a change in one appears in the other. Drop: The app has one configuration store and one Settings screen, with no plugin and runtime split. | drop | none | not applicable |
| S12 | Settings | The vault picker shows a notice and keeps the saved vault when the chosen one is missing, and explains once when a different vault is picked. | carry | M4 | automated |
| S13 | Settings | The rail shows three sections divided by light lines, with Pulse in its own colour in each theme. Change: The older link ids for Code and Brain are not carried; the app never had them. | change | M6 | manual |
| ST-LOCAL | Settings | Settings shows the local engine, its address and state, the model and embedder, today's local calls and queued wraps, and can pull the model. Change: The app talks to the local server directly and has no `aos local` terminal button. | change | M4 | automated |
| ST-HOSTS | Settings | Settings lists each add-on engine with its detected state, sign-in help, the exact install steps when missing, and a re-check button. Change: The app only detects and explains; it never installs or edits an add-on (section 5.4). | change | M5 | automated |
| ST-IMPORT | Settings | Settings shows the last import from an existing vault with its counts and can import again. Change: Only `unidex import <path>` of a vault is carried; host history import is a non-goal. | change | M4? | automated |

## Config (both hosts)

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| C1 | Config (both hosts) | The config command lists every section with each row's source, prints what a set would write without changing any file under a dry-run flag, and unset restores the default. Change: The verb is `unidex config`, and the file is `config.toml`. | change | M1? | automated |
| C2 | Config (both hosts) | Turning the Chief of Staff off creates its disabled marker file and turning it on removes it, while setting the vault through config and changing the provider headlessly are refused. Change: The vault is chosen by the per-machine file, not by `config set`. | change | M6 | automated |
| C3 | Config (both hosts) | The doctor command shows a config row that warns about an unknown key. Change: The verb is `unidex doctor`. | change | M1? | automated |
| C4 | Config (both hosts) | `/aos config` works inside a Claude Code session. Drop: Host plugin commands are not installed (non-goal). | drop | none | not applicable |
| C5 | Config (both hosts) | `$agenticos:aos config` works inside a Codex session. Drop: Host plugin commands are not installed (non-goal). | drop | none | not applicable |

## Pulse

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| P1 | Pulse | The Health area of the Pulse popup shows a status light for every background job (index, scheduler, embeddings and so on), with idle or disabled ones grey and carrying the reason, and nothing red for a job that never ran. Change: The lights show the app's own jobs, not the runtime's manifest pipelines. | change | M6 | automated |
| P2 | Pulse | The briefing band is headed by the persona's name (or Briefing without one), is composed from facts when no saved briefing exists, can be turned on to add its routine, and each phrase opens its area. | carry | M6 | automated |
| P11 | Pulse | The cockpit shows ten tiles with no scrollbar at full size and at 960 by 600, where extra rows become "+N more", and the rail badge counts what needs the user, turning rose on an error. | carry | M6 | automated |
| P12 | Pulse | In the popup a tile opens its area, the list switches areas (also with the arrow keys), Escape closes it, and safe actions act in place while decisions jump to their screen. | carry | M6 | automated |
| P15 | Pulse | The Brain tile shows today's count with how many arrived since the user looked, and the to-review, drafts and bootstrap percentage figures jump to their Brain screens without any keep, edit or revert. | carry | M9 | automated |
| P16 | Pulse | The popup's Brain area shows the three figures as jumping rows, today's memories newest first with their state and an Open link, and Needs you rows open Brain's Review. | carry | M9 | automated |
| P13 | Pulse | Running the briefing routine writes the saved briefing with the provider that produced it and a spend row, Pulse shows it, and refresh writes it again. Change: Any role's model can write it; there is no Claude-only variant. | change | M6 | automated |
| P14 | Pulse | When an add-on engine or a local model writes the briefing, Pulse records that provider, and its Review buttons start a session on the chosen engine. Change: The source names Codex; the app starts whichever engine is configured, native or add-on. | change | M6 | automated |
| P3 | Pulse | The cost row in Pulse is absent while cost tracking is off and present with it on and a budget set. | carry | M6 | automated |
| P4 | Pulse | With cost tracking off Pulse shows no cost reconciliation cards, while the open-health card still shows. Change: The anchor and backfill cards repair transcript-derived costs, which the app does not use; only the cost cards that remain are covered. | change | M6 | automated |
| P5 | Pulse | The deck's scan command refreshes the vault index. Change: It runs the app's own indexer rather than a Node script. | change | M6 | automated |
| P6 | Pulse | The deck's reflect-week command writes the week's reflection, or reports it cannot when no provider is configured. Change: The writer is any role's model. | change | M6? | automated |
| P7 | Pulse | The deck's remember command opens a one-field form and the note lands in the session working-memory file. | carry | M6 | automated |
| P8 | Pulse | The deck's pattern command asks for an area, title and pattern and writes the pattern file and its index line. | carry | M6 | automated |
| P9 | Pulse | Switching tabs five times leaves exactly one live subscription to new-session events. Change: The source counts DOM listeners; the app counts event-stream subscriptions. | change | M4 | automated |
| P10 | Pulse | The heartbeat pill is absent before the watchdog has run, shows a heart with per-duty tooltips, turns amber at two hours and rose when a duty was missed. | carry | M6 | automated |
| P17 | Pulse | Pulse shows "Digesting history: N of M sessions" while queued host sessions are processed, and the digest routine can be turned off. Drop: Digesting host session history is a non-goal (section 2). | drop | none | not applicable |

## To-Do

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| T1 | To-Do | To-Do heads the rail's second section, shows "Nothing open" without a to-do file, and opening it creates the file. | carry | M8 | automated |
| T2 | To-Do | Quick-add appends a line under the Open heading of the to-do file. | carry | M8 | automated |
| T3 | To-Do | To-dos group as overdue, today, upcoming and someday in priority order with late markers, and the badge follows a hand edit within a second even with another tab active. | carry | M8 | automated |
| T4 | To-Do | Ticking an item moves it to Done and unticking moves it back. | carry | M8 | automated |
| T5 | To-Do | Double-clicking edits the whole line, Enter saves and Escape cancels, and priority, date and delete controls work. | carry | M8 | automated |
| T6 | To-Do | Tag chips filter the list, a row's tag filters, and clear removes the filter. | carry | M8 | automated |
| T7 | To-Do | If a line changed on disk since it was read, the edit is reported as stale and the quick-add draft survives. | carry | M8 | automated |
| T8 | To-Do | A to-do due today turns overdue at local midnight while the screen stays open. | carry | M8 | automated |

## Proposals

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| R7b | Proposals | A proposed change to the app's own prompts, persona, skills or agent files shows the exact diff, is approved from the screen and can be undone, including a file the user owns. Change: Diffs apply to the app's own files only, never to host instruction files (section 2 non-goals; section 5.3 never mirrors back). | change | M6 | automated |
| R1 | Proposals | Proposals closes the rail's second section after Routines, hides its badge with no proposals, shows an amber count within a second of a proposal appearing, and clears it when deleted. | carry | M6 | automated |
| R2 | Proposals | Pending proposals show slug, target, kind, surface, age, days confirmed (green at two or more) and a warning with reasons. | carry | M6 | automated |
| R3 | Proposals | An expanded proposal shows its needs, recipe, What, Why and Risk as Markdown and premises, with Open file, and group heads collapse. | carry | M6 | automated |
| R4 | Proposals | Backlog and History list newest first (History without filed rows), and the rates shown match the ledger summary command. | carry | M6 | automated |
| R5 | Proposals | Review opens a session on the chosen engine running the flag-review prompt, with no extra shell. Change: The source types a Claude command or a Codex skill; the app opens a native or add-on session. | change | M6 | automated |
| R6 | Proposals | Without a persona folder, Proposals shows only the "isn't set up" line. | carry | M6 | automated |
| R7 | Proposals | A proposal's page link appears after the next scan, a row opens its page, and the page reads well in light and dark. Change: The page is opened in the embedded or default browser rather than generated by a Node script. | change | M6 | automated |
| R8 | Proposals | A proposal's Markdown gains the page link once, a second run skips it, and backlog and history rows with a page link to it. | carry | M6 | automated |

## Notifications

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| N1 | Notifications | Notifications heads the rail, and with no notifications folder shows no badge and "No notifications yet" with a hint. | carry | M6 | automated |
| N2 | Notifications | Posting a notification shows an amber count within a second even with another tab active. | carry | M6 | automated |
| N3 | Notifications | A breaking notification raises an OS notification and turns the badge rose, and turning the OS alert option off suppresses it. | carry | M6 | manual |
| N4 | Notifications | Unread notifications list newest first, an edition opens in a reading pane with intro, sections and buttons, the row turns read, and Mark all read works. | carry | M6 | automated |
| N5 | Notifications | Level chips and a sender dropdown filter, All and Archived views switch, and Archive, Unarchive, Mark unread and Open note work. | carry | M6 | automated |
| N6 | Notifications | More like this turns chosen and appends to the reactions file. | carry | M6 | automated |
| N7 | Notifications | Deep dive opens a session on the chosen engine running the skill, with one button per available engine. Change: The source offers one button per CLI host; the app offers native and any detected add-on. | change | M6 | automated |
| N8 | Notifications | A hand-edited notification action that is not allowed shows no button, and a file without frontmatter shows the unreadable footer. | carry | M6 | automated |
| N9 | Notifications | A notifications command in the app's own engine chat lists and manages notifications. Change: It is an own-engine skill; add-on engines have no UniDeX plugin (section 2). | change | M6 | automated |

## Agent teams (both hosts)

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| A1 | Agent teams (both hosts) | Team commands initialise, list and show status for the agent teams from the command line. Change: The verb is `unidex team`. | change | M10 | automated |
| A2 | Agent teams (both hosts) | Putting an item on the team board works from the command line, and a headless run refuses a gated step. Change: The verb is `unidex team`; a headless approval is a deny, as in section 5.2. | change | M10 | automated |
| A3 | Agent teams (both hosts) | A detached team dispatch survives the process that started it, and a hard kill of the dispatcher is recorded. | carry | M10 | automated |
| A4 | Agent teams (both hosts) | A team command in the app's own engine chat works the team board. Change: It is an own-engine skill; add-on engines have no UniDeX plugin. | change | M10 | automated |

## Agent Teams tab (both hosts)

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| AT1 | Agent Teams tab (both hosts) | Agent Teams sits after Brain and above Agents, has a palette entry, shows "No teams yet" with a hint, and Seed creates a starter team. | carry | M10 | automated |
| AT2 | Agent Teams tab (both hosts) | A gate put on the board shows an amber count within a second, and a Needs You card shows item, gate, wait, spend, the lead's pitch, budget presets with minus and plus, and disables presets below spend. | carry | M10 | automated |
| AT3 | Agent Teams tab (both hosts) | Approve asks first, Cancel writes nothing, Approve records the decision and opens the lead, and a stale approval is refused. | carry | M10 | automated |
| AT4 | Agent Teams tab (both hosts) | Redirect opens the lead's session with the gate and the recording line, offering one button per available engine. Change: The source opens Claude or Codex terminals; the app opens native or add-on sessions. | change | M10 | automated |
| AT5 | Agent Teams tab (both hosts) | The board shows columns, status chips and folded done items, a detached dispatch on its card, and an item detail with facts, posts, runs and a budget stepper, with a paused hint. | carry | M10 | automated |
| AT6 | Agent Teams tab (both hosts) | The roster shows provider, model and effort, status, item and last run per member, with a button to open the member's session, and sub-teams indented and marked in the tree. Change: The per-member buttons name engines rather than two CLIs. | change | M10 | automated |
| AT7 | Agent Teams tab (both hosts) | Interact shows the team channel with mentions and item chips, posting works with Enter and Shift+Enter, and the draft is kept. | carry | M10 | automated |
| AT8 | Agent Teams tab (both hosts) | Manage has no text box, uses preset pickers and a running switch, Add lists agents not on the team, and Remove works. | carry | M10 | automated |
| AT9 | Agent Teams tab (both hosts) | An unsupported team file line shows a rose mark and an error card with Open file, and the list command shows it unreadable. | carry | M10 | automated |
| AT10 | Agent Teams tab (both hosts) | Seat sessions (Talk, Redirect, Approve) run on the engine the member is set to, and with no add-on installed only the native engine is offered. Change: The source dispatches per CLI host; the app dispatches per engine. | change | M10 | automated |
| AT11 | Agent Teams tab (both hosts) | If a detached dispatcher is killed, the member leaves the working state within a minute. | carry | M10 | automated |

## Status line (both hosts)

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| SL1 | Status line (both hosts) | The status line installs into Claude Code with model, context and gate lines. Drop: Installing a status line into a host is a non-goal. | drop | none | not applicable |
| SL2 | Status line (both hosts) | The status line chains an existing one and shows subagent rows. Drop: Host status lines are a non-goal. | drop | none | not applicable |
| SL3 | Status line (both hosts) | A replaced Claude status line is reported and taken back. Drop: Host status lines are a non-goal. | drop | none | not applicable |
| SL4 | Status line (both hosts) | Codex's status line is set in its config file. Drop: A status line set up in a host is a non-goal. | drop | none | not applicable |
| SL5 | Status line (both hosts) | Uninstalling restores both host files. Drop: Nothing is installed into hosts, so nothing is restored. | drop | none | not applicable |
| SL6 | Status line (both hosts) | The status bar shows live state and gate counts, a gate jumps to Agent Teams, a flag to the state file, "all clear" when quiet, and idle dimmed. | carry | M10 | automated |
| SL7 | Status line (both hosts) | A `unidex://` link opens the named tab, also when the app was not running, and an unknown tab keeps the current one. Change: The link scheme is `unidex://` rather than the older scheme. | change | M4 | automated |

## Spaces / Runs

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| SMR1 | Spaces / Runs | The Spaces list groups workspaces as pinned, active, stalled and idle (idle folded with paused and done), filters by name, branch or next step, offers archived and outside folders, and each row shows its status dot, name, live mark, age, git state, session counts and next or now line. | carry | M8 | automated |
| SMR2 | Spaces / Runs | A workspace row shows per-engine session counts for sessions run in this app in the last 30 days, and a vault-level session that mostly touches one workspace raises that workspace's count. Change: Counts come from this app's own sessions, not from host transcripts (non-goal). | change | M8 | automated |
| SMR3 | Spaces / Runs | "Outside workspaces" lists folders where sessions ran outside any workspace, with their engines, last activity and git state, with vanished ones marked, an Adopt popover, and temp, home and engine config folders never listed. Change: Only folders from this app's sessions are listed; host transcripts are not read. | change | M8 | automated |
| SMR5 | Spaces / Runs | The Runs screen updates within a second of a new run record. | carry | M8 | automated |
| SMR6 | Spaces / Runs | A session in a worktree checkout of a workspace counts for that workspace rather than as an outside folder, whichever engine ran it. Change: The source names Codex worktrees; the rule applies to every engine. | change | M8 | automated |
| SMR7 | Spaces / Runs | The dossier header offers the status pill and menu, a pin toggle, the meta line, a Resume split button, Terminal, the file manager, and a More menu with rename, pin, status, link code folder and archive. | carry | M8 | automated |
| SMR8 | Spaces / Runs | Pick up shows last, now, next and read sections with their sources and empty states, and refresh on the read works unless no provider is configured. | carry | M8 | automated |
| SMR9 | Spaces / Runs | Overview shows the summary, objectives with progress and documents, and Files shows one described tree with new and changed marks, a filter, Describe N new, and a preview with Open, Copy path, Reveal and refresh. | carry | M8 | automated |
| SMR10 | Spaces / Runs | History lists sessions and commits with Resume or Open, and Linked shows to-dos, proposals and memory matched to the workspace and how. | carry | M8 | automated |
| SMR11 | Spaces / Runs | Resume restarts a thread in its workspace terminal: a native thread continues in-app and an add-on thread is loaded again over ACP, with the row marked live and Resume disabled with the reason when its engine is off or not resumable. Change: The source types per-CLI resume commands; the app resumes native threads and loads add-on sessions through ACP. | change | M8 | automated |
| SMR12 | Spaces / Runs | Links by workspace open Sessions and Code scoped to it, Pulse rows open Spaces, a workspace link selects only, and Sessions links are off without a provider. | carry | M8 | automated |
| SMR13 | Spaces / Runs | Spaces lays out three panes that fold below 1100 px and stack below 760 px, with colour used for state only and every dot labelled. | carry | M8 | automated |
| SMR14 | Spaces / Runs | New space asks for a name with why-not messages, offers Blank folder, Code repo or Clone from GitHub and an Open with choice among available engines and the terminal, and creates a pinned workspace with its stub files. Change: The Open with list holds native and detected add-on engines. | change | M8 | automated |
| SMR15 | Spaces / Runs | Clone from GitHub checks the URL, creates an empty workspace and runs the clone in a terminal followed by the stub creation, with a progress card and a failed clone leaving an empty workspace. Change: The follow-up command is `unidex workspace stubs`. | change | M8 | automated |
| SMR16 | Spaces / Runs | The Adopt popover adopts an outside folder into a workspace as an alias, links it as the code folder where allowed, or creates a workspace, each after a confirmation. | carry | M8 | automated |
| SMR17 | Spaces / Runs | The status pill's menu sets automatic, active, paused or done, pinning writes the pinned key, and the rest of the file is kept. | carry | M8 | automated |
| SMR18 | Spaces / Runs | Draft workspace.md shows the fields as text with sources, who answered and an exact-file preview, saves only changed keys, and can redraft or cancel, with the drafting model named. Change: Any role's model can draft it, with a heuristic fallback. | change | M8 | automated |
| SMR19 | Spaces / Runs | Archive and Restore show a confirmation of what moves, keep threads read-only, refuse turns in an archived workspace with a message, and restore reverses it. | carry | M8 | automated |
| SMR20 | Spaces / Runs | Rename shows the new name's checks and what moves, the list and thread groups follow, and older threads continue in the new folder for native and add-on engines. Change: The source resumes Claude and Codex threads; the app resumes native threads and add-on ACP sessions. | change | M8 | automated |
| SMR21 | Spaces / Runs | With the Spaces write surface off, every action is refused with a message and nothing changes. Drop: The design has no staged or read-only write mode. | drop | none | not applicable |
| SMR22 | Spaces / Runs | Under Linked, the to-do section keeps its heading at zero with an add-to-do control, which writes a tagged line under Open, and is disabled with a reason when to-do writes cannot run. | carry | M8 | automated |
| SMR23 | Spaces / Runs | Link to-dos offers untagged to-dos that name the workspace, one click tags the line and moves it into the to-do list, and nothing is tagged without a click. | carry | M8 | automated |
| SMR24 | Spaces / Runs | Inside a session, the workspace lookup works from a workspace, its worktree or its code folder, the todo, propose and project commands link the workspace and are off at the vault root, and the set command pins it. Change: The commands are own-engine skills and `unidex workspace` verbs; add-on engines have no plugin. | change | M8 | automated |

## Brain

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| BR1 | Brain | Brain has its own rail icon and opens by palette or link, with a header showing the store line, search, Capture, five mode tabs with counts, arrow-key mode switching, and typing that opens and filters Library. | carry | M9 | automated |
| BR2 | Brain | Review by session groups this week's batches newest first, each titled by its session with the engine's mark, time, new and looks-known counts and a lead, with older ones folded and unknown sessions shown as an id and day. Change: Batches come from native and add-on sessions run in this app, not from host transcripts. | change | M9 | automated |
| BR3 | Brain | A batch shows its title, meta, lede, a Keep-the-new action, a menu with Keep all and Archive all, and per-card type, title, summary and actions with keys, a looks-like score, arrow navigation, and Edit that writes nothing. | carry | M9 | automated |
| BR4 | Brain | Keeping a memory marks it reviewed only, adds a trail row, leaves the index line alone, selects the next card, and Keep all and the count update. | carry | M9 | automated |
| BR5 | Brain | Archive archives without asking and keeps the description, removes the index line and recall presence, Archive all asks first, Library can show and restore archived memories, and nothing is deleted. | carry | M9 | automated |
| BR6 | Brain | Pin pins a memory, marks it, rebuilds the bootstrap naming it under critical rules, and pinning again restores the same bytes. | carry | M9 | automated |
| BR7 | Brain | Merge shows the memory beside the one it looks like with their dates, Keep the older supersedes the newer with its trail and a Restore, and Keep both records the pair. | carry | M9 | automated |
| BR8 | Brain | The queues show counts for drafts (approve or reject), duplicates (keep older or newer, not duplicates, open both) and memories older than 30 days by month with Keep all and Archive all. | carry | M9 | automated |
| BR9 | Brain | Mark everything before a day reviewed shows the default day and a live count, is off at zero, Cancel keeps nothing, and Mark keeps exactly those written before the day. | carry | M9 | automated |
| BR10 | Brain | Library shows the four kinds with counts, live memories sorted by use then newest, filters by type, status, source and use, and searches titles and bodies, and a row opens the reader. | carry | M9 | automated |
| BR11 | Brain | The reader shows title, meta and Markdown body with Edit, Pin, Status, Keep, Archive or Restore, open in the split pane's inspector, and whether it is in the bootstrap, with draft and pattern variants. | carry | M9 | automated |
| BR12 | Brain | The context pane shows use in the last 30 days, what links to the memory, nearby memories (by meaning or by words with scores), and its history trail and path. | carry | M9 | automated |
| BR13 | Brain | The Map's Every node view draws the memories only, linked by the links our own index finds, with counts, a settle then fit, unreviewed marks, a legend, and open, drag, modifier-click and Resettle controls. Change: The Map is drawn from our own index links, not from an external graph builder (section 7). | change | M9 | automated |
| BR19 | Brain | The Map's Communities and Around views draw bubbles sized and named by memories with the unreviewed ring, a list, a card with members and linked communities, a footer and an Around picker, in both themes. Change: The communities come from our own index, not an external graph builder (section 7). | change | M9 | manual |
| BR14 | Brain | Bootstrap shows the three budgets as progress bars, the bootstrap file by section with token counts, actions to unpin, mark complete and pin a memory, a looks-shipped list and left-out items. | carry | M9 | automated |
| BR20 | Brain | The recall preview runs the app's recall on typed text and shows hits with score, title, kind, age and stale marks, opens memories and notes, says when nothing is recalled, and writes no usage row. Change: It calls the app's own recall rather than a Node script. | change | M9 | automated |
| BR21 | Brain | Health shows six tiles with state words, duplicate and cluster actions, dead-weight lists with Review and Archive shipped, and a rescan. | carry | M9 | automated |
| BR22 | Brain | Health's hygiene fixes list each kind with a count, show a dry-run dialog by file with Cancel focused, apply exactly what was listed, and write one trail row per change. | carry | M9 | automated |
| BR24 | Brain | Merge with AI opens a dialog with sources, a by-hand draft and an estimate, drafts with one model call (with cost line, Redraft and replace confirmation), and saves the merge with its effects listed. | carry | M9 | automated |
| BR25 | Brain | A merged memory's history offers Undo merge for 30 days, which asks first, restores the older text and the other memories with their lines, and is not offered once the memory changed or the merge was undone. | carry | M9 | automated |
| BR26 | Brain | The consolidation sweep shows its estimate or off with the reason, runs and lists cards by scope with merge, archive and retitle actions, and Dismiss for good. | carry | M9 | automated |
| BR27 | Brain | Consolidate with AI on a Health cluster or a Map community asks with a scoped estimate, sweeps only those memories and shows its cards in place, or explains why it is off. | carry | M9 | automated |
| BR28 | Brain | The merge-draft, merge, unmerge and sweep commands work from the command line and from an outside agent through the app's MCP server. Change: The verbs are `unidex memory`, and an outside agent reaches them through `unidex serve`, not a plugin. | change | M9? | automated |
| BR15 | Brain | The Brain rail badge counts memories written since Review was last open (never the backlog), is absent on first start, and shows one within seconds of a new memory. | carry | M9 | automated |
| BR16 | Brain | With the brain write surface off, every action is refused with a message and the card returns. Drop: The design has no staged or read-only write mode. | drop | none | not applicable |
| BR17 | Brain | The keep, archive and restore commands run from the command line or an outside agent through the MCP server and show in the hub. Change: The verbs are `unidex memory` and the MCP tools of `unidex serve`. | change | M9 | automated |
| BR23 | Brain | The memory-fix command previews what Health counts without writing, applies it so Health updates within a second, rebuilds the index with an index kind, and reports an unknown kind. Change: The verb is `unidex memory fix`. | change | M9 | automated |
| BR18 | Brain | Brain's panes fold the same way (context below 1100 px, stacking below 760 px), and in both themes every coloured mark has a word, type tags stay neutral, and amber marks what needs the user. | carry | M9 | automated |

## Routines

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| RT1 | Routines | Routines lists every routine with its cadence, next fire, last run and health, with the seeded duties guarded. | carry | M6 | automated |
| RT2 | Routines | New routine shows an error for an invalid cron and the cadence with the next three fires for a valid one, and create writes the routine and registers it. | carry | M6 | automated |
| RT3 | Routines | Changing the schedule of a protected duty asks for confirmation first, Cancel leaves it untouched, and Write anyway saves it. | carry | M6 | automated |
| RT4 | Routines | The on and off switch rewrites the routine's enabled flag. | carry | M6 | automated |
| RT5 | Routines | Run on a command routine runs it. | carry | M6 | automated |
| RT6 | Routines | An "outside the app" section lists externally labelled jobs from configuration, read-only. Change: The source also listed an Obsidian Git timer, which no longer exists. | change | M6 | automated |
| RT7 | Routines | Codex Automations appear with their pills and chips, with a refresh, and without the app one dim line says why. Drop: The design includes no reading of another tool's scheduler or cloud routines. | drop | none | not applicable |
| RT8 | Routines | Claude cloud routines appear after the cloud command, with a pill, link, schedule and ran-once state. Drop: The design includes no reading of another tool's scheduler or cloud routines. | drop | none | not applicable |
| RT9 | Routines | A duty run made outside the open app, by the OS wake-up or the command line, shows in the routine's last run with its trigger. Change: The trigger is the app's own run-due record, not a duty-log shell script. | change | M6 | automated |
| RT10 | Routines | A prompt routine has an engine select (default, a native role, or a detected add-on) with a model list, the local model's fields, and the engine shown on each prompt row. Change: The source offers its two CLIs and a local option; the app offers native roles and add-on engines. | change | M6 | automated |
| RT11 | Routines | A routine on a local model runs, writes only inside its writes list, logs its tool calls at no cost, and when the local server is down is skipped with a local-down reason. | carry | M6 | automated |

## Skills (both hosts)

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| SK1 | Skills (both hosts) | Your Skills lists skills from the vault and per-user folders with source marks, plus plugin and built-in groups, head counts and a note on import. Change: The source shows host pills and a sharing note; the app shows where each skill was found or imported from. | change | M10 | automated |
| SK2 | Skills (both hosts) | The skill list re-renders when skill files change on disk. Change: File watching replaces the stale-cache sync command (section 5.5). | change | M10 | automated |
| SK3 | Skills (both hosts) | The filter narrows both sections without losing focus. | carry | M10 | automated |
| SK4 | Skills (both hosts) | A skill can be run in a new session of the chosen engine, copied as its invocation, or opened as its SKILL.md. Change: The run buttons name native or add-on engines rather than two CLIs. | change | M10 | automated |
| SK5 | Skills (both hosts) | Sharing or unsharing a skill with the other host. Drop: Skills are imported from other tools' folders but never mirrored back (section 5.3). | drop | none | not applicable |
| SK6 | Skills (both hosts) | A new skill made in Claude Code is mirrored to Codex at session end. Drop: No mirroring into other tools (section 5.3). | drop | none | not applicable |
| SK7 | Skills (both hosts) | A new skill made in Codex is mirrored to Claude Code. Drop: No mirroring into other tools (section 5.3). | drop | none | not applicable |
| SK8 | Skills (both hosts) | A hand-edited mirrored copy is left alone, reads "copy edited", doctor warns and reset restores it. Drop: No mirrored copies exist. | drop | none | not applicable |
| SK9 | Skills (both hosts) | With Codex off, sharing is off, only the Claude pill shows and nothing is written under the Codex skills folder. Drop: No sharing exists; the app never writes into other tools' folders. | drop | none | not applicable |
| SK10 | Skills (both hosts) | The skill list ends with a link to Connectors, which opens Connectors. | carry | M10 | automated |

## Connectors (both hosts)

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| CN1 | Connectors (both hosts) | The board shows the head line, four tiles, a needs-attention card, connector and MCP server cards with tools, use and a sparkline, and folded rows, in both themes. Change: Cards mark which engines use a server instead of two host dots. | change | M10 | automated |
| CN2 | Connectors (both hosts) | A card's sheet shows tags, usage, tools with call counts and where it was found, closes with Escape, the close button or the scrim, and returns focus to the card. | carry | M10 | automated |
| CN3 | Connectors (both hosts) | The filter keeps focus, and a tile filters until clicked again. | carry | M10 | automated |
| CN4 | Connectors (both hosts) | Check now checks every server, a check runs on open when due and every minute while in front, and a refused check shows a notice or a head message with no retry before its cadence. Change: The verb is `unidex mcp check`. | change | M10 | automated |
| CN5 | Connectors (both hosts) | The rail badge shows the attention count in rose after a check within a day and is hidden at zero, before any check and after a day. | carry | M10 | automated |
| CN6 | Connectors (both hosts) | For Claude Code a connector's fix opens its web settings, shows a slash-command hint and a Check again, and a missing `claude` shows a banner. Drop: Claude.ai connectors belong to the add-on, which the app detects and does not manage (section 5.4). | drop | none | not applicable |
| CN7 | Connectors (both hosts) | On a Codex-only machine the board shows Codex's servers and ChatGPT apps, signs in through the Codex command, and shows a banner when it is missing. Drop: Another tool's server list and login belong to that add-on, which the app does not manage (section 5.4). | drop | none | not applicable |
| CN8 | Connectors (both hosts) | Copy to Codex confirms the file, backup and add command, runs it, and shows a new dot on the card. Drop: The app never writes into another tool's configuration (sections 5.3 and 5.4: imports are one-way, add-ons are detected and not edited). | drop | none | not applicable |
| CN9 | Connectors (both hosts) | Copy to Claude Code confirms the write to its user file and runs its add command. Drop: The app never writes into another tool's configuration (sections 5.3 and 5.4: imports are one-way, add-ons are detected and not edited). | drop | none | not applicable |
| CN10 | Connectors (both hosts) | Before any check the board says "Not checked" and "Nothing to report until the first check", with Check now available. | carry | M10 | automated |
| CN11 | Connectors (both hosts) | Park confirms what it will change, backs up the entry and keeps it stashed unreadable by the page, shows the row as parked, and Turn on puts the entry back unchanged and removes the stash. Change: Park applies to the app's own server list, not to another tool's command. | change | M10 | automated |
| CN12 | Connectors (both hosts) | On Codex, Park runs its remove command and Turn on its add command with the real values, with environment variables returned as a line to paste. Drop: The app never writes into another tool's configuration (sections 5.3 and 5.4). | drop | none | not applicable |
| CN13 | Connectors (both hosts) | Remove (danger) says nothing is stashed, removes the server after a backup, and the card leaves the board. Change: It removes from the app's own server list. | change | M10 | automated |
| CN14 | Connectors (both hosts) | Add server shows a dialog (names only, a secret value refused, stored in the keychain), the exact entry it will write, and adds the server for the app's engines. Change: The source adds to two hosts; the app adds to its own list. | change | M10 | automated |
| CN15 | Connectors (both hosts) | On a Codex-only machine Add server offers Codex alone, a remote server takes a bearer-token variable, and Park and Turn on work on Codex. Drop: The app never writes into another tool's configuration (sections 5.3 and 5.4). | drop | none | not applicable |
| CN16 | Connectors (both hosts) | The servers the app cannot park or remove, such as its own `unidex serve` entry or one defined by a project or an import, show what to do instead of Park or Remove. Change: Plugin servers and claude.ai or ChatGPT connectors are host things the app does not manage (CN6, CN7); the protected kinds here are the app's own (section 5.6). | change | M10 | automated |
| CN17 | Connectors (both hosts) | With MCP server changes switched off, no confirm appears and a notice names the exact command to run. Drop: The design has no staged or read-only write mode. | drop | none | not applicable |

## Agents (both hosts)

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| AG1 | Agents (both hosts) | Your Agents lists sub-agent definitions from the vault and per-user folders with source marks and plugin groups, with head counts. Change: The source shows host pills and read-only pills for mirrored copies; the app shows where each was found or imported from. | change | M10 | automated |
| AG2 | Agents (both hosts) | The agent list re-renders when files change on disk, and the filter keeps focus. Change: File watching replaces the stale-cache sync command. | change | M10 | automated |
| AG3 | Agents (both hosts) | An agent can be run in a new session of the chosen engine, copied as its mention, or opened as its file. Change: The run buttons name native or add-on engines. | change | M10 | automated |
| AG4 | Agents (both hosts) | Sharing or unsharing an agent with the other host. Drop: Agents are imported but never mirrored (section 5.3). | drop | none | not applicable |
| AG5 | Agents (both hosts) | A new agent made in Claude Code is mirrored to Codex. Drop: No mirroring into other tools. | drop | none | not applicable |
| AG6 | Agents (both hosts) | A new agent made in Codex is mirrored to Claude Code. Drop: No mirroring into other tools. | drop | none | not applicable |
| AG7 | Agents (both hosts) | A hand-edited mirrored agent copy is left alone and an agent that cannot be shared says so. Drop: No mirrored copies exist. | drop | none | not applicable |
| AG8 | Agents (both hosts) | With Codex off, sharing is off and a disabled agents setting in its config warns. Drop: No sharing exists; the app never touches the other tool's configuration. | drop | none | not applicable |

## Cross-review and handoff (both hosts; each run spends)

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| X1 | Cross-review and handoff (both hosts; each run spends) | A review of work built on one engine is run by a different engine and returns a verdict. Change: The reviewer is any other engine or role, not only Codex reviewing Claude (section 5.4). | change | M5 | automated |
| X2 | Cross-review and handoff (both hosts; each run spends) | Cross-review and handoff work started from the other side, from an add-on engine to a different engine or role. Change: Any two engines or roles can be paired. | change | M5 | automated |
| X3 | Cross-review and handoff (both hosts; each run spends) | With only one engine available, review runs as a same-provider review by another role. Change: The fallback is another role on the same provider. | change | M5 | automated |

## Codex host

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| CX1 | Codex host | The Codex plugin is installed at the repo's version and its MCP entry and skills folder are listed. Drop: Installing a plugin into Codex is a non-goal. | drop | none | not applicable |
| CX2 | Codex host | The doctor command shows Codex login, plugin, hook-trust and MCP rows. Drop: The app has no Codex plugin or hooks to check; add-on detection is covered by ST-HOSTS. | drop | none | not applicable |
| CX3 | Codex host | An upgrade on a directly wired machine installs the plugin, removes the old wiring and prints the hooks reminder. Drop: There is no host wiring or upgrade. | drop | none | not applicable |
| CX4 | Codex host | A session run on an add-on engine appears in Runs with its cost and model where the engine reports usage. Change: The record comes from ACP session updates, not hooks or a transcript. | change | M8? | automated |
| CX5 | Codex host | With no Claude available, a prompt routine still runs through whichever role's model is configured and the run and spend are recorded. Change: Roles with fallbacks replace the Claude-then-Codex provider chain. | change | M6 | automated |
| CX6 | Codex host | A new native session's first prompt receives the bootstrap context and conventions. Change: An internal event does this for the own engine (section 5.2); add-on engines have no injection. | change | M3 | automated |
| CX7 | Codex host | The wrap, remember and recall skills are offered in Codex, and no stale source-command skills appear. Drop: Host plugin skills are a non-goal. | drop | none | not applicable |
| CX8 | Codex host | After a turn, a live record holds tool-use batches with Codex tool names and the stop hook raises no error. Drop: Host hooks and transcripts are not used. | drop | none | not applicable |
| CX9 | Codex host | With cost on, closing a thread appends a Codex-rollout cost record, and the Runs tab shows it. Drop: Parsing host transcripts for cost is a non-goal. | drop | none | not applicable |
| CX10 | Codex host | An upgrade with no Claude vendors the runtime from the Codex marketplace folder and status reads the Codex daily cap. Drop: There is no runtime to vendor. | drop | none | not applicable |
| CX11 | Codex host | Skills say what Codex can do, name per-host fixes and avoid references to other hosts' tools. Drop: Host plugin skills are a non-goal. | drop | none | not applicable |
| CX12 | Codex host | The session-start update notice compares the Codex plugin's version with the vault's. Drop: There is no plugin version. | drop | none | not applicable |
| CX13 | Codex host | With no Claude, the semantic graph build names Codex in its consent line, runs through it, and records graph spend rows. Drop: The external graph builder is not carried (section 7). | drop | none | not applicable |
| CX14 | Codex host | With cost on, a Codex run missed at session end is costed later by a backfill command and counted in the Pulse fix queue. Drop: Parsing host transcripts for cost is a non-goal. | drop | none | not applicable |
| CX15 | Codex host | Uninstalling the host removes the plugin and marketplace and its hook entries from Codex. Drop: The app installs nothing into Codex, so nothing is removed. | drop | none | not applicable |
| CX16 | Codex host | Resuming an add-on engine's thread from Spaces or History reopens it in its workspace folder, and the end bar's Resume returns the same thread. Change: The thread is reloaded through ACP rather than a typed resume command. | change | M8 | automated |
| CX17 | Codex host | A Codex thread shows its name from the host's index file in the pick-up and History after a scan. Drop: History covers sessions run in this app (section 2), so host index files are not read. | drop | none | not applicable |
| CX18 | Codex host | Codex skills for to-do, propose and project tag the workspace from inside a workspace or worktree and ask to approve out-of-folder writes. Drop: Host plugin skills are a non-goal; the own-engine equivalent is SMR24. | drop | none | not applicable |
| CX19 | Codex host | Brain's Review lists a batch for each session run on an add-on engine, titled by its thread and marked with a violet dot, and keep, archive and restore from outside appear in the hub. Change: Batches come from sessions in this app, and outside access is through `unidex serve`. | change | M9 | automated |
| CX20 | Codex host | Brain's Bootstrap, Map and Health work with only an add-on engine or none, offering every action and naming no engine, and the working-memory card names the app's own wrap command. Change: The source names a Codex-only vault; the app names no engine and uses its own wrap command. | change | M9 | automated |
| CX21 | Codex host | A hygiene fix run from an outside agent previews without writing, then applies, and Health shows its trail rows as fixed. Change: The verbs are `unidex memory fix` and the MCP tools of `unidex serve`. | change | M9 | automated |
| CX22 | Codex host | Brain's AI helpers (Draft with AI, the sweep, Consolidate with AI) use the configured role's model and caps, and without a usable provider the dialog works by hand and the sweep reads off, naming the missing provider. Change: The source names Codex; the app uses any role's model, including an add-on engine. | change | M9 | automated |

## Vault chat (per provider)

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| CH1 | Vault chat (per provider) | Vault chat's header names the role's provider and model, an answer appears, the menu's pick replaces the default, and a spend row is written. Change: The header names the provider, not a per-CLI script and its caps. | change | M4 | automated |
| CH2 | Vault chat (per provider) | With no provider configured, Sessions shows History alone (or is hidden with no past sessions), and Vault shows a hint. Change: The state is "no provider" rather than the runtime's `none` setting. | change | M4 | automated |
| CH3 | Vault chat (per provider) | With only a local model configured, Vault chat's header says it is local and an answer comes back. Change: There is no Claude login to be absent; the local role simply answers. | change | M4 | automated |
| CH6 | Vault chat (per provider) | On a fresh install with a provider configured, Sessions appears on the rail once the check answers, with no relaunch. Change: The condition is a configured provider, not a signed-in host. | change | M4? | automated |
| CH5 | Vault chat (per provider) | With chat on a local model that is down, Sessions stays on the rail, the This Mac chip and a down line appear, and a retry on another role runs only after a click that names its cost cap. Change: The fallback is another role, not Claude Code specifically. | change | M4 | automated |
| CH4 | Vault chat (per provider) | With a hosted provider, Vault chat's header names it with its caps, and an answer shows its cost. Change: The header names the role's provider and model. | change | M4 | automated |

## Sessions (per host; each turn spends)

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| SE1 | Sessions (per host; each turn spends) | A turn in a workspace shows the prompt, Markdown text, tool lines that open to their input and result, a done line, the thread under its workspace, its session log, and the run and spend rows, and a reply resumes it. Change: The turn runs on the native engine or an add-on, and the log is our own JSONL (section 5.2). | change | M4 | automated |
| SE2 | Sessions (per host; each turn spends) | An add-on engine's turn shows its access note, command and patch lines per file, an estimated cost, and a reply resumes it. Change: The source's Codex turn is an ACP turn here. | change | M5 | automated |
| SE3 | Sessions (per host; each turn spends) | Stop in the top bar (or Escape in the tab) ends the turn as stopped, clears the running mark, and the thread accepts a reply. | carry | M4 | automated |
| SE4 | Sessions (per host; each turn spends) | Review changes opens a drawer card with a summary and a file's diff, and says so when the folder is not a repository. | carry | M4 | automated |
| SE5 | Sessions (per host; each turn spends) | Commit uses the edited message, shows refusals in the card, and never pushes. | carry | M4 | automated |
| SE6 | Sessions (per host; each turn spends) | A daily spend cap refuses a turn in the thread and no engine runs. | carry | M4 | automated |
| SE7 | Sessions (per host; each turn spends) | An engine that is off or missing is a disabled entry in the engine and model menu with the reason, and Vault chat still works. Change: The engine menu lists native roles and detected add-ons; the surface-off wording is dropped. | change | M5 | automated |
| SE8 | Sessions (per host; each turn spends) | The engine and model menu shows each engine's list and where it came from, search, older models folded, the efforts each lists, a custom id (a leading dash refused), a refresh, and a fallback to aliases when an engine does not answer. Change: The lists come from the provider catalog (section 5.1) and from ACP for add-ons. | change | M5 | automated |
| SE9 | Sessions (per host; each turn spends) | Mid-thread, the engine chip locks, New session starts on the other engine, and a model or effort change applies from the next turn with its marker and done line. | carry | M4 | automated |
| SE10 | Sessions (per host; each turn spends) | The access menu offers each engine's levels and what they run as (plan, accept edits, shell, read-only, workspace-write), with notes. Change: Levels are the app's permission modes for the native engine and mapped ACP modes for add-ons. | change | M5 | automated |
| SE11 | Sessions (per host; each turn spends) | Typing a slash lists the engine's commands and skills, filtered as you type, Enter inserts the name when it matches or was highlighted, Escape closes, and the prompt reaches the engine unchanged. Change: One slash list serves the app's skills and an add-on engine's advertised commands; there is no separate dollar list. | change | M4 | automated |
| SE12 | Sessions (per host; each turn spends) | Sessions shows the list (New session, Vault, workspaces, engine dots, running mark, age, spend), tool lines, the plan card, the working line, and a top bar with branch, change counts, Stop and status. | carry | M4 | manual |
| SE13 | Sessions (per host; each turn spends) | Vault chat's picker on each ready engine shows a fixed read-only chip and a header that follows the pick, passes the model and effort through, shows the footer, and is off while a question runs. Change: There are no per-CLI invocation details; the call goes through the provider or ACP. | change | M5 | automated |
| SE14 | Sessions (per host; each turn spends) | History lists past sessions of this app by day newest first with engine dot, title, time, engine and workspace, filters keeping focus, and Show 100 more, leaving out headless runs, team seats and sub-agents. Change: It covers sessions run in this app only (non-goal on host transcripts). | change | M4 | automated |
| SE15 | Sessions (per host; each turn spends) | History's Resume reopens a thread in its workspace, native threads in-app and add-on threads over ACP, and is disabled with the reason when its engine is off or signed out. Change: The source types per-CLI resume commands. | change | M5 | automated |
| SE16 | Sessions (per host; each turn spends) | With no engine usable, Sessions shows History alone with a no-engine line, and is not on the rail with no past sessions. Change: The condition is no provider and no add-on. | change | M4 | automated |
| SE17 | Sessions (per host; each turn spends) | Delete thread asks first, Cancel keeps it, Delete removes it, shows New session and forgets the thread's browser. | carry | M7 | automated |

## Code

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| TM1 | Code | A fresh install's Code tab opens a live shell at once with no "Terminal unavailable" message. | carry | M4 | automated |
| TM2 | Code | The new-terminal shortcut starts a session on the last engine used in the vault, shows a one-line Started note, and Scratch is chosen deliberately and made with stubs and never used as context. Change: The source starts the last host CLI; the app starts the last engine. | change | M4 | automated |
| TM3 | Code | New offers existing names to start there and creates and starts a new name, and the refusals of the new-workspace shortcut apply. | carry | M4 | automated |
| TM4 | Code | Linking a code folder in Spaces makes new terminals and new engine sessions start in that folder. | carry | M8? | automated |
| TM6 | Code | The deck groups terminals by place with a New session control, a place menu, restart or resume for an ended agent, and the next, previous and close shortcuts. | carry | M4 | automated |
| TM7 | Code | The composer works under a shell and under an agent's chat, with the focus shortcut, file mentions, slash commands, one paste, snippets, links and a draft per terminal. | carry | M4 | automated |
| TM8 | Code | The find shortcut searches the selected terminal with highlights and a count, Enter and Shift+Enter step, and Escape clears. | carry | M4 | automated |
| TM9 | Code | After a visit to Code or Agent Teams every tab keeps its layout. | carry | M10 | automated |
| TM10 | Code | Code never scrolls the whole window at full size or at 960 by 600, the composer's send and the list's foot stay inside it, and the wheel scrolls the terminal only. | carry | M4 | automated |
| TM11 | Code | Code chat is on by default, off when its write surface is left out, and the next agent terminal opens in its chat. Drop: The chat over a live terminal is not rebuilt; add-on turns are ACP sessions (section 5.7). | drop | none | not applicable |
| TM12 | Code | A turn on an add-on engine shows a prompt bubble, Markdown, tool rows, an Edited row that opens, the title, place, model name and Stop, and an engine that exits shows an ended line. Change: The view is the ACP session view, not a chat drawn over a live terminal. | change | M5 | automated |
| TM13 | Code | The switch moves between chat and terminal for the same process at the same size without restarting, and the view is remembered. Drop: There is no terminal under an ACP session to switch to. | drop | none | not applicable |
| TM14 | Code | Code chat follows clear, resume and new commands, two terminals in one folder follow their own conversations, and without the hook it says it cannot tell. Drop: It depends on a host hook and transcript matching; ACP sessions are explicit. | drop | none | not applicable |
| TM15 | Code | Code chat shows the host's status line rows, colours and links, or the footer rows mirrored from the screen. Drop: Host status lines are a non-goal. | drop | none | not applicable |
| TM16 | Code | Past add-on conversations are listed per group with the recent ones, Show more, and All history to Sessions' History, one opens read-only with Resume, and a missing record shows why. Change: The list holds this app's ACP sessions, not host transcripts. | change | M5 | automated |
| TM17 | Code | The model and access chips change the session's model and mode in place over ACP, are disabled while a turn runs with the reason, and picking the same model does nothing. Change: The source restarts a CLI with new flags; ACP can switch in the session. | change | M5 | automated |
| TM18 | Code | An add-on engine's permission request appears as a card with its own choices, a click answers once, and a dialog the card cannot show reads as waiting with a pointer to the terminal. Change: ACP permission requests map to the app's permission UI (section 5.4); there is no screen scraping. | change | M5 | automated |
| TM19 | Code | The Diff pane shows branch and counts, stays closed until opened, lists changed files with counts, shows a read-only diff that keeps its scroll while the agent edits, covers a workspace's linked folder, and shows Files without Diff when not a repository. | carry | M4 | automated |
| TM20 | Code | The Files button opens the Files screen at the workspace. | carry | M8? | automated |
| TM21 | Code | With a turn open and the agent quiet, or the chat shown and the terminal blurred, the row reads Working. Drop: The heuristic exists only to read a live terminal; ACP reports turn state. | drop | none | not applicable |
| TM22 | Code | A pasted screenshot, a dropped file or the attach button adds a thumbnail chip with a remove control, dimmed while saving, at most eight per message. Change: Images travel as message content to engines that accept them, not as terminal pastes. | change | M4? | automated |
| TM23 | Code | Sending a message with images delivers them with the text in order, and an image with no text sends alone. Change: The source types image paths into a TUI as bracketed pastes; ACP carries image content. | change | M5 | automated |
| TM24 | Code | While a permission card is pending, Send writes nothing, says to answer the card first, and keeps the typed text and image chips. Change: The guard is the app's own permission card (sections 5.2 and 5.4) rather than a shared terminal screen. | change | M5 | automated |
| TM25 | Code | Image refusals explain themselves with no chip: a TIFF or HEIC, over 10 MB, content that disagrees with its type, a ninth image, and a full images folder. | carry | M4 | automated |
| TM26 | Code | Images are kept for about three months and the images folder is capped at 2 GiB or 5,000 files, and a resumed conversation can still open them. Change: Kept images belong to the app's own sessions. | change | M5 | automated |
| TM27 | Code | Image attach appears only where a chat composer is, not in a shell terminal or an agent terminal with no chat, and a pasted image there is not taken. | carry | M4 | automated |
| TM5 | Code | An app opened from the dock or launcher still finds add-on engine adapters installed through a package manager. Change: The app resolves the login shell's PATH once at start (section 5.4). | change | M5 | manual |

## Computer pane (per host; each turn spends)

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| CP1 | Computer pane (per host; each turn spends) | In a chat turn on the native engine or an add-on, the agent opens a page in the pane, reads it and clicks, the card asks on an unmarked site, Deny leaves the page and Allow clicks once, with no engine prompt. Change: The source names Claude Code; the engine may be native or add-on. | change | M7 | automated |
| CP2 | Computer pane (per host; each turn spends) | The same flow works for a second engine, with no engine prompt. Change: The plugin and direct modes do not exist. | change | M7 | automated |
| CP3 | Computer pane (per host; each turn spends) | In a code session the pane follows the engine, shows an activity log, Take over cancels a waiting ask, and Hand back returns control. Change: The session is any engine's session. | change | M7 | automated |
| CP4 | Computer pane (per host; each turn spends) | An ask raised off screen badges Code, marks the row waiting, sends an OS notification, and the notification or the Notifications item opens the pane on it. Change: The same for any engine; the OS notification needs a hands-on check. | change | M7 | manual |
| CP5 | Computer pane (per host; each turn spends) | The plus button opens a search tab with an empty address bar, words typed in the address bar search it while an address still opens, and an agent asked to search opens results then a result with no card. | carry | M7 | automated |
| CP6 | Computer pane (per host; each turn spends) | In a fresh session the agent finds the browser tools through a status then an open call without the user naming a tool, and the browser skill is listed. Change: The check is for the own engine; add-on engines depend on the tools being advertised. | change | M7 | manual |
| CP7 | Computer pane (per host; each turn spends) | Codex finds the browser through the plugin skill or the direct skill. Drop: Host plugin skills are a non-goal. | drop | none | not applicable |
| CP8 | Computer pane (per host; each turn spends) | Back, Forward and Reload follow the tab's history, are greyed out with nowhere to go or no tab, and the log says what happened. | carry | M7 | automated |
| CP9 | Computer pane (per host; each turn spends) | A fresh code session and a thread start with the browser off (a narrow strip with a power button), the agent's first open shows a Turn on card, Turn on loads the page, and Not now tells the agent and a retry shows no card. | carry | M7 | automated |
| CP10 | Computer pane (per host; each turn spends) | The same starts-off flow works for a second engine. Change: The plugin and direct modes do not exist. | change | M7 | automated |
| CP11 | Computer pane (per host; each turn spends) | Two sessions working in the same folder each keep their own browser tabs, and neither is told the other's browser is in use. | carry | M7 | automated |
| CP12 | Computer pane (per host; each turn spends) | Off closes the tabs and stops an action in flight, closing the last tab switches it off, and On brings the tabs back reloaded. | carry | M7 | automated |
| CP13 | Computer pane (per host; each turn spends) | A web link in a terminal, a message or a thread opens in that session's browser and turns it on, a link with a user name or password is not opened, and outside a session https goes to the default browser while http opens nowhere with a notice. | carry | M7 | automated |
| CP14 | Computer pane (per host; each turn spends) | The pop-out window follows the selected session, is titled with the session's name, stays open and empty with an "(off)" title while the browser is off, and a Turn on request raised away from the pane notifies. | carry | M7 | automated |
| CP15 | Computer pane (per host; each turn spends) | After quit and relaunch a thread's browser returns asleep with its tabs, a resumed conversation gets its tabs back off, and browsers over the awake limit sleep least recently used first. | carry | M7 | automated |
| CP16 | Computer pane (per host; each turn spends) | A page a program opens (a dev server's open flag, a web login, a companion server) in a session shows in that session's browser after Turn on or adds a tab with no card when on, and opens nothing in the default browser. | carry | M7 | automated |
| CP17 | Computer pane (per host; each turn spends) | The same works for a second engine, and a sandbox that refuses the control socket shows a notice. Change: The plugin and direct modes do not exist. | change | M7 | automated |
| CP18 | Computer pane (per host; each turn spends) | Open in my browser sends the current page to the default browser and keeps the tab, and is greyed out on an http page, a blank tab and an off or asleep browser. | carry | M7 | automated |
| CP19 | Computer pane (per host; each turn spends) | The reply to a browser action already describes the resulting page, and an element ref from it works in the next call without a new snapshot. | carry | M7 | automated |
| CP20 | Computer pane (per host; each turn spends) | The agent can open a second tab, switch and close the tab it opened with no card, closing a tab the person opened asks, and closing the last tab leaves the browser on. | carry | M7 | automated |
| CP21 | Computer pane (per host; each turn spends) | Going Back, Forward or Reload on an ordinary page never shows a permission card. | carry | M7 | automated |
| CP22 | Computer pane (per host; each turn spends) | Reload after a form's result page asks and Allow sends the form again while Deny reloads nothing, and Back and Forward never ask or resend. | carry | M7 | automated |
| CP23 | Computer pane (per host; each turn spends) | The console tool shows a [request] 404 line for a page that asks for a missing file. | carry | M7 | automated |
| CP24 | Computer pane (per host; each turn spends) | Hover opens a menu and clicks nothing, and a double click reaches a control that reacts to it. | carry | M7 | automated |
| CP25 | Computer pane (per host; each turn spends) | When the browser refuses an agent's action, the message is a single sentence that also says what to do next. | carry | M7 | automated |
| CP26 | Computer pane (per host; each turn spends) | When the agent closes the last tab the browser stays on with one new tab at the start page without a card, and closing the start tab asks and Deny keeps it. | carry | M7 | automated |
| CP27 | Computer pane (per host; each turn spends) | On a Codex-only machine a code session asks for the new tools and a chat turn refuses them before an upgrade, and they run after it with no Codex prompt. Drop: It depends on an `aos upgrade` and Codex's plugin modes. | drop | none | not applicable |
| CP28 | Computer pane (per host; each turn spends) | A pointer with the session's name travels to a click, hover and typed field in the main window and pop-out, is in no screenshot or outline the agent gets, and disappears at Take over, at Off and after three seconds. | carry | M7 | automated |
| CP29 | Computer pane (per host; each turn spends) | Reload on a form's result page shows a Send this form again card, Cancel sends nothing, Send again sends once more and the log says Reloaded, and Back and Forward ask nothing. | carry | M7 | automated |

## Launch sequence

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| LS1 | Launch sequence | On first launch the app runs onboarding, then offers import of an existing vault, a What's new note and the tour, then Getting started, each once. Change: The source sequence covers an upgrade from before 2.0; a new app starts at onboarding. | change | M11 | manual |
| LS2 | Launch sequence | Go local backs up and switches an older install to the local engine, Keep my settings changes nothing, Settings can Undo it, and a fallback shows when the surface is off. Drop: It migrates an older Electron install; the app has no earlier install. | drop | none | not applicable |
| LS3 | Launch sequence | When the app supplies a new default hourly-check prompt, an edited copy is kept with a notification and an untouched one is replaced. Change: The prompt file lives in the app's vault; the rule stays. | change | M6? | automated |
| LS4 | Launch sequence | With no provider or add-on, Pulse says to add one to start the Chief of Staff with an Add button, a lapsed sign-in is worded from the skip and cleared by a sign-in or a run, and the card's box reads "when you add one". Change: The text names providers and add-ons rather than two hosts. | change | M6 | automated |
| LS5 | Launch sequence | An upgrade command asks once on a terminal and prints the commands otherwise, and the import command settles the import. Drop: There is no upgrade command; import is covered by LS6. | drop | none | not applicable |
| LS6 | Launch sequence | The Import sheet shows the dry run's counts by kind (memory, patterns, notes, daily notes, persona, routines), the proposals ticked, progress and reported conflicts, with a Not now. Change: It imports an Electron vault read-only into this app's vault with no host history. | change | M11? | automated |

## Guided tour

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| GT1 | Guided tour | A new install starts the welcome tour after onboarding, with its stops, the clicks it waits for and Getting started at the end. Change: There is no existing-install What's new path on a first release. | change | M11 | automated |
| GT2 | Guided tour | The spotlight lets clicks through only its cutout, supports arrows and Escape, moves focus to the popover, survives a redraw of the screen under it, yields to a modal and respects reduced motion. | carry | M11 | automated |
| GT3 | Guided tour | Help has five items that also appear as commands in the palette and the app menu, and Documentation opens the README. | carry | M11 | automated |
| GT4 | Guided tour | Getting started shows n of m with tours and first steps ticked from the app's state, Start and Show me, and can be hidden and brought back from Help. | carry | M11 | automated |
| GT5 | Guided tour | Opening a screen for the first time shows its intro chip once, and never again or during a tour. | carry | M11 | automated |
| GT6 | Guided tour | Every tab's tour spotlights each control without skipping any, adapts to an app with no add-on or no provider, and reads well in both themes. | carry | M11 | manual |

## Telemetry switch

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| TS1 | Telemetry switch | With the telemetry setting off no live run records or reconcile run, Runs still reads its records, and hooks record nothing. Drop: The design does not include a switch for local run recording. | drop | none | not applicable |
| TS2 | Telemetry switch | Settings from 0.17 or earlier lose the old cost and telemetry keys on first load. Drop: There are no old settings to migrate. | drop | none | not applicable |

## Review readiness

| id | source section | behaviour | fate | milestone | how verified |
|---|---|---|---|---|---|
| RR1 | Review readiness | The app binds one shortcut to Omnisearch and another to the palette, with no clash. Change: The source guarded a default-hotkey quirk of the old plugin stand-in; the app only binds its two shortcuts. | change | M4 | automated |
| RR2 | Review readiness | Pulse's disk donut and cost sparkline are drawn. Change: They are drawn natively by the UI toolkit, not as web graphics. | change | M6 | automated |
| RR3 | Review readiness | Timestamps follow the OS locale. | carry | M4 | automated |
