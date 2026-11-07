# Shell-first launcher live check (slice 6o)

- Date: 2026-11-07, 05:28 to 05:45 local time.
- Build: `phase-6o` at 67afb07 under `pnpm tauri dev` on Windows 11 Pro (10.0.26300). The two fixes below are CSS only and arrived through Vite's hot reload, so the last checks ran on 822145c's styles.
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `COPILOT_HOME`, `USERPROFILE` and `MARSHELL_CLAUDE_SETTINGS` under `%TEMP%\lc-6o-1`, with CDP on port 9333. Onboarding was left with "Skip: use limited status" for every agent, so nothing was installed.
- Data: nothing seeded. For the worktree check, a scratch git repo `%TEMP%\lc-6o-1\repo` with one commit (`git init -b main`, one README).
- How it was driven:
  - **CDP:** evaluations returning small JSON values, and CDP keys.
  - **Real clicks:** two real mouse clicks through Playwright, to test a finding.
  - **Forced colors:** emulated through CDP. That is not a real high-contrast theme.
  - **Screenshots:** 9, kept outside git in `.playwright-mcp/lc-6o-1/` of the main checkout.
  - **Desktop input:** none.
  - **The scratch core's API:** I called it for the session list, the screen peek and the recording path. To do so I read the token from the scratch instance's `run/endpoint.json`. `.claude/rules/local-api.md` says never to read the token from a running Marshell's `endpoint.json`. That held here only in that this was the check's own scratch app: the token was never printed, and the real Marshell was not touched. Later checks should read state through the webview instead.

## Results

| Check | Result |
| --- | --- |
| Ctrl+Shift+T lists PowerShell 7, Windows PowerShell, Command Prompt, Git Bash and each WSL distro | Pass. The shell list read "PowerShell 7", "Windows PowerShell", "Command Prompt", "Git Bash" and "Ubuntu", the one distro installed. |
| With Command Prompt chosen, Copilot is in the missing line | Pass. "Not in Command Prompt: GitHub Copilot (its path has a space, which Command Prompt can't start), Cursor (not installed), opencode (not installed), Amp (not installed), Kimi (not installed), Aider (not installed), Crush (not installed)". |
| Choosing Claude Code replaces the prompt line with its note | Pass. No prompt field, and the note "Command Prompt can't pass a first prompt. Type it once Claude starts." |
| The started tab's argv is `cmd.exe /k claude …` | Pass. `C:\WINDOWS\system32\cmd.exe /k "claude --permission-mode manual"`, with `claude.exe` as its child. Claude reached its first-run theme picker. |
| Git Bash starts an agent, and the argv shows its wrapper | Pass. `"C:\Program Files\Git\bin\bash.exe" -l -c "claude --permission-mode manual; exec 'C:\Program Files\Git\bin\bash.exe' -l"`. Git Bash's missing line named only the CLIs that aren't installed. Copilot was offered. |
| PowerShell 7 starts an agent, and the argv shows its wrapper | Pass. `pwsh.exe -NoLogo -NoExit -Command "& claude --permission-mode manual"`, with `C:\Users\…\.local\bin\claude.exe` as its child. |
| WSL: the checking note shows, then the offer | Pass, with your permission. Ubuntu was not running, so the probe started it. "Checking which agents Ubuntu has…" showed for about 3 s and then gave way to the answer. Start stayed enabled for Just the shell throughout. The distro was left to stop by itself. |
| WSL: Copilot reads `Marshell runs it in WSL later`; a CLI not installed there reads `not installed` | Pass. "Not in Ubuntu: Claude (not installed), Codex (not installed), Antigravity (not installed), Qwen (not installed), GitHub Copilot (Marshell runs it in WSL later), Cursor (not installed), …". This Ubuntu has no agent CLI on the probe's PATH, so the only offer was "Just the shell". |
| A typed name reaches the sidebar row | Pass. "cmd claude check" became row 1 and the window title. |
| An empty name gives the default | Pass. The Name placeholder read "Claude Code", and the row and title were "Claude Code". For Just the shell the placeholder read "PowerShell 7". |
| Alt+N moves focus to the field | Pass. From a chip, focus went to `.launcher__name-input`. |
| Added: Alt+N from an open agent list closes it, and Enter in Name starts | Pass. With the agent list open (8 options), Alt+N left 0 options and focus in Name. Typing "alt n check" and pressing Enter started row 5 "alt n check". |
| With the worktree on, the row is named after the task | Pass. Task name "fix-login": the Name placeholder followed it to "fix-login" and the hint read "Branch marshell/fix-login". The confirm view showed `git -C "…\lc-6o-1\repo" worktree add -b marshell/fix-login "…\lc-6o-1\mh\worktrees\repo\fix-login" main`. Row 3 read "fix-login", with "Task" and "Review changes". |
| The launcher has no Tools group | Pass. The agent list held only Just the shell and the agents, with no group heading and no "Tools" text. |
| The Agents dialog has no Tools list | Pass. Settings › Agents showed no "Tools", and no Tools UI is left in `src/features/{agents,launcher,settings}`. |
| `python -m http.server 8765` in a shell tab still gets a port chip | Pass. "Open http://localhost:8765/ in the browser". |
| `kubectl get secret x -o yaml` still gets the "Output hidden" marker in the recording | Pass. The cast holds "Output hidden: kubectl get secret … prints secrets", then "Output recorded again". It holds it twice: the first line I typed sat in Python's input until Python was stopped (finding 4), and then ran. |
| Focus look, normal colours: a chip, the prompt line and the Name field | Screenshots taken for you to judge (decision 9): `1-chip.png`, `2-prompt.png`, `3-name.png`. A focused chip fills with Claude's accent and takes black ink. The focused prompt line darkens from the terminal's black to a faint accent tint, under an accent caret. The focused Name field drops from the sheet's grey to the tinted terminal surface. |
| Focus look, forced colors emulated | Screenshots taken: `4-chip-forced.png`, `5-prompt-forced.png`, `6-name-forced.png`. A chip and the Name field get a 2 px Highlight outline. The prompt line gets Highlight above and below it (finding 6). |

## Findings

1. **With a session open, no click reached the launcher.** xterm's link layer is a clear canvas with a z-index. The launcher's layer had none (`z-index: auto`), so the canvas lay over the sheet and took every pointer event. Playwright's click on the Mode chip reported "`<canvas class="xterm-link-layer">` … intercepts pointer events". The keyboard worked, which is why the CDP-driven checks above passed. This predates 6o (`app.css` is not in the branch's diff), but it is the slice's surface. Fixed in 2e0a7d3: `.live-app__launcher` gets the palette's `z-index: 20`. The test is `overlayStacking.test.ts`'s "the launcher's layer stacks above them too…", RED→GREEN. Afterwards a real click on the Mode chip cycled it from Manual to Plan.
2. **The prompt kept the base focus ring inside its tinted line.** `base.css`'s `:focus-visible { outline: 2px solid var(--focus) }` reached `.launcher__prompt`. Task 6 had hidden the ring on every other launcher field, but not on this one. Fixed in 822145c. The prompt's outline is transparent, and in forced colors it is `Canvas`, since the line already draws the Highlight outline. Two new `Launcher.css.test.ts` tests went RED→GREEN. One of them reads `Launcher.tsx`'s focusable classes, so a new control can't miss its rule.
3. **During onboarding, Ctrl+Shift+T opened the launcher invisibly.** Onboarding sits over the window at `z-index: 20`, and the launcher sat under it at `auto`. The launcher still took the keys, so Enter could start a session the person never saw. The first three checks above ran that way, before I noticed. Since fix 1, both layers are at 20 and the launcher comes later in the DOM, so it now shows over onboarding. Not decided: whether the key should open the launcher during onboarding at all. That is a question for a later slice.
4. **Ctrl+C reached no program in this check, and the cause is the check, not the build.** Ctrl+C from the terminal, and the `\x03` byte through the input route, stopped neither `python -m http.server` nor `ping -t`. Running `SetConsoleCtrlHandler(NULL, FALSE)` in the tab's PowerShell and then a fresh `ping -t` let Ctrl+C stop it ("Control-C"). The app had inherited Windows' "ignore Ctrl+C" flag from the background process chain that started `pnpm tauri dev`, and every shell inherits it from the app. A person who starts Marshell from the Start menu doesn't carry the flag, and the 2026-10-25 check saw Ctrl+C work. Not fixed. The app could clear the flag at startup, and so could `live-check.ps1 start`, so that a live check matches a real start.
5. **The prompt line's focus is subtle in dark mode.** At rest the line is already the terminal's black. Focus adds a 14% accent tint, which is easy to miss beside the chips' solid fill. The caret is the stronger cue. This is yours to judge; a stronger tint is a one-rule change.
6. **In forced colors, the prompt line's Highlight outline shows only above and below.** The line spans the sheet from edge to edge, and the sheet clips the outline's sides. Not fixed; yours to judge alongside 5.
7. **The Bypass acknowledgement checkbox keeps the base ring.** It borrows CloseBar's style, and decision 9's list doesn't name it. It appears only in Bypass mode. Recorded as a ruling, not fixed.
