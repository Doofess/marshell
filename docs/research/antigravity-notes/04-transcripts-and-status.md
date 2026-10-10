# Antigravity CLI: Transcripts & Status Line

## Transcript Parsing (`Adapter::parse_transcript`)
Antigravity logs every turn to a JSONL file. 
You can find the exact path to this file during a live session via the `transcriptPath` field in any hook payload (e.g., `PreToolUse`, `Stop`).
If resolving it statically, it is usually located at:
`~/.gemini/antigravity-cli/transcript.jsonl` (or `transcript_full.jsonl`).

### Schema Notes
- **Format:** JSON Lines (JSONL).
- **Entries:** Each line represents a single step or action.
- **Fields:** `step_index`, `source` (`USER_EXPLICIT`, `MODEL`, `SYSTEM`), `type` (`USER_INPUT`, `PLANNER_RESPONSE`), `status`, `created_at`, `content`, `tool_calls`.
- **Truncation:** If `truncated_fields` is present in `transcript.jsonl`, the full un-truncated content resides in the corresponding line in `transcript_full.jsonl`.
- **Media/Artifacts:** `media` arrays contain `mime_type` and a `uri` referencing disk files (usually in the `artifacts` directory). 

### Token Usage Extraction
Unlike Claude Code which reports context directly in a status line, for Antigravity you must derive session usage by summing `message.usage` values from the transcript JSONL (if available) or estimating based on characters.

## Status Line / Recaps
Antigravity supports `agy --print` (or `-p`) which can run headless.
`Adapter::headless_summarize` can use this to generate a smart recap:
```bash
agy -p "Summarize what you've done in one short sentence" --dangerously-skip-permissions
```
*(Ensure you pass `--dangerously-skip-permissions` or equivalent flags to avoid blocking the headless call on a permission request!)*
