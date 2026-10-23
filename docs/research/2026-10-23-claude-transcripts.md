# Claude Code transcripts: the shape slice 5a parses

- Date: 2026-10-23.
- Source: a read-only survey of the transcripts on the development machine (`~/.claude/projects/*/*.jsonl`): 275 files, 257,279 lines, versions 2.1.278 to 2.1.309.
  - Every line parsed as JSON.
  - Median file size 172 KB, largest 94 MB.
- What was read: only key names, line types and counts. No values were read, apart from the name of the tag a user string starts with.
- Claude Code calls the format internal and says it changes between versions. Slice 5a's parser is best effort and version-gated (docs/PLAN.md §9, phase 5, "Parsing").

## Layout

- **Main transcript:** `<project dir>/<session id>.jsonl`, one JSON object per line.
- **Subagents:** `<project dir>/<session id>/subagents/agent-<id>.jsonl` (733 files), separate from the main transcript. Large tool results go in `<session id>/tool-results/`.
  - Marshell's archive copies only the main file (slice 3e1).
  - 5a shows a subagent through the `Agent` tool call and its result in the main file.

## Line types (count)

| Type | Lines | Use in 5a |
| --- | --- | --- |
| `assistant` | 102,123 | One content block per line (`apiBlockIndex`): `text`, `thinking` or `tool_use`. `message.id` repeats across the lines of one API message; `message.model` names the model. |
| `user` | 62,939 | A prompt (`message.content` a string, or `text` and `image` blocks), or `tool_result` blocks for the assistant's tool calls, with a structured `toolUseResult` beside them. |
| `attachment` | 102,820 | Ignored. |
| `api-request`, `api-request-blob`, `api-request-shape` | 75,910 | Ignored. |
| `system` | 3,070 | `subtype: compact_boundary` marks a compaction. `turn_duration`, `stop_hook_summary`, `away_summary`, `informational`, `local_command` and the rest are ignored. |
| `ai-title` | 10,234 | `aiTitle`: Claude's own title. The last one wins. |
| `custom-title` | 7 | `customTitle`: a title the person set. Wins over `ai-title`. |
| `last-prompt`, `mode`, `permission-mode`, `atis-latch`, `pr-link`, `worktree-state`, `agent-name`, `agent-color`, `queue-operation`, `relocated`, `file-history-*`, `bridge-session`, `frame-link`, `cost-state` and others | the rest | Ignored. |

Common fields on `user`, `assistant` and `system` lines: `uuid`, `parentUuid`, `timestamp` (ISO 8601), `sessionId`, `cwd`, `gitBranch`, `version`, `isSidechain`, `userType`, `entrypoint`. Some lines also carry `isMeta`, `isCompactSummary`, `isVisibleInTranscriptOnly`, `isApiErrorMessage` or `agentId`.

## User strings that are not prompts

A user string that starts with one of these tags is something Claude Code wrote, not something the person typed. Counts are of strings that start with the tag.

| Tag | Count | 5a |
| --- | --- | --- |
| `task-notification` | 935 | Skipped. |
| `local-command-caveat` | 222 | Skipped. |
| `command-name` | 188 | A slash command. Shown as the prompt `/<name> <args>`, from `<command-name>` and `<command-args>`. |
| `local-command-stdout` | 156 | Skipped. |
| `bash-input` | 55 | A `!` shell command. Shown as the prompt `! <command>`. |
| `bash-stdout` | 54 | Skipped. |
| `command-message` | 7 | Skipped. |
| `system-reminder` | 2 | Skipped. |

`isCompactSummary` user lines (274) are the summary a compaction feeds back. `isMeta` lines are Claude's own. Neither is a prompt.

## Tool calls

`tool_use` blocks carry `id`, `name`, `input` and `caller`. Inputs of the tools 5a summarises:

| Tool | Input keys |
| --- | --- |
| `Bash` | `command`, optional `description`, `timeout`, `run_in_background` |
| `Edit` | `file_path`, `old_string`, `new_string`, `replace_all` |
| `Write` | `file_path`, `content` |
| `Read` | `file_path`, optional `offset`, `limit` |
| `Grep` | `pattern`, `path`, `output_mode` and flags |
| `Agent` | `description`, `prompt`, `subagent_type`, optional `model`, `effort`, `run_in_background` |
| `WebFetch` | `url`, `prompt` |
| `WebSearch` | `query`, `mode` |

A `tool_result` block has `tool_use_id`, `content` and an optional `is_error`. Its line's `toolUseResult` depends on the tool:
- **Shell:** `stdout`, `stderr`, `interrupted`.
- **Edits:** `filePath` and `structuredPatch`, plus `oldString` and `newString` for an edit, or `type` and `content` for a write.
- **Subagents:** `agentId`, `status` and `prompt`.

It can also be a plain string or an array.
