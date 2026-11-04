# Maintenance live check (slice 6m)

- Date: 2026-11-04, 19:13 to 19:48 local time.
- Build: `phase-6m` at `70da330` under `pnpm tauri dev` on Windows 11 Pro (10.0.26300).
- Home: three scratch homes in turn (`lc-6m-1`, `lc-6m-2`, `lc-6m-3`), each with its own `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `MARSHELL_CLAUDE_SETTINGS` and `USERPROFILE`, and CDP on port 9333. In `lc-6m-1`, onboarding set up all four CLIs into the scratch files. `lc-6m-2` started with no WSL profiles (finding 5) and was stopped unused. In `lc-6m-3`, onboarding was skipped for all four CLIs ("Skip: use limited status"), because only the WSL distro was under test there.
- Data:
  - A throwaway distro `Marshell-6m` was imported with `wsl --import` from Canonical's Ubuntu WSL rootfs into a scratch folder. Only it was ever terminated (`wsl --terminate Marshell-6m`).
  - The user's own Ubuntu was used read-only for item 1. The check created `/tmp/marshell-6m-lc` there.
  - For item 4, the scratch Claude settings file was hand-written twice: once as one line of JSON (the one-line note), and once with CRLF line ends and no secret-looking keys (the Raw save; finding 4 explains the second condition).
- How it was driven:
  - CDP evaluations and CDP keys through the `webview` MCP server. The launcher was opened by keyboard, because a mouse click on it is intercepted (finding 1).
  - Request and response bodies were read from the webview's network log, and there were 9 screenshots.
  - There was no desktop input and no forced-colors emulation.
  - `marshell uninstall` ran from `target\debug\marshell.exe`, with the scratch variables set and the answer piped to stdin.
  - The real `~/.claude/settings.json` kept its modified time, 2026-11-04T14:15:35.0283807Z.

## Results

| Check | Result |
| --- | --- |
| 1. #85 in the user's Ubuntu: `sleep 15` shows a running command, an empty line gives a bare `D`, `mkdir -p /tmp/marshell-6m-lc/x && cd "$_"` works | Pass, on bash 5.3.9, which takes the PS0 path. The recording has one `C` per command and a bare `D` for an empty line, and `cd "$_"` lands in `/tmp/marshell-6m-lc/x`. Bash 4.4 to 5.0, which take the DEBUG-trap fallback, were not run. |
| 1. A hook mark posted through the bridge with the tab's id is accepted | Pass. `marshell-hook.exe` ran from inside the WSL tab with a fake SessionStart payload and a scratch `HOME` in `/tmp`. The row showed "Claude, Status limited" while the command ran, then went back to "WSL, Idle". To detect agents, the core also ran a read-only `claude --version` in that distro. |
| 2. #82: set hooks up in `Marshell-6m` for each installed CLI | Pass, after one retry (finding 7). Each preview's only targets were in the distro: `\\wsl.localhost\Marshell-6m\root\.claude\settings.json` (+140), `\.codex\hooks.json` (+92), `\.gemini\config\hooks.json` with `\.gemini\antigravity-cli\settings.json` (+57), and `\.qwen\settings.json` (+106). The bridge is the quoted Linux path of the scratch `mh\bin\marshell-hook.exe`. Apply wrote exactly those line counts. |
| 2. `marshell uninstall` with the scratch `MARSHELL_HOME` and `USERPROFILE`, answer n | Pass. It printed "Looking in WSL distros: Ubuntu, Marshell-6m.", "Ubuntu: nothing of Marshell's." and "Marshell-6m: Marshell's hooks found.", then the five distro diffs, and "The folder …\lc-6m-3\mh\bin and the programs in it will be removed." Answering n gave "Nothing changed." and exit 1. |
| 2. Apply only if the preview names no file outside `\\wsl.localhost\Marshell-6m\`, then check the files | Pass. The preview named only the five distro files and the scratch `mh\bin`. The second run, answered y (never `--yes`), printed "Done. Snapshot sb025408254f4 holds the files as they were before this change." and exited 0. Afterwards the Claude, Codex, agy `settings.json` and Qwen files were `{}`, agy's `hooks.json` was gone, `mh\bin` was gone, and `managed.json` had 0 entries. |
| 2. Stop the distro and run uninstall again, for the "didn't answer" line | Can't be made to happen with what the check may do. After `wsl --terminate Marshell-6m` the distro showed Stopped, and uninstall's home lookup started it again (Running), so it answered: "Marshell-6m: nothing of Marshell's.", exit 0 in 2 s. Only `--shutdown` or stopping the WSL service could stop it answering, and both would also hit the user's Ubuntu and docker-desktop. The line is covered by `a_distro_that_does_not_answer_is_named` and `a_managed_distro_that_is_no_longer_installed_does_not_keep_bin` in `crates/cli/src/uninstall.rs`. |
| 3. Approve cards: the dead first click on an unfocused, armed card, then one click answers | Pass. The dead first click did not reproduce. A single CDP click on "Allow once" on the unfocused, armed card answered it, and the requesting process wrote its result file. |
| 3. A shell tab with no agent gets no card; a pwsh tab that runs an agent gets one | Pass. A bridge-posted request from a bare pwsh tab got 204 and no card. The same request from a pwsh tab running Qwen Code got the card. |
| 4. Hooks off for Claude deletes `statusline-chain.json`, and Undo restores it | Pass, with a limit. "Turn Claude Code hooks off" deleted the chain file, and its snapshot holds both `settings.local.json` and `statusline-chain.json`. The UI has no Undo for Hooks off (finding 6). The restore of both files is covered by `hooks_off_deletes_the_chain_and_undo_puts_back_both_files` in `crates/core/tests/agents_api.rs`. |
| 4. Setup on a one-line settings file shows the one-line note | Pass. Under the open diff: "This file was on one line. Marshell lays it out over several lines." |
| 4. The Company tab's review names the scratch snapshot folder | Pass. "A backup is saved first, in …\lc-6m-1\mh\snapshots." |
| 4. A CRLF raw save shows a one-line diff | Pass. The diff read "+1 −1". The file kept CRLF (9 CR, 9 LF), and Undo ("Restore: Change Claude Code settings") put it back. |
| 5. Reopen and Restart log no 404 in the webview console | Pass. Reopen (Ctrl+Shift+Alt+T) and Restart (Mode ▸ Plan, then the "Restart 3 Qwen Code in Plan?" bar) logged no 404. The only console errors were the favicon's 404 and an expected 409 from a stale plan. |
| 5. No unadopted panel recording (changed to close/kill by the Task 13 ruling) | Partly checked. A dead panel shell's recording was deleted when "Open in a pane" found it dead (the `take_and_adopt` path). Folding and unfolding the panel doesn't ask for a new shell (finding 2). The close path (`take_panel_where`) was not exercised. An app exit is left to retention, per the Task 13 ruling. After `lc-6m-1` was stopped, `recordings/` held no orphan panel recording. |

## Findings

None of these come from this slice's changes, and none is fixed here.

1. **The launcher can't be clicked with a mouse where it overlaps a terminal.** The xterm link layer's canvas (`z-index: 2`) sits above the launcher (`z-index: auto`) and takes the click. Keyboard use works. Not fixed.
2. **A panel shell that exits stays dead.** Folding and unfolding the panel doesn't start a new shell, because the panel asks for one only when it has none or the endpoint changed (`LiveApp.tsx`). "Open in a pane" then fails with only a `console.warn` and a 404. The core does clean the dead shell's recording. Not fixed.
3. **The Company page goes stale after Hooks off changes the same file.** A Raw save there then gets the 409 guard ("changed"), which holds. The page doesn't refresh, because `companyRefresh` is bumped only after an update task. Not fixed.
4. **`is_secret_key` matches "auth" as a substring**, so `includeCoAuthoredBy` is masked, and a file holding it opens read-only in Raw. Not fixed.
5. **The 2 s `wsl -l -q` timeout at startup can leave the app with no WSL profiles and no distros until the next start.** Seen in `lc-6m-2`. Just before `lc-6m-3`, the same command answered in 59 ms. Not fixed.
6. **Hooks off has no Undo in the UI.** The snapshot exists and the restore route works (Task 2 ruling). Not fixed.
7. **The first WSL set-up of a fresh app process got a 409 "changed" on Allow, though nothing had changed.**
   - When: Claude Code's "Set up in Marshell-6m" in `lc-6m-3`. The plan request booted the freshly imported distro, and its first files appeared at 17:38:50–52Z, around the preview's watch at 17:38:51.93Z. Allow came about 46 s later.
   - What the core did: it refused with `{"reason":"changed"}`. Nothing was written and no snapshot folder was made. The UI said "The file changed after this preview was made, so nothing was written. Reload to see what it says now." That isn't true: the file didn't exist before or after.
   - Why it is apply's own re-plan: the UI sent the hash it showed, so the refusal came from that re-plan. Either its target list differed, or it came back Refused. The two branches give the same reason and log nothing, so the run can't tell which. The preview after Reload had the same path, no before hash, the same after hash (`44951a41…`) and the same diff, and its Allow wrote the file.
   - Not reproduced: Codex's plan cold-started the distro after a terminate, Antigravity applied with the distro Stopped, and Qwen applied after a long wait. All three went through first time.
   - Not fixed. Passed to the final review against the slice's WSL plan changes.
