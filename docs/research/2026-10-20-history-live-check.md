# History live check (slice 3e1)

- Date: 2026-10-20
- Build: `phase-3e1` at da4976f (the last plan task) plus the fix below, `pnpm tauri dev` on Windows 11 Pro (26300), Claude Code 2.1.304
- Home: a scratch `MARSHELL_HOME` with a space in its path (`%TEMP%\marshell 3e1 home`)
- Target: `MARSHELL_CLAUDE_SETTINGS` set to `%TEMP%\marshell-3e1 project\.claude\settings.local.json`; the status hooks were installed there through `POST /v1/agents/claude/hooks/plan` and `/v1/config/apply`, so the real `~/.claude` settings were never written
- Claude sessions started for this check in the scratch project. Driven through the local API with the scratch home's token; Enter and arrow keys went over `/v1/pty/<tab>`. No zstd CLI was installed, so archives were decoded with Node's `zlib.zstdDecompressSync`

## Results

| Check | Result |
|---|---|
| 1. One prompt, then the archive | Pass after finding 1: `<id>.jsonl.zst` (63,586 B) and `<id>.meta.json` were written 16 ms after the Stop event. Before the fix the decoded copy was a strict prefix of the transcript (476,611 of 479,505 B); after a clean `/exit`, SessionEnd archived all 481,296 B, byte-equal |
| 2. `/clear`, then one prompt | Pass: `/clear` sent SessionEnd (`reason=clear`) for the first session and SessionStart for a new one, whose sidecar has `previous_in_tab` set to the first id |
| 3. Two Claude tabs in one folder, close Marshell, reopen, Resume each | Pass: both rows came back Ended; Resume returned 200 for each and each answered with its own word (MANGO, QUINCE). The process command lines were `claude.exe --resume <that tab's id> --permission-mode manual`, under `pwsh -NoExit -Command "& claude --resume …"` |
| 4. The session id after `claude --resume <id>` | Kept: SessionStart with `source=resume` carried the same `session_id`, no new `.jsonl` appeared in Claude's projects folder, and the existing transcript grew. The archive rewrote the same `<id>.jsonl.zst` and sidecar; `started_ms` stayed and `updated_ms` advanced |
| 5. Garbage in `history/index.sqlite`, then start | Pass: `index.sqlite.broken-<ms>` existed, the new index held all four sessions, and Resume ran `--resume` with each tab's own id. The log line could not be seen (finding 2) |
| 6. A conversation Claude no longer has | Pass: Resume and Restart both returned 409 "Claude Code no longer has this conversation, so it can't be resumed."; the row stayed Ended and no `claude.exe` started |

Check 6 differs from the plan. Claude keeps its transcripts in the real `~/.claude/projects`, which this check does not write, so instead of renaming a transcript, the tab's scratch sidecar was pointed at a missing `<id>.jsonl` in the scratch project and the index was deleted so it rebuilt from the sidecars. The refusal takes the same path: the indexed transcript is not a file. Renaming the real file is covered by `tests/resume.rs` (`resume_refuses_a_conversation_claude_no_longer_has`).

## Findings

1. **The Stop-time archive lacked the turn's last reply.** Claude Code writes the final `assistant` line, `stop_hook_summary` and `turn_duration` about 90 ms after the Stop hook fires, and a few hundred bytes after SessionEnd, so a copy made at once missed them. If Marshell then closed or was killed, nothing copied them later. Each copy now waits 1.5 s for the transcript to settle first (6015da1, `copy_waits_for_the_turns_last_lines`).
2. **The rebuild is logged to nowhere.** The core has no tracing subscriber yet, so `tracing::warn!` lines (the index rebuild, a failed copy) are not printed or kept, with or without `RUST_LOG`. Check 5 was confirmed from the `.broken-` file and the rebuilt rows. Logs with rotation are slice 3e2.

### Minor, not fixed

- `POST /v1/sessions` accepts a `cwd` that is not a folder, starts the agent in the user's home and stores the bad text as the row's folder. This predates 3e1.
- Each sidecar is written through the store, which keeps the previous copy as `<id>.meta.json.bak` next to it.

## Notes

- A new folder makes Claude show its trust prompt, which takes the launch prompt. The trust was accepted for the scratch project only and the prompts were then typed.
- Peek returns the last non-blank rows; for one tab it showed only Claude's input box, so the answers in check 3 were read from the transcripts.
- Hook events are kept in memory and are empty after a restart.
- Closing the window ended its Claude sessions with it, so the tabs came back Ended without a SessionEnd: the case finding 1 is about.
- Claude itself wrote the four scratch transcripts (and a `memory` folder) under `~/.claude/projects/C--Users-Home-Office-AppData-Local-Temp-marshell-3e1-project/` and probably recorded the folder's trust in `~/.claude.json`. Marshell wrote only to the scratch home.
- Cleanup: the dev app, its Tauri and Vite parents and the Claude sessions it started were stopped by PID; both scratch folders were removed after the evidence was taken.
