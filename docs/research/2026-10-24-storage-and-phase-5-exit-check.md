# Storage live check and phase 5 exit check (slice 5e)

- Date: 2026-10-24, 16:05 to 16:34 local time.
- Build: `phase-5e` (4e2f023) under `pnpm tauri dev` on Windows 11 Pro (26300).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `MARSHELL_CLAUDE_SETTINGS` from `live-check.ps1 start`, with CDP on port 9333. The person logged in the scratch Claude Code for the heavy session, so that session used their quota.
- Data:
  - A heavy Claude Code session in a scratch `proj` holding a copy of the repo's `docs/` (90 files). It was asked to write a summary of every doc and worked from 16:16:04 to 16:23.
  - A busy shell running `Get-ChildItem -Recurse` in a loop for 10 minutes, from 16:08:26.
  - Seeded through the API and by hand in the scratch history: conversation `0a5eed00-0000-4000-8000-000000000001` (tab `tshellseed`) linked to a copy of the shell's compressed recording; a large plain copy `tplainseed`; recordings `tseed01` to `tseed03` (60, 45 and 35 days old) and `tseed04` (2 days); images `tseedimg1/a.png` (50 days), `tseedimg1/b.png` (40 days) and `tseedimg2/c.png` (5 days). The Claude conversation was pinned through `PATCH`, and its recording and `tshellseed`'s were dated 60 days back.
  - Backup targets beside the home: `backup1` (empty), `other` (another install's backup) and `foreign` (a "Marshell history" folder Marshell didn't make).
- How it was driven: the core API with curl and the scratch `mh\run\endpoint.json`; the webview over CDP, with evaluations returning small JSON, CDP key events, emulated forced colours (not a real high-contrast theme) and one screenshot. The heavy session's task was typed through the input route and Enter was pressed over CDP. No desktop input was sent.
- Search: `cargo test -p marshell-core --release --lib search_1000_sessions_under_100ms -- --ignored --nocapture` printed "search median over 1000 sessions: 11.1037ms".

## Results

| Check | Result |
| --- | --- |
| A 1000-session index searches in under 100 ms | Pass. Median 11.1 ms in a release build. |
| Recordings stay within a stated MB-per-hour budget | Pass. The heavy Claude Code session wrote 499,573 bytes in its 426 s active window, 4.03 MB per hour plain. Its whole recording, 552,774 bytes, compressed to 64,614 (8.6×), so about 0.47 MB per hour compressed. The budget in `docs/PLAN.md` is under 1 MB per hour compressed for an agent session. |
| A shell that floods output | Pass. The busy shell hit the recorder's existing 256 MiB cap after 570.6 s, with the marker "Recording stopped: it reached the size limit." 268,435,492 bytes compressed to 35,631,103 (7.5×). Uncapped, that rate is about 1.6 GiB per hour plain and about 225 MB per hour compressed. |
| Finished recordings are compressed | Pass. The first closed Claude recording (closed about 16:07:55) was compressed at 16:09:03, 4.9 KB to 1,124 bytes. The shell's at 16:19:08, about 40 s after it ended; the heavy session's at 16:27:07, 44 s after its tab closed at 16:26:23. `tplainseed` was compressed by a tick. With a 60 s tick and a 10 s settle the worst case is about 70 s. Open recordings stayed plain. |
| Replay of a compressed recording | Pass. Page 0 of `tshellseed` has 11,995 events. `Offset` pages continue exactly where the last one ended: page 1 took 442 ms, and a deep offset (126,015,766) 741 ms. An offset that isn't a frame boundary gets "Not a frame boundary." A seek with `from_ms` 300 s into the 256 MiB recording took 20.9 s (finding 4). |
| Retention limits | Pass. In History settings (palette Ctrl+Shift+K, "History settings"), choosing 30 days with ArrowDown showed "This removes 6 recordings and images now (34.3 MB), the oldest from 25 August 2026." The preview's 35,931,563 bytes matched the log line "retention removed 6 files (35931563 bytes)" at 16:30:33. The pinned conversation's recording and the 2- and 5-day files stayed, and the emptied tab folder went. Reopened, the section reads "Last cleanup just now: removed 6 files (34.3 MB)." |
| Cancel and focus | Pass. Cancel set the select back to Forever and kept focus on it. "Remove and save" kept focus on the select too. |
| Backup mirror | Pass. `history_backup_dir` set to `backup1` at 16:31:12 was mirrored by 16:31:14. The mirror matched history except the app's own open shell recording `t6f513abc2b43`, which waits until it is finished. A removed recording left the mirror 51 s later. |
| Backup folders refused | Pass, with nothing saved and the other folder's files untouched each time. `other`: "That folder holds another Marshell's backup. Choose another folder." `foreign`: "That folder already has a "Marshell history" folder that Marshell didn't make. Choose another folder." A missing folder: "That folder doesn't exist." The home and a folder inside it: "Choose a folder outside Marshell's own folder." |
| A backup folder that goes away | Pass after finding 3. Renamed away at 16:32:45, the error was set at 16:33:13 and logged once. Renamed back at 16:33:33, it recovered at 16:34:13 and the section read "Last copy just now." |
| Keyboard | Pass. Tab order in the dialog is the import switch, "Keep recordings and images up to", "Keep recordings and images for", Choose folder…, Stop backup, Close, each with a focus ring. |
| Forced colours | Pass (emulated). The Storage rows, the selects, the question and its buttons carry system colours. Screenshot: `.playwright-mcp/5e-storage-forced.png` (not committed). |
| Real config | `~/.claude/settings.json` gained `advisorModel` at 16:19:45, during the check. Nothing in the check writes that file; the global setup has other Claude Code sessions write that key. It was removed again, and the file otherwise matches `C:/dev/claude-setup/settings.json`. The real `~/.marshell` and `~/.codex` were never roots. |

## Findings

1. **Compressing a recording reset its modified time, so an age limit counted from the compression date.** Retention dates a recording by its modified time, and the `.zst` was a new file. Fixed in 2a00b53: `encode` sets the plain file's modified time on the `.zst`. Test: `history::storage::compress::tests::compress_keeps_the_recordings_modified_time`, failing first.
2. **After "Remove and save" the sizes and the last-cleanup line were stale.** The section read the report right after the save, before the storage thread's cleanup had run. Fixed in 183af88: with a limit set, `reportAfterCleanup` reads the report until the last cleanup changes (at most 20 reads, 250 ms apart). Tests in `src/features/history/storageActions.test.ts`, failing first. Not re-checked live.
3. **A backup folder that went missing read "That folder doesn't exist." instead of Can't reach.** The section treated that error as a refusal, but for a folder already chosen it means an unplugged drive or a share that's away, which retrying fixes. Fixed in 54fead0: it is no longer in `REFUSALS`, so the line reads "Can't reach <folder>. Marshell will try again in a minute." Test: "shows Can't reach for a backup folder that has gone missing, as an unplugged drive does", failing first.
4. **A time seek into a 256 MiB compressed recording took 20.9 s.** Seeking by time decodes from the start of the stream, while `Offset` pages are fast. Plain files were not timed. Not fixed; noted for the replay work.
5. **The first Claude Code session started in the person's home folder.** The check created its tab with an empty cwd. It was closed within seconds, got no input, and used the scratch `CLAUDE_CONFIG_DIR`. A check mistake, not a product finding.
6. **Not from this slice:** the webview console shows a favicon 404, and a pty WebSocket 404 for a tab created through the API just after creation, which then reconnects.
