# Screen-reader pass (about 10 minutes)

Run this once per sub-review sign-off, with **Narrator** (Windows, built in) and again with **NVDA** (free, nvaccess.org). Real speech catches what no markup test can: things read twice, things never read, names that do not match what is on screen.

Open the review page (`pnpm review:build <out.html> 2c`, then the published link or a local server) in Edge or Chrome. Turn on the screen reader. Browse by Tab and arrow keys; use Insert+Space (NVDA) or Caps Lock+Space (Narrator) to leave browse mode where a step says "enter focus mode".

Tick a box only when you heard the phrase (small wording differences between Narrator and NVDA are fine; a missing or doubled phrase is a fail). Write what you heard next to any fail.

| Key | Narrator | NVDA |
| --- | --- | --- |
| Start / stop | Win+Ctrl+Enter | Ctrl+Alt+N |
| Stop reading | Ctrl | Ctrl |
| Read current item | Caps Lock+Enter | NVDA+Up |

## A. Window (sub-review B)

Go to the **Main window** section, the first dark window.

| # | Do | You should hear | Narrator | NVDA |
| --- | --- | --- | :-: | :-: |
| A1 | Tab into the sidebar list | "Sessions, list" then a session by number: "Session 3, api-server …" (number first, then name, project, what it is doing, the agent and its state) | ☐ | ☐ |
| A2 | Arrow Down through rows | Each row once, number first; the selected row says "selected" | ☐ | ☐ |
| A3 | Reach the lane | "Needs you · 2" as a heading; the targeted card reads who is asking, what for and how long it has waited | ☐ | ☐ |
| A4 | Move focus to another session (click a different row, or use the navigation chord + Tab) | Exactly once: "Session N, name" (and, if it is waiting, what it needs: "Session 1, billing is waiting for your approval") | ☐ | ☐ |
| A5 | Look at a split view with **two panes waiting** (Split view stories) | Nothing is read just because they are on screen. When focus moves to one of them: one announcement, not two | ☐ | ☐ |
| A6 | Press the "Read the focused session's status" shortcut (Ctrl+Shift+G, ⌘G on a Mac) | "Session 3, api-server: Working. Running npm test", every time you press it, even twice in a row | ☐ | ☐ |
| A7 | Clear the selection (navigation chord + Backspace) | "No session selected" | ☐ | ☐ |
| A8 | Tab to a pane's zoom button, press Space twice | The name stays "Zoom <session name>" both times; the state is spoken as pressed / not pressed | ☐ | ☐ |
| A9 | Tab to the shell panel's hide button | "Shell panel, button, expanded" then "collapsed" after Space; the name does not change | ☐ | ☐ |
| A10 | Tab through the status bar | The first thing is the session's number and name, then its facts; nothing is announced on its own | ☐ | ☐ |
| A11 | Read the empty window ("No sessions" story) | "No sessions yet. Start one with Ctrl+Shift+T." once, as plain text | ☐ | ☐ |

Pass rule for A: nothing is announced when the page loads, **no phrase is spoken twice for one change**, and "working", "thinking" or tool progress is never spoken.

## B. Launcher and palette (sub-review C)

Go to the **Launcher** section.

| # | Do | You should hear | Narrator | NVDA |
| --- | --- | --- | :-: | :-: |
| B1 | Tab to the first chip | "Program: Claude, combobox, collapsed" | ☐ | ☐ |
| B2 | Open it (Space), arrow down | The groups "Agents", "Shells", "Tools", then each program by name | ☐ | ☐ |
| B3 | Pick a shell (PowerShell) | The sentence changes; there are no mode, pace or worktree chips to Tab to | ☐ | ☐ |
| B4 | Tab to the prompt field | "Run a command first (optional)" (agents: "What should Claude do first? (optional)") | ☐ | ☐ |
| B5 | Type a folder that does not exist | "No folder matches …" once | ☐ | ☐ |
| B6 | Press Esc in the folder field | Focus returns to the folder chip, and Esc again closes the sheet | ☐ | ☐ |

Go to the **Command palette** section.

| # | Do | You should hear | Narrator | NVDA |
| --- | --- | --- | :-: | :-: |
| B7 | Focus the search field, type `6` | The option for session 6 is read first: "Go to session 6, scratch …" with its shortcut and state | ☐ | ☐ |
| B8 | Type `#12` | "Go to session 12 …" (no shortcut: only 1–9 have one) | ☐ | ☐ |
| B9 | Type `4` (a number nobody holds) | "No session 4. Open sessions: 1, 2, 3 …" | ☐ | ☐ |
| B10 | Arrow through the results | One option at a time, each with its group; the active one says "selected" | ☐ | ☐ |

## C. Approve card (sub-review A, if you touched it)

| # | Do | You should hear | Narrator | NVDA |
| --- | --- | --- | :-: | :-: |
| C1 | Tab to a card | Who is asking, what for, the exact payload; risky cards say why before the buttons | ☐ | ☐ |
| C2 | Press the buttons inside the first half-second | Nothing happens (the buttons are inert for the first 500 ms), then they work | ☐ | ☐ |
| C3 | Decide | The receipt is read once ("Allowed · npm test") | ☐ | ☐ |

## D. Batch 3 A: onboarding

Go to the **Onboarding** section.

| # | Do | You should hear | Narrator | NVDA |
| --- | --- | --- | :-: | :-: |
| D1 | Tab through the Detected list | Each row as "Claude Code, version 2.1.296" with the whole config path; a missing one says "Not installed" | ☐ | ☐ |
| D2 | Tab to a consent switch | "Approve from anywhere, switch, on" (or "off"); an unavailable switch is read as unavailable, with its reason as the description | ☐ | ☐ |
| D3 | Tab to View exact changes and press Enter | The button is read as expanded; the diff is a figure labelled "Exact changes to <path>", and added and removed lines say "added" and "removed" | ☐ | ☐ |
| D4 | Tab through the sounds | "Play Needs you sound", "Play Escalation sound", and so on, without reading the triangle | ☐ | ☐ |
| D5 | Tab into the first-session launcher | The sheet is read as the launcher, and Enter starts a session | ☐ | ☐ |
| D6 | Move between the four steps | The current step is read as "current step"; nothing is announced on its own | ☐ | ☐ |

## E. Batch 3 B: doctor and settings

Go to the **Doctor** section, then **Settings**.

| # | Do | You should hear | Narrator | NVDA |
| --- | --- | --- | :-: | :-: |
| E1 | Tab through the Doctor rows | Each row as "Warning, Approve from anywhere is not installed" (severity word first), then what it means and the file | ☐ | ☐ |
| E2 | Open the fix sheet (Fix 4 issues) | The dialog's title is read on open ("Fix 4 issues"), and focus is on the title, not on a button | ☐ | ☐ |
| E3 | Tab through the fix sheet | The diff is a figure labelled "Changes to <path>"; Cancel, then Fix 4 issues; nothing is applied by Enter on the title | ☐ | ☐ |
| E4 | Apply the fix (the story's Fixed state) | The receipt is said once: "Fixed 4 issues. 2 sessions pick this up after a restart." and nothing is read twice | ☐ | ☐ |
| E5 | Tab to a scope badge in Settings | "Managed" with its description "Set by your organisation. Marshell can't change it." | ☐ | ☐ |
| E6 | Tab to a locked value | It is read as unavailable, followed by the reason | ☐ | ☐ |
| E7 | Move through "Edit as" | "Edit as, radio group, Form, selected"; arrows move between Form and Raw | ☐ | ☐ |
| E8 | In Raw mode with a mistake, Tab into the text area | The line and column of the mistake are read as the description ("Line 3, column 23: ...") | ☐ | ☐ |

## F. Batch 3 C: Departures, banners and the tray

Go to the **Departures** section, then **Notifications**, then **Tray and icon**.

| # | Do | You should hear | Narrator | NVDA |
| --- | --- | --- | :-: | :-: |
| F1 | Tab into the board and arrow down to a gate row | "Session 3, api-server, needs you, ..." the whole row read from its cells, with the session number first, and "Gate calls" as the group | ☐ | ☐ |
| F2 | Press Y on a row that needs you | The request opens inline under the row and is read as an approve card; Enter on the row jumps to the session and never answers it | ☐ | ☐ |
| F3 | Press Replay animations | Nothing is announced: the flipping cells are not a live region; the new status is read only when you move to the row | ☐ | ☐ |
| F4 | Read a banner (Notifications section) | The title, then the body ("api-server needs you. Wants to run npm test"), then the buttons Allow and Deny (Linux: Open); a risky one reads Review only | ☐ | ☐ |
| F5 | Move through the tray menu | "Open Marshell, menu item", then each waiting session with its number, "Go to the oldest waiting session, Ctrl+Shift+N", and the disabled line as unavailable | ☐ | ☐ |
| F6 | Move to the tray icon | "Marshell, 2 need you" as one image, with no drawing details read | ☐ | ☐ |

## Recording the result

In the review page's sign-off, answer with the table's letters (A to F): for example **"Screen reader: A pass, B fail (B6: focus went to the page, not the chip), C pass, Narrator and NVDA"**. A fail is a bug to fix before the sub-review is signed off, not a note for later. Put anything surprising (a phrase that is correct but long or odd) in `docs/IDEAS.md`.
