# Exit gaps live check (slice 3g)

- Date: 2026-10-21
- Build: `phase-3g` up to 5ac09dd, `pnpm tauri dev` on Windows 11 Pro (26300), Claude Code 2.1.305
- Home: a scratch `MARSHELL_HOME` (`%TEMP%\m3g\home`)
- Target: `MARSHELL_CLAUDE_SETTINGS` set to `%TEMP%\m3g\proj\.claude\settings.local.json`, which started as `{}`. The project was new, so Claude Code asked to trust it; the check answered Yes in the session.
- Driven over CDP (`--remote-debugging-port=9333`) and the local API with the scratch home's token. One Claude session ("3g check") was started for the check, in Manual mode, and resumed twice.

## Results

| Check | Result |
| --- | --- |
| Consent | Pass: "Add 10 hook entries and a status line in …settings.local.json." Allow wrote 10 groups (9 status events and `PermissionRequest`) and the status line. |
| Partial, Set up again | Pass. The install with `SubagentStart` cut out (the previous build's file, same bytes otherwise) read `hooks: "partial"`, and the Agents dialog said "Some hooks are missing … Set up again to repair them." The diff under View exact changes was `+12 −0`, the `SubagentStart` group alone. After Allow the file was byte-identical to a fresh install and the dialog read "Hooks on". |
| Subagent count, core | Pass after the fix below. The row's `live.subagents` went 0 → 1 → 2 → 1 → 0. |
| Subagent count, UI | Pass after the fix below. The row's second line read "Opus 5.5 · Using \`Glob\` · 2 subagents", the session header "2 subagents", and the row's accessible name "… · 2 subagents". One reads "1 subagent". |
| Live mode | Pass. Shift+Tab in the session turned Claude Code's footer to "accept edits on". The chip stayed Manual until the next hook event, then read Auto-edit from the prompt's `UserPromptSubmit` (`permission_mode: "acceptEdits"`), and held through the subagents' events, which also reported `acceptEdits`. |
| Resume in the mode last seen | Pass. The bar read "Resume runs claude --continue --permission-mode acceptEdits", and the process started was `claude.exe --resume <session id> --permission-mode acceptEdits`. |
| Fixtures | `crates/core/tests/fixtures/claude/2.1.305/`: `SubagentStart.json`, `SubagentStop.json` and `Stop-background.json`, scrubbed with `pnpm scrub:fixtures`. `fixtures_scrubbed` passes. |
| Real config | `~/.claude/settings.json` (2026-10-19T20:31:45) and `~/.marshell` (2026-10-17T22:16:46) kept their dates. |

## Findings

1. **Claude Code runs subagents in the background (fixed in 5b43b5e).** In 2.1.305 the Agent tool returned at once, and the main turn's `Stop` came while a subagent still ran ("I've started both Explore agents … I'll report back once both have finished."). Claude Code then resumed the turn on its own with a `UserPromptSubmit` when each one finished. The plan's rule, "the main Stop clears the set", dropped the count to 0 while one agent was still working, and its `SubagentStop` 200 ms later found nothing to take out. `Stop` and `SubagentStop` carry `background_tasks`, a list of `{ id, type: "subagent", status: "running", description, agent_type }`. At `Stop` it lists exactly the subagents still running; at `SubagentStop` it still lists the one stopping. The main `Stop` now keeps the running subagents it lists, and clears the set only when the list is missing (an older Claude Code). Test: `a_background_subagent_outlives_the_main_stop`, folding the three recorded bodies.
2. **A subagent's events report a mode (fixed in 5b43b5e).** Its `PreToolUse`, `PostToolUse` and `SubagentStop` carry `permission_mode`. Here it matched the session's, but a custom agent can set its own `permissionMode`, which would have moved the row's chip. `mode_of` now skips bodies with an `agent_id`. Test: `mode_of_skips_a_subagents_own_events`.
3. **The comfortable row never showed the count (fixed in 5ac09dd).** Only the expanded row's meta line and the session header had it, while docs/PLAN.md "Sidebar rows" draws the second line as "Editing src/auth.ts · 2 subagents". The count now follows the phrase in a comfortable row and in its accessible name, and stays on the meta line alone in an expanded row. "1 subagents" is now "1 subagent" in the row, the header and the label.
4. **The repair card counts the whole install.** Set up again on a Partial install said "Add 10 hook entries" while its diff added one group. The card's headline counts from the capability table, not from the plan. Not fixed here.
5. **Resume on an agent that ended inside a live shell does nothing.** After `/exit`, the profile's PowerShell kept running, so the row stayed Running with live state Ended. The Resume bar showed, but `resume()` returns at once for a running row. It worked once the shell exited too. This predates 3g.
6. **The Resume bar names `claude --continue`, the core runs `claude --resume <id>`.** Both carry the right mode flag. This predates 3g.
7. **While background subagents run, the row reads Done.** The glyph follows the main turn, and the count is the only sign of work still going on.

## Notes on the check

- Each `tauri dev` rebuild (here, on the `status.rs` edit) restarts the app without ending its sessions. The resumed Claude Code, its PowerShell and its MCP children kept running with no app. The check listed that tree (all started at 01:32 by its Resume) and stopped it by PID.
- After the same restart, the page hit a stale Vite module ("lineSubagents is not defined") left from an edit made in several writes. Touching the file and reloading cleared it. The code itself, the tests and `pnpm build` were fine.
- The session's `/v1/sessions/{tab}/input` refuses line breaks by design, so Enter, Shift+Tab and the trust prompt's answer went through the app's terminal over CDP.
