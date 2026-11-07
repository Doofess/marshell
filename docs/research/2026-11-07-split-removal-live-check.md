# Split removal live check (slice 6q-1)

- Date: 2026-11-07, 16:58 to 17:10 local time.
- Build: `phase-6q-1` at 4447064 under `pnpm tauri dev` on Windows 11 Pro (10.0.26300).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `MARSHELL_CLAUDE_SETTINGS` under `%TEMP%\lc-6q1-1`, with CDP on port 9333. Onboarding was skipped for every agent ("Skip: use limited status" for Claude Code, Codex, Antigravity CLI, Qwen Code and Copilot CLI), so no hooks were installed. `~/.claude/settings.json` kept its modified time (04:42:51).
- Data: a hand-written `mh\layout.json`, saved right after `live-check.ps1 start` and before the core first read it. It holds three plain PowerShell tabs, all with profile `pwsh` and cwd the scratch `proj` folder:
  - `seed-alpha` (1, "alpha"), `seed-beta` (2, "beta") and `seed-gamma` (3, "gamma");
  - a view of the old split shape: `"panes":["seed-alpha","seed-beta","seed-gamma"]`, `"active":2`, `"arrangement":"columns"`, `"ratios":[0.5]`.

  The check also started three sessions of its own, all in the scratch `proj` folder:
  - gamma, resumed from its row menu;
  - session 4, the shell panel's shell, opened as a session;
  - session 5, a Claude Code session from the launcher. It stopped at Claude's first-run theme prompt and was never answered.
- How it was driven:
  - **Input:** CDP evaluations, CDP keys (the held Ctrl+Shift chord through Playwright's `keyboard.down`/`up`), and CDP mouse clicks, right-clicks and drags, inside the webview only.
  - **Forced colours:** emulated, not a real high-contrast theme.
  - **Screenshots:** two, in `.playwright-mcp/` outside the repo: the preview beside the terminal, and the same in forced colours.

## Results

| Check | Result |
| --- | --- |
| The app opens on pane 2's session, and the other two are rows. | Pass. The title read "gamma — Marshell". Rows 1–3 showed "Ended last time", and only row 3 had `aria-selected="true"`. The window held one `[data-nav-pane]`. Until the first view change the file kept the seeded split, as expected: `layout::load` never writes. |
| After any view change, the file on disk holds `"panes":["<id>"]`. | Pass. After a Ctrl+click on beta the file read `"panes":["seed-beta"]`, `"arrangement":null`, `"ratios":[]`, `"active":0`, `"zoomed":null`. After "Open PowerShell as a session" it read `["t003eb02c911c"]` with `active` 0. |
| No layout button shows, and Ctrl+Shift+D and Ctrl+Shift+P run nothing. | Pass. The top bar's buttons were "Marshell menu", "Command palette", "Sidebar" and the three window buttons, and its nav targets were `palette` and `sidebar`. A bubble-phase listener on `window` saw both keys arrive with `defaultPrevented` false. The title, the pane count and the focus stayed the same, and no dialog or menu opened. |
| Ctrl+click and Ctrl+Space on a row select it. | Pass. Ctrl+click on beta's row selected it ("beta — Marshell", one pane). Ctrl+Space on alpha's focused row selected alpha, and focus stayed on the row. |
| The row menu has no "Split into a pane", and the palette has no Split, Zoom or layout rows. | Pass. An ended plain shell's menu: "Rename F2", "Resume", "Mute notifications", "Close Ctrl+Shift+W". A running shell's menu: "Rename", "Pause", "Mute notifications", "Close". Claude Code's menu: "Rename", "Copy resume command", "Open in system terminal", "Mode ▸", "Pause", "Bookmark", "Mute notifications", "Close". In the palette, "zoom", "layout", "unsplit" and "arrange" listed nothing. "split" listed only "Move session up the list", a fuzzy match. "pane" listed "Show or focus the shell panel" and "Settings: Appearance…". "select the terminal" listed "Select the terminal Ctrl+Shift+Tab"; running that row closed the palette and put focus in gamma's terminal input. |
| Ctrl+Shift+Tab in the terminal, then Ctrl+Shift+Left, puts the cursor on the session's row. | Pass. With focus in gamma's terminal, the held chord took Tab, and no hint showed yet. After Left the `.nav-cursor` box was exactly row 3's box (`0,180,287,56`), and the hint read "Session 3, gamma" with "Release Ctrl+Shift to select · Backspace for none". Releasing selected gamma. Backspace, then a release, selected nothing and showed "No session selected · Tab to select a terminal". |
| The Preview divider drags, the arrow keys move it, and it shows its focus ring. | Pass. The preview was opened on `notes.md` with "Preview" › file. The divider is `role="separator"`, "Resize the preview", `aria-valuenow` 50 in 30–70, `tabindex="0"`. Shift+Tab reached it, with a `:focus-visible` outline of `solid 2px`. Left, Left, Right gave 48, 46, 48. A drag moved it from 48 to 35, with the accent fill while dragging. Dragging past either end stopped at 70 and at 30. In emulated forced colours it is `CanvasText`, and its keyboard-focus outline is `solid 2px` in Highlight. |
| The restart and Pause bars show in the top bar for a session picked from the row menu. | Pass. With alpha selected, "Pause" in gamma's menu selected gamma and showed, in `header.window__bar > .window__bar-main`, "Pause 3 gamma? It freezes the program and everything it started. Network connections can time out while it's paused.", with focus on "Cancel". With gamma selected, Claude Code's "Mode ▸ Plan" selected session 5 and showed, in the same place, "Restart 5 Claude Code in Plan? It ends and runs claude --continue --permission-mode plan.", with focus on "Cancel". Both were cancelled. |
| The shell panel's "Open PowerShell as a session" selects the new session. | Pass. Row 4, "PowerShell 7", appeared and was selected ("PowerShell 7 — Marshell"), with one pane still. The panel's button stayed enabled for its next shell. |
| Focus mode and the rail still work. | Pass. "Sidebar" collapsed to the rail, which put the pane at x 52 and 1148 wide. A rail click on beta selected it (`aria-current="true"` on that item only). "Sidebar" again restored the expanded list. "Focus mode" from the palette hid both, giving the pane `0..1200`, and Ctrl+Shift+I brought the expanded sidebar back. |

## Findings

1. **Ctrl+Shift+T opens the launcher without moving focus into it.** It happened from a session's terminal and right after the palette closed.
   - What happens: focus stays where it was (the terminal input, or the palette button), and no `focusin` fires. Esc then goes to that control, so it doesn't close the launcher.
   - Likely cause, read from the code but not verified: `LiveLauncher`'s first focus aims at `.launcher__prompt`, else `.launcher__start`. While the shell's agent answer is still out there is no prompt and Start is disabled, so that `focus()` does nothing. When the answer lands, `refocus` returns null, because the picks are unchanged and there is a prompt now. `lostFocus` only steps in when focus is on `body`.
   - Not this slice: this branch changes none of `LiveLauncher.tsx`, `launchPrograms.ts`, `Launcher.tsx` or the focus guards (`git diff 6185a67..HEAD` on them is empty). The last change there was 71ff3d5.
   - Not fixed. It is a lead for maintenance.
2. **Hovering an ended row logs a 404.** The peek asks `/v1/sessions/seed-beta/peek?lines=30` and gets a 404, which shows as "Failed to load resource" in the console. Nothing breaks. This slice didn't touch it. Not fixed.
3. **Ctrl+Shift+I logs a WebView2 refusal.** Focus mode's key is also WebView2's DevTools toggle, and the console logs "capability: default, permission: deny-internal-toggle-devtools". Focus mode itself works. This slice didn't touch it. Not fixed.
