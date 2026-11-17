# Platform trial: the maintainer's checklist (macOS)

What a program cannot check on itself: typing through a real input method and real dead keys, clicking the tray icon,
the menus and dialogs as a person sees them, and what VoiceOver actually says. Fourteen steps, about 15 minutes.
Results feed decision 0010 (`docs/decisions/0010-platform-integration-trial.md`), which lists what the self-check
already verified.

Before starting: Terminal in the repository, `cd spikes/platform`. Steps 3 to 5 need a Japanese or Chinese input
source; if none is installed, add one in System Settings, Keyboard, Text Input, Edit, "+" (and remove it afterwards
the same way). Step 6 assumes the U.S. or ABC layout.

| # | Do this | Expected result |
|---|---|---|
| 1 | `cargo run --release --locked -- --selfcheck` (the first build takes a few minutes) | Windows open and close within about 10 seconds; a blue circle appears in the menu bar and disappears; the last line is `selfcheck: 35 checks, 0 failed`; `echo $?` prints `0`. (A FAIL line names what broke. The 35 was counted with three displays attached; with one display the check that opens a window on a second display is replaced by an informational line that is not counted, so a single-display Mac should see 34 checks. Every line should read PASS either way.) |
| 2 | `cargo run --release --locked` | A window titled "UniDeX platform trial" with Name and Notes fields and five buttons; a blue circle in the menu bar; the menu bar shows "UniDeX Trial", "File", "Trial". The caret blinks in Name. |
| 3 | Switch to the Japanese (Romaji) or Pinyin input source. In Name, type `kanji` (Pinyin: `hanzi`), press Space, then Return. | While typing, the text is underlined and the candidate list appears just below the typed text, not in a screen corner. Return commits 漢字 (or 汉字); the underline goes; "last input call" ends with `replace_text_in_range(..., "漢字")`. |
| 4 | Click in Notes; type a word with the input method, press Return to commit it, then press Return again. | The first Return only commits (no new line); the second Return starts a new line. The candidate list follows the caret onto the second line when you type there. |
| 5 | In either field, type `nihon` and press Escape before committing. | The underlined text disappears and nothing is inserted; "last input call" shows `unmark_text`. Switch back to the U.S. or ABC input source. |
| 6 | In Name: Option-E then E; Option-U then O; Option-\` then A; Option-N then N. Repeat in Notes. | Name shows `éöàñ`; the pending accent is shown highlighted until the second key. The same in Notes. |
| 7 | Hold the E key for two seconds in Name, then press 2 (or let go, if press-and-hold is off). | Either the accent picker appears and 2 inserts ê, or the key repeats (`eeee`), whichever this Mac's keyboard setting chooses; no crash, no stuck key. |
| 8 | Click the blue circle in the menu bar. Press Escape. Right-click the circle and choose "Show popover". | A small "Tray popover" window opens directly below the circle; Escape closes it; the right-click menu shows "Show popover" and "Quit the trial", and its first item opens the popover again. |
| 9 | Use the menu bar: Trial, Say Hello. Then press Cmd-Shift-H. Then Cmd-Shift-P. | Status reads "Say Hello ran (1)", then "(2)"; Cmd-Shift-P opens the popover below the circle. The menu items show their shortcuts (Cmd-O, Cmd-S, Cmd-N, Cmd-Shift-P, Cmd-Shift-H). |
| 10 | Press Cmd-N. Drag the new window around, and to another display if there is one. Click "Close this window". | A "Second window" opens showing "Open windows: 2"; its bounds update as it moves, and its display id changes on the other display. It closes; the main window stays. |
| 11 | Press Cmd-O, choose any file, then press Cmd-O again and Cancel. | A native macOS open panel; the status shows `Open dialog returned ["/path/to/file"]`, then `Open dialog cancelled`. |
| 12 | Press Cmd-S, keep the name `trial-note.txt`, click Save. | A native save panel; the status shows `Save dialog returned ".../trial-note.txt" (nothing written)`; no file is created. |
| 13 | Turn VoiceOver on (Cmd-F5). Click in the window, then move with Control-Option-Right through it; at Name, type `hi`; at Count, press Control-Option-Space. Turn VoiceOver off (Cmd-F5). | VoiceOver names the window "UniDeX platform trial"; reads the heading and the text lines; says "Name, edit text" with its contents and echoes the typed letters; says "Count, button" once (not twice) and the status changes to "Count pressed 1 times"; the line "Plain string child (not published)" is skipped. Note anything it reads wrongly. |
| 14 | Quit with Cmd-Q, then run `pgrep -fl platform-trial`. | The window and the menu bar circle disappear; `pgrep` prints nothing. |

## Optional: the `unidex://` link

This registers the trial's bundle with LaunchServices on this Mac. Undo it with the last command.

1. `./make-bundle-macos.sh` builds `target/UniDeX Platform Trial.app` (Info.plist from `bundle/`, ad hoc signature).
2. `open "target/UniDeX Platform Trial.app"` starts it; opening the bundle registers its `unidex` scheme. Then
   `open "unidex://thread/42?from=checklist"`. Expected: the running window's status shows
   `Link received: ["unidex://thread/42?from=checklist"]`, and `pgrep -fl platform-trial` lists one process (the
   link went to the running instance). Quit with Cmd-Q, run the same `open "unidex://..."` again: the app starts and
   shows the link. If the link opens nothing, register explicitly with
   `/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$PWD/target/UniDeX Platform Trial.app"`.
3. Undo: quit the app, then
   `/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -u "$PWD/target/UniDeX Platform Trial.app"`
   and `rm -rf "target/UniDeX Platform Trial.app"`.
