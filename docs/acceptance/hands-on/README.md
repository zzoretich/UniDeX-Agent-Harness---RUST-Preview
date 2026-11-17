# Hands-on procedures

These are the steps a person takes on the Mac, for checks no program can make on itself, kept from the M0 trials after
their code was removed from `main` at the M0 gate: [`platform-trial.md`](platform-trial.md) is the platform trial's
checklist (input methods, dead keys, the tray, menus, dialogs, VoiceOver; decisions 0010 and 0011), and
[`browser-trial.md`](browser-trial.md) is a first look at the embedded browser (decision 0012), which waits for the
maintainer's explicit word because Chromium contacts its vendor on every start. Each says how to check out the trial
from commit c6880ec into a scratch folder and how to remove it. The two other hands-on checks of the gate are described
elsewhere: the cookbook window check in the cookbook's README ([`crates/udx-cookbook/README.md`](../../../crates/udx-cookbook/README.md),
"Checking the examples by hand"), and the nightly package check in
[decision 0014](../../decisions/0014-packaging-and-identity.md) ("Maintainer's step: the macOS package by hand").
