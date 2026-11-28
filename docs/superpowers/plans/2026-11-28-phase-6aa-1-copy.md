# Slice 6aa-1: the "terminal" copy table

The words Tasks 3 and 4 of `2026-11-28-phase-6aa-1-terminal-ids.md` change. The rule comes from 6aa decision 10. What runs in a tab (a shell or an agent CLI) is a **terminal** on screen. **Session** stays only for an agent CLI's conversation: Resume session, History, "Go to session" inside History, a conversation's id, and what a CLI loads into each conversation.

Strings that exist only to show or explain a session number are not here. Tasks 1 and 2 remove them and set their replacements.

Each row reads `line: old → new`. Line numbers are those of `06ba7e02` and move as earlier tasks edit a file, so find the old string, not the line. An old string that isn't in the file is reported, not guessed at. Tests that assert an old string are updated to the new one in the same task.

## A. Frontend (`src/`), Task 3

**src/lib/keymap.ts**
- The `Command["group"]` union loses `"Session"`. Every `group: "Session"` becomes `group: "Terminal"`, and the commands keep their order in `COMMANDS`, so the former Session commands come first in the Terminal group.
- 55: `Start a session` → `New terminal`
- 56: `Write a prompt to the active session` → `Write a prompt to the active terminal`
- 59: `New session as administrator` → `New terminal as administrator`
- 61: `Go to the oldest waiting session` → `Go to the oldest waiting terminal`
- 62: `Close session` → `Close terminal`
- 63: `Reopen closed session` → `Reopen closed terminal`
- 64: `Rename session` → `Rename terminal`
- 65: `Resume this session` stays.
- 69: `Move session up the list` → `Move terminal up the list`
- 70: `Move session down the list` → `Move terminal down the list`
- 72: `Pause session` → `Pause terminal`
- 73: `Continue session` → `Continue terminal`
- 80: `Read the focused session's status` → `Read the focused terminal's status`
- 81: `Copy this session's recording path` → `Copy this terminal's recording path`
- 147: `Group sessions by project` → `Group terminals by project`

**src/features/palette/paletteList.ts**
- 7: `GROUP_ORDER` becomes `["Terminal", "Window", "App"]`.

**src/features/shortcuts/ShortcutSheet.tsx**
- 9: `GROUPS` becomes `["Terminal", "Window", "App"]`.
- 26: `Move the cursor to the terminal, a session or a control` → `Move the cursor to the terminal, a list row or a control`

**src/lib/adminCopy.ts**
- 7: `Pause isn't available for a session running as administrator.` → `Pause isn't available for a terminal running as administrator.`
- 9: `New session as administrator` → `New terminal as administrator`

**src/lib/navigation/machine.ts**
- 141, 144: `No session selected` → `No terminal selected`

**src/lib/navigation/names.ts**
- 1: `launcher: "New session"` → `launcher: "New terminal"`

**src/design/review/renderReview.tsx** (the design review page, a developer tool)
- 36: `while its session works` → `while its terminal works`
- 52: each `session` → `terminal` (`that terminal's colour`, `while the terminal waits on you`, `when a terminal starts working`, `for the focused terminal`)
- 53: delete the clause that says you can tell an agent "go talk to session 6", and keep the rest of the sentence.
- 54, 72, 73: these lines are too long to quote here. Read them, then apply the rule. A clause about session numbers is deleted.
- 55: `The session list in the sidebar and rail` → `The terminal list in the sidebar and rail`
- 56: `The accent follows the session's vendor.` → `The accent follows the terminal's vendor.`
- 59: delete `The footer says which session number it will take.`
- 60: rewrite with no numbers: `Open terminals come first, matched by name.`, then the line's remaining text with every numbers clause deleted.
- 63: `and the first session.` → `and the first terminal.`

**src/features/accounts/accountsText.ts**
- 35: `Sign in with /auth in a Qwen Code session.` → `Sign in with /auth in a Qwen Code terminal.`

**src/features/agents/OwnAgentsSection.tsx**
- 190: `Marshell matches a running session to the agent by these.` → `Marshell matches a running terminal to the agent by these.`

**src/features/agents/agentsState.ts**
- 55: `Not set up. Sessions show limited status.` → `Not set up. Terminals show limited status.`
- 59: `New sessions report status once Codex trusts them.` → `New terminals report status once Codex trusts them.`
- 60: `New sessions report status.` → `New terminals report status.`
- 89, 97: `Sessions started from now show limited status.` → `Terminals started from now show limited status.`

**src/features/browser/BrowserSection.tsx**
- 58: `In a WSL session, only files on a Windows drive (under /mnt) open.` → `In a WSL terminal, only files on a Windows drive (under /mnt) open.`
- 70: `in its session's Browser panel` → `in its terminal's Browser panel`

**src/features/browser/autoOpen.ts**
- 16, 17: `Not in this session: ` → `Not in this terminal: `

**src/features/capture/capture.ts**, **src/features/regions/RegionComment.tsx**, **src/features/sessions/LiveApp.tsx** (916, 1406, 1442, 1460)
- `No session to add it to.` → `No terminal to add it to.`

**src/features/changes/githubCopy.ts** 105, **src/features/sessions/LiveApp.tsx** 1432
- `No session to ask.` → `No terminal to ask.`

**src/features/changes/send.ts**
- 59: `No session to send the comments to.` → `No terminal to send the comments to.`

**src/features/companies/CompanySection.tsx**
- 314: `` `Saved. New ${name} sessions use it.` `` → `` `Saved. New ${name} terminals use it.` ``

**src/features/companies/companyFields.ts**
- 8, 16, 24, 32, 40: `The model a new <CLI> session starts with` → `The model a new <CLI> terminal starts with`
- 33: `Sessions Marshell starts pass their own mode, which wins.` → `Terminals Marshell starts pass their own mode, which wins.`
- 34: `after the session ends.` → `after the terminal ends.`
- 41: `The permission mode a Copilot CLI session starts in.` → `The permission mode a Copilot CLI terminal starts in.`

**src/features/composer/Composer.tsx**
- 56: `This session has ended` → `This terminal has ended`

**src/features/composer/StatusStrip.tsx**
- 68: `Session status` → `Terminal status`

**src/features/composer/stripItems.ts**
- 106: `` `Session running ${…}` `` → `` `Terminal running ${…}` ``

**src/features/departures/DeparturesBoard.tsx**
- 143: `Sessions` (the table's aria-label) → `Terminals`
- 170: `Other sessions · {n}` → `Other terminals · {n}`
- 180: `more session` / `more sessions` → `more terminal` / `more terminals`
- 182: `All sessions` → `All terminals`

**src/features/doctor/LiveDoctor.tsx**
- 25 (used at 251): `session` / `sessions` → `terminal` / `terminals`

**src/features/doctor/report.ts**
- 40: `"session picks", "sessions pick"` → `"terminal picks", "terminals pick"`

**src/features/history/HistoryView.tsx**
- 70 stays (Claude Code sessions in History).
- 73: `in a Claude Code session, or bookmark a turn here.` → `in a Claude Code terminal, or bookmark a turn here.`

**src/features/instructions/DocList.tsx**
- 65: `Open a session in a folder,` → `Open a terminal in a folder,`

**src/features/launcher/Launcher.tsx**
- 352: `Start a session` → `New terminal`

**src/features/notifications/NotificationsSection.tsx**
- 81: `Open sessions from the notification centre` → `Open terminals from the notification centre`
- 84: `select a session.` → `select a terminal.`

**src/features/onboarding/Detected.tsx**
- 34: `Marshell runs it as a terminal session for now.` → `Marshell runs it as a terminal for now.`

**src/features/onboarding/FirstSession.tsx**
- 10: `Enter starts your first session.` → `Enter starts your first terminal.`

**src/features/onboarding/NotificationsStep.tsx**
- 22: `when a session needs you.` → `when a terminal needs you.`

**src/features/onboarding/emptyStates.ts**
- 10: `No sessions yet` → `No terminals yet`
- 10, 12, 17: the action `Start a session` → `New terminal`
- 12: `Finished sessions are kept here, with their transcript.` stays (History).
- 17: `No sessions on the board` → `No terminals on the board`

**src/features/onboarding/onboardingFlow.ts**
- 7: `First session` → `First terminal`

**src/features/palette/Palette.tsx**
- 245: `No session or command matches` → `No terminal or command matches`
- 269: `Open sessions` → `Open terminals`
- 314: `Sessions and commands` → `Terminals and commands`

**src/features/peek/Peek.tsx**
- 68: `Can't read this session's screen` → `Can't read this terminal's screen`

**src/features/perf/perfLines.ts**
- 27: `` `all tabs ${n} sessions · …` `` → `` `all tabs ${n} terminals · …` ``

**src/features/regions/deliver.ts**
- 28: `The session has ended.` → `The terminal has ended.`

**src/features/sessions/LiveApp.tsx**
- 462: `Group sessions by project` → `Group terminals by project`
- 1499: `"the session"` → `"the terminal"`
- 2875: `"the session"` → `"the terminal"`

**src/features/sessions/launchPrograms.ts**
- 306: `Couldn't start the session: ` → `Couldn't start the terminal: `

**src/features/sessions/resumeActions.ts**
- 16: `End the session first.` → `End the terminal first.` (the core's `server/outside.rs` says the same; part B)

**src/features/sessions/sessionActions.ts**
- 58: `Reopen the closed session as administrator? Antigravity may skip every prompt, so this session may run in Bypass.` → `Reopen the closed terminal as administrator? Antigravity may skip every prompt, so this terminal may run in Bypass.`
- 329, 347: `Couldn't reopen the session: ` → `Couldn't reopen the terminal: `
- 375: `Couldn't resume the session: ` stays (Resume continues a conversation).
- 480: `Couldn't restart the session: ` → `Couldn't restart the terminal: `
- 521, 523: `Couldn't pause the session: ` → `Couldn't pause the terminal: `
- 534, 536: `Couldn't continue the session: ` → `Couldn't continue the terminal: `
- 577: `Couldn't rename the session: ` → `Couldn't rename the terminal: `
- 591: `Couldn't move the session: ` → `Couldn't move the terminal: `

**src/features/settings/DefaultFolderRow.tsx**
- 16: `new sessions start` → `new terminals start`
- 19: `so shells and sessions start in your home folder.` → `so shells and terminals start in your home folder.`

**src/features/settings/ProfilesSection.tsx**
- 33: `How the profile is listed when you start a session.` → `How the profile is listed when you open a terminal.`
- 35: `Where sessions start. Empty means the default folder.` → `Where terminals start. Empty means the default folder.`
- 76: `Set in every session started from this profile,` → `Set in every terminal started from this profile,`
- 98, 134, 159: `Applies to new sessions.` → `Applies to new terminals.`

**src/features/settings/ProjectsSection.tsx**
- 11: `Start a session in a folder first; its project shows here.` → `Open a terminal in a folder first; its project shows here.`

**src/features/settings/SettingsShell.tsx**
- 38: `Back to sessions` → `Back to terminals`

**src/features/settings/fields.ts**
- 28: `Applies to new sessions.` → `Applies to new terminals.`; `Applies after you restart the session.` → `Applies after you restart the terminal.`

**src/features/sidebar/RenameBox.tsx**
- 30: `Session name` → `Terminal name`

**src/features/tasks/reviewOpen.ts**
- 12: `This session isn't a task.` → `This terminal isn't a task.`

**src/features/terminal/coreRestart.ts**
- 2: `Session ended. Close it with Ctrl+Shift+W.` → `Terminal ended. Close it with Ctrl+Shift+W.`

**src/features/tray/trayItems.ts**
- 31: `Go to the oldest waiting session` → `Go to the oldest waiting terminal`

**src/features/window/AllSessionsMenu.tsx**
- 14: `All sessions` → `All terminals`
- 20: `session` / `sessions` (the count) → `terminal` / `terminals`

**src/features/window/CloseDialog.tsx**
- 25: `so your sessions keep going.` → `so your terminals keep going.`

**src/features/window/MainWindow.tsx**
- 467: `Search sessions, projects and commands` → `Search terminals, projects and commands`
- 487: `The side pane shows beside a session` → `The side pane shows beside a terminal`
- 558: `No sessions yet. Start one with` → `No terminals yet. Start one with`

**src/features/window/NavCursor.tsx**
- 13: `No session selected · Tab to select a terminal` → `No terminal selected · Tab to select a terminal`

**src/features/window/Rail.tsx**
- 39: `Sessions` (aria-label) → `Terminals`

**src/features/window/ShellPanel.tsx** (the button moves the panel's shell into the sidebar list)
- 19: `Couldn't open the shell as a session.` → `Couldn't move the shell to the list.`
- 70: `` `Open ${shellName} as a session` `` (aria-label and title) → `` `Move ${shellName} to the list` ``
- 18, 49 and every other "Open as a session" a person reads in this file or in `LiveApp.tsx` (1572, 1628, 1645, 1655) → `Move to the list`, or the matching sentence with "move … to the list".

**src/features/window/Sidebar.tsx**
- 257: `Sessions and approvals` → `Terminals and approvals`
- 262: `Sessions` → `Terminals`
- (290, the row menu's `` `Session ${name}` ``, is Task 2's.)

**src/features/window/announce.ts**
- 39, 46: `No session selected` → `No terminal selected`
- 53: `1 session needs you` / `` `${count} sessions need you` `` → `1 terminal needs you` / `` `${count} terminals need you` ``

**src/features/pane/paneCommands.ts**
- 13: `No session to open it in.` → `No terminal to open it in.`
- 28: `` `This session has ${PANE_LIMITS.tabs} tabs. Close one to open another.` `` → `` `This terminal has ${PANE_LIMITS.tabs} tabs. Close one to open another.` ``

**src/features/doctor/LiveDoctor.tsx**
- 25: `plural` gives `session` / `sessions` → `terminal` / `terminals` (so `Restarted 2 terminals.`)

**src/features/doctor/report.ts**
- 40: `` `${n} session picks` `` / `` `${n} sessions pick` `` `this up after a restart` → `terminal picks` / `terminals pick`

**Stays "session" in `src/`:** History and its "Go to session" (`ConversationView.tsx:287`, `LiveHistory.tsx:82`), `Resume this session`, `Couldn't resume the session: `, the History empty state, and `src/lib/api/sessions.ts` doc comments.

## B. Rust (`crates/core`, `crates/host` tests, `src-tauri`), Task 4

**crates/core/src/pty/elevated.rs**
- 22: `This session runs as administrator, so it takes input only from Marshell's own window.` → `This terminal runs as administrator, so it takes input only from Marshell's own window.`
- 25: `Only Marshell's own window can start a session as administrator.` → `Only Marshell's own window can start a terminal as administrator.`
- 28: `This session runs as administrator, so only Marshell's own window can start it.` → `This terminal runs as administrator, so only Marshell's own window can start it.`
- 32: `A session running as administrator can't keep an always-allow rule from Marshell. Allow this once instead.` → `A terminal running as administrator can't keep an always-allow rule from Marshell. Allow this once instead.`
- 35: `Not an administrator session.` → `Not an administrator terminal.`
- 53 and 857: `Another program connected to the administrator session's pipe, so it was refused. Nothing was started.` → `Another program connected to the administrator terminal's pipe, so it was refused. Nothing was started.`
- 64: `The administrator session could not be set up: {e}` → `The administrator terminal could not be set up: {e}`
- 146: `The administrator session is not taking input right now.` → `The administrator terminal is not taking input right now.`
- 177: `The administrator session is closed.` → `The administrator terminal is closed.`

**crates/core/src/pty/freeze.rs**
- 19: `Only a running session can be paused.` → `Only a running terminal can be paused.`
- 21: `Pause isn't available for a session running as administrator.` → `Pause isn't available for a terminal running as administrator.`

**`No such session.` → `No such terminal.`** in `server/fill.rs` (164, 190, 197), `server/mod.rs` (1247), `server/opens.rs` (248), `server/outside.rs` (220, 348, 359) and `server/sessions.rs` (991, 1009, 1030, 1310, 1443, 1564). `crates/cli/src/open.rs` (166–167) matches this text to rewrite it, so Task 4 changes the matched text there to `No such terminal.` too, along with its `Fake`'s reply in the `open.rs` tests. What the CLI prints is left to Task 6.

**`There is no such session.` → `There is no such terminal.`** in `server/changes.rs` (203, 262, 326) and `server/github.rs` (438, 516, 623, 799, 1036, 1045, 1068).

**`This session has no folder.` → `This terminal has no folder.`** in `server/changes.rs` (206), `server/files.rs` (958), `server/search.rs` (584) and `server/tree.rs` (304).

**crates/core/src/server/github.rs**
- 551: `The session moved to another folder. Look again before you go on.` → `The terminal moved to another folder. Look again before you go on.`
- 554: `Git isn't installed, or isn't on the PATH a session gets.` → `Git isn't installed, or isn't on the PATH a terminal gets.`

**crates/core/src/server/outside.rs**
- 22: `End the session first.` → `End the terminal first.`
- 72: `A {name} session needs Marshell to start it, so there is no command to copy.` → `A {name} terminal needs Marshell to start it, so there is no command to copy.`
- 73: `A {name} session needs Marshell to start it.` → `A {name} terminal needs Marshell to start it.`

**crates/core/src/server/sessions.rs**
- 682: `The session was closed while it was starting.` → `The terminal was closed while it was starting.`
- 1040: `The list must hold every open session exactly once.` → `The list must hold every open terminal exactly once.`
- 1081: `This session has no recording.` → `This terminal has no recording.`
- 1492: `Only an agent session has a mode.` → `Only an agent terminal has a mode.`

**crates/core/src/tasks.rs**
- 1144: `Closes 1 session running in this worktree.` → `Closes 1 terminal running in this worktree.`
- 1145: `Closes {n} sessions running in this worktree.` → `Closes {n} terminals running in this worktree.`

**crates/core/src/history/recorder.rs**
- 53: `Bookmarks work in Claude Code sessions for now.` → `Bookmarks work in Claude Code terminals for now.`

**Stays "session" in Rust:** `history/export.rs` (`- Session: `), `history/index.rs`, `server/history.rs` (`Use Go to session.`), what the Doctor says a CLI loads into each conversation (`doctor/ext.rs:718`, `doctor/measure.rs:119` and `:263`, `doctor/usage.rs:495` and `:503`), `agents/antigravity/instructions.rs` (24, 552: Antigravity loads a file into a conversation), `agents/codex/mod.rs:51` (`your other Codex sessions`: Codex's own sessions, outside Marshell too) and `crates/host/src/main.rs:19` (Windows logon sessions).

Tests that assert a changed string move with it, including `crates/core/tests/*.rs` and `crates/host/tests/elevated_session.rs`. The tray's `Go to the oldest waiting session` in `src-tauri/src/tray.rs` tests becomes `Go to the oldest waiting terminal`, as in part A.
