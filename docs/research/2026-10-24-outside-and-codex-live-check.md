# Outside conversations and Codex live check (slice 5d)

- Date: 2026-10-24, 12:15 to 12:28 local time.
- Build: `phase-5d` (2276c77, then the fixes 7128d8a, 4567e79 and d029050 through Vite's hot reload) under `pnpm tauri dev` on Windows 11 Pro (26300).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `MARSHELL_CLAUDE_SETTINGS` from `live-check.ps1 start -Seed`, with CDP on port 9333. The onboarding was left with Continue, then "Skip: use limited status", so no hooks were installed.
- Data: a seed built from the repo's fixtures (`python -I build.py <repo> <seed> <scratch>\proj`), with every cwd moved to the scratch `proj`:
  - Claude Code: `…5d1` (`entrypoint` cli, from `tools.jsonl`), `…5d2` (claude-vscode, from `basic.jsonl`) and `…5d3` (sdk-cli, from `commands.jsonl`).
  - Codex: `…c1` (tui), `…c2` (exec), `…c4` (a sub-agent thread) and `…c3` (Desktop app) as `.jsonl.zst`, a hand-built zstd frame of raw blocks.
  - During the check: two lines appended to `…5d1` with the current time, and 1,500 copies of `…5d3` (as cli) moved into `cc\projects\bulk-project`.
- How it was driven: the webview over CDP, with evaluations returning small JSON, CDP key events, emulated forced colours (not a real high-contrast theme), and two screenshots. The core API was read with the scratch `mh\run\endpoint.json`. No desktop input was sent.
- Real config: `~/.claude/settings.json` had the same modified time (07:50:25.8019963Z) before and after. The real `~/.codex` was never a root: the importer read the scratch `CODEX_HOME`.

## Results

| Check | Result |
| --- | --- |
| The first run copies what Claude Code and Codex already hold | Pass. `GET /v1/history/import` ends at `{on: true, copying: false, done: 7, total: 7}`. Six conversations are archived; the Codex sub-agent thread is skipped, and the `.zst` rollout is decoded. |
| Where each conversation ran | Pass. The clients read `terminal` (5d1, c1), `vscode` (5d2), `headless` (5d3, c2) and `desktop` (c3). The list shows four rows with the marks "Desktop app", "Outside Marshell" (twice) and "VS Code"; the headless runs are hidden. |
| Filters | Pass. "Show headless runs" adds two rows marked "Headless". Where offers Any, Marshell and Outside Marshell; Marshell shows no rows (nothing here ran in Marshell) and Outside shows them all. The Agent filter offers "Claude Code" and "Codex CLI". |
| A Codex conversation | Pass. The view has Resume and More actions, and no Fork. Recording says "Marshell has no recording of this conversation." Files changed (3) lists "Created src/new.ts", "Edited src/app.ts" and "Edited gone.txt", and the turn shows "Used docs.search". After finding 1 the note reads "Only edits made with Codex's apply_patch are listed. Changes made through shell commands, such as sed or a code generator, aren't." |
| Search | Pass. "five attempts" finds the Codex turn. A search for a headless run's title gives no hits by default. |
| A turn added outside updates its row | Pass. After the next rescan, 5d1's row moves to the top as "1 min ago · 2 turns", and `updated_ms` is the appended line's time. |
| The resume warning | Pass. Resume on 5d1 shows "This conversation changed 1 min ago and may still be open in another terminal. Resume here anyway?" with Cancel focused and the text as its description. Esc closes it and focuses Resume. "Resume here" opens a tab in `proj` running `claude.exe --resume 00000000-0000-4000-8000-0000000005d1`. Its Claude stopped at the login, and no input was sent to it. |
| Delete Marshell's copy | Pass. "Delete Marshell's copy of “Retry limit work”? Claude Code's own file is not touched." removes 5d2, and it stays gone after the next 60 s rescan. |
| The progress line | Pass. With the 1,500 files the line reads "Copying conversations from Claude Code and Codex: 194 of 1500." and counts up every second, then the list reloads with every row. The end is announced as "Finished copying conversations from Claude Code and Codex." |
| History settings | Pass. The dialog "History settings" has the switch "Copy conversations run outside Marshell", On, with the hint "Marshell reads Claude Code's and Codex's conversation folders and keeps its own copy of each conversation. It never changes their files." Turning it off mid-copy stops it (`on: false, copying: false`, 576 of 1500), and turning it on resumes with the 924 left. |
| Keyboard | Pass. The whole check ran on keys: Ctrl+Shift+Y, the arrows and Enter in the list, End in the More actions menu, Tab and Space on the switch, and Esc. Focus returns to the list after a delete. |
| Forced colours | Pass (emulated). The dialog, the switch, the marks and the diffs carry system colours. |

## Findings

1. **A Codex conversation's Files changed spoke of Claude Code.** The note said "Only edits made with Claude Code's Edit and Write tools are listed", and the empty line "Claude Code didn't edit any files in this conversation." Fixed in 7128d8a (with 4567e79 for the type check): the note and the empty line follow the conversation's agent. Failing first: "names Codex and its apply_patch in a Codex conversation" in `FilesChanged.test.tsx`.
2. **Turning the switch off mid-copy announced "Finished copying conversations from Claude Code and Codex."** The poll took any end of copying as a finish. Fixed in d029050: an end with the switch off says "Stopped copying conversations from Claude Code and Codex: 297 of 924 copied.", and a failed read says nothing. Failing first: "words the end of a copy: finished, stopped by the switch, or unknown after a failed read" in `historyState.test.ts`; the announcement was checked live.
3. **A Codex update with several hunks numbers its lines as one hunk.** The parser drops the `@@` lines, so in `src/app.ts` the second hunk's lines go on from the first (5, 6, 7). Not fixed; a minor.
4. **New outside files wait for the 60 s rescan.** The bulk folder was found 71 s after it arrived. That matches the plan (a rescan every 60 s, no file watcher). Not a bug.
5. **The Project filter listed "proj" twice.** It comes from the seed: the Claude Code fixtures write the cwd with forward slashes and the Codex ones with backslashes, and the list groups by the cwd as written. This check didn't confirm whether real Windows transcripts from the two agents agree. Not fixed.
