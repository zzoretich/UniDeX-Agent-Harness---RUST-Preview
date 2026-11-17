# Decision records

Each design decision is one file, numbered in the order it was made: `NNNN-short-title.md`. Copy
`0000-template.md`, fill in every section, and add a row to the index below. A decision is never edited to change its
meaning. To change one, write a new record and set the old one's status to "superseded by NNNN".

Paths under `spikes/` in records 0008 to 0014 refer to the M0 trials' code, which was removed at the M0 gate; it
remains in the repository's history at commit c6880ec. The same holds for the trials' temporary workflows
(`.github/workflows/spike-*.yml`) those records name. The platform trial's checklist (`spikes/platform/CHECKLIST.md`
in record 0010) now lives at `docs/acceptance/hands-on/platform-trial.md`.

| Number | Title | Status |
|---|---|---|
| [0001](0001-own-engine-acp-addons.md) | Own engine, CLIs as optional add-ons over ACP | accepted |
| [0002](0002-native-ui-gpui.md) | Native Rust UI on GPUI | accepted |
| [0003](0003-own-vault-with-import.md) | Own vault with a read-only import | accepted |
| [0004](0004-embedded-chromium.md) | Embedded Chromium, rendered off-screen | accepted |
| [0005](0005-platform-tiers.md) | Platform support in tiers | accepted |
| [0006](0006-addon-vendor-terms.md) | Add-on vendor terms (Claude Code and Codex over ACP) | proposed |
| [0007](0007-gpui-source-and-pins.md) | GPUI source and pins | accepted |
| [0008](0008-chat-and-terminal-trial.md) | Streaming chat and terminal trial | accepted |
| [0009](0009-pty-crate.md) | PTY crate | accepted |
| [0010](0010-platform-integration-trial.md) | Platform integration trial | accepted |
| [0011](0011-tray-crate.md) | Tray crate | accepted |
| [0012](0012-embedded-browser-trial.md) | Embedded browser trial | accepted with open questions |
| [0013](0013-rendering-in-ci-and-vms.md) | Rendering in CI and in virtual machines | accepted with open questions |
| [0014](0014-packaging-and-identity.md) | Packaging tool and application identity | proposed |
