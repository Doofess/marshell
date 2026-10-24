# Codex CLI rollouts: the shape slice 5d parses

- Date: 2026-10-24.
- Source: a read-only survey of the Codex files on the development machine (`~/.codex/sessions`, plus the schema of `state_5.sqlite` and `thread_history_1.sqlite` read from copies) and of the Claude Code transcripts (`~/.claude/projects/*/*.jsonl`).
  - Codex: `codex-cli 0.163.0`; 61 rollout files (54 `.jsonl`, 7 `.jsonl.zst`), 14,941 lines, every line parsed as JSON; rollout `cli_version` values `0.162.0-alpha.17.2` to `0.163.0`; sessions from 2026-10-10 to 2026-10-23.
  - Claude Code: 294 main transcripts, 278,181 lines, versions 2.1.278 to 2.1.309, 2026-09-20 to 2026-10-24.
- What was read: only key names, value kinds, enum values and counts, through scripts that printed aggregates. No prompt, reply, code or path was copied into this note. Example values below are placeholders.
- `docs/research/2026-10-23-claude-transcripts.md` already covers the Claude line types, tags and tool calls; this note adds only the entrypoint findings for Claude Code.
- The survey is of one machine, mostly Codex Desktop and `codex exec` runs. Only 2 of the 61 files are from the interactive terminal TUI, none from the VS Code extension. Treat every "never seen" below as unverified.

## Layout

| Item | Finding |
| --- | --- |
| Codex home | `C:\Users\<user>\.codex`. `CODEX_HOME` is not set. |
| Rollout path | `sessions/YYYY/MM/DD/rollout-YYYY-MM-DDThh-mm-ss-<uuid>.jsonl`, or the same name with `.zst` appended. |
| Date parts | The folder date and the file-name timestamp are local time (UTC+3 here). `session_meta.payload.timestamp` is UTC. All 61 folder dates equal the name dates. |
| Session id | `<uuid>` in the name equals `session_meta.payload.id` in all 61 files. The id is a UUID whose version nibble is 7 (time-ordered). It is the thread id and the argument of `codex resume`. |
| Size | 61 files, 61.6 MB on disk, 79.9 MB decompressed. Median 0.84 MB decompressed, largest 9.7 MB. Longest line 0.71 MB (a `compacted` line); the next longest are 0.29 MB. |
| Compression | The 7 `.zst` files are plain zstd frames (decompressed with no dictionary), 5.7 MB on disk for 23.9 MB of JSONL. They are the oldest: first lines 2026-10-10 to 2026-10-15, while every `.jsonl` starts 2026-10-16 or later. The feature `local_thread_store_compression` is listed "under development, false" by `codex features list`, so what compresses them is unknown. |
| `state_5.sqlite` | The thread index. Table `threads` (61 rows) has `id`, `rollout_path`, `source`, `originator`, `thread_source`, `cwd`, `title`, `name`, `first_user_message`, `preview`, `cli_version`, `archived`, git columns, timestamps in seconds and ms. `thread_spawn_edges` links parent and child threads. `rollout_path` still ends in `.jsonl` for the 7 files that are now `.zst` on disk. |
| `thread_history_1.sqlite` | A projection of the rollouts: `thread_turns` (139 rows, status `completed` 118 or `failed` 21), `thread_items` (4,396 rows, one per `item_completed`, `item_json` is the same item), and `thread_history_projection_state` with `next_rollout_byte_offset`. For all 54 uncompressed files that offset equals the file size, so Codex tails the rollout by byte offset and the rollout stays the source of truth. |
| `session_index.jsonl` | 10 lines of `{id, thread_name, updated_at}`: the names the user gave to threads (matches `threads.name`). |
| `history.jsonl` | 6 lines of `{session_id, ts, text}`: prompt history (inferred from the key names). |
| Other | `version.json` (update check), `models_cache.json`, `config.toml`, `auth.json`, `logs_2.sqlite`, `queue_1.sqlite`, `goals_1.sqlite`, `memories_1.sqlite`. Marshell has no reason to read them. |

`codex resume [SESSION_ID] [PROMPT]` takes a UUID or a thread name. Its picker filters by cwd (`--all` lifts that) and hides non-interactive sessions (`--include-non-interactive` lifts that). `codex exec resume` and `codex fork` / `codex exec fork` also exist. `codex migrate-rollouts` is described as "inspect or migrate legacy local sessions to paginated thread history".

## Line types

Every line is `{"timestamp": <ISO 8601 UTC, ms, Z>, "ordinal": <int>, "type": <string>, "payload": <object>}`. `ordinal` starts at 0 in every file and is strictly increasing. `response_item` lines may also carry a `metadata` object.

| Top-level `type` | Lines | Use in 5d |
| --- | --- | --- |
| `event_msg` | 6,676 | Yes: `item_completed`, `task_started`, `task_complete`. |
| `response_item` | 5,948 | Optional: the model-level transcript. Needed only as a fallback or to tell injected context from prompts. |
| `token_usage_record` | 1,816 | Ignore. |
| `inter_agent_communication_metadata` | 181 | Ignore (`trigger_turn` boolean, paired with an `agent_message` response item). |
| `turn_context` | 146 | Optional: model, effort, approval and sandbox for the turn. |
| `world_state` | 106 | Ignore. |
| `session_meta` | 62 | Yes: first line of every file. |
| `compacted` | 6 | A compaction marker. Has no `payload.type`. Carries a very large `replacement_history`. |

`event_msg` payload `type`:

| Payload type | Lines | Keys |
| --- | --- | --- |
| `item_completed` | 4,396 | `type`, `thread_id`, `turn_id`, `item`, `started_at_ms`, `completed_at_ms` |
| `token_count` | 1,887 | `type`, `info` (object or null), `rate_limits` |
| `task_started` | 140 | `type`, `turn_id`, `root_turn_id`, `started_at` (Unix seconds), `model_context_window`, `collaboration_mode_kind`, `turn_attribution` |
| `task_complete` | 139 | `type`, `turn_id`, `root_turn_id`, `started_at`, `completed_at` (seconds), `duration_ms`, `time_to_first_token_ms` (132 of 139), `last_agent_message` (string or null), `error` (21) |
| `thread_settings_applied` | 114 | `type`, `thread_id`, `thread_settings` (model, effort, approval policy, permission profile, cwd, ...) |

`item_completed.item` is a tagged object (`item.type`, PascalCase). This is the UI-level projection and the one to parse for the conversation view:

| `item.type` | Lines | Keys besides `type` and `id` |
| --- | --- | --- |
| `Reasoning` | 1,467 | `summary_text` (array), `raw_content` (array). Almost always empty: the text is in the encrypted `response_item` reasoning. Ignore. |
| `CommandExecution` | 1,267 | `process_id`, `command` (array of 3: shell exe, `-Command`, script), `cwd`, `parsed_cmd` (array of `{type, cmd}`, `type` is `unknown` or `read`), `source` (`unified_exec_startup`), `status` (`completed` 1,066, `failed` 201), `aggregated_output`, `exit_code`, `duration` (`{secs, nanos}`) |
| `AgentMessage` | 550 | `content` (array of `{type: "Text", text}`), `phase` (`commentary` 390, `final_answer` 160), optional `delivery` (`async`) and `questions` (42) |
| `McpToolCall` | 413 | `server`, `tool`, `arguments`, `status` (`completed` 409, `failed` 4), `result` (`content`, `isError`, optional `structuredContent`, `_meta`), `duration`, optional connector fields |
| `FileChange` | 299 | `changes` (object), `status` (`completed` for all 299), `stdout`, `stderr` |
| `SubAgentActivity` | 183 | `kind` (`started` 14, `completed` 52, `interacted` 117), `agent_thread_id`, `agent_path`, `model`, `reasoning_effort` |
| `UserMessage` | 160 | `content` (array), optional `client_id` |
| `Extension` | 46 | `kind` (`web.search` 41, `clock.sleep` 5), `query`, `action`, `results`, `durationMs` |
| `ContextCompaction` | 6 | none |
| `CollabAgentToolCall` | 3 | `tool` (`wait`), `status`, `sender_thread_id`, `receiver_thread_ids`, `agents_states` |
| `ImageView` | 2 | `path` |

`response_item` payload `type`:

| Payload type | Lines | Keys |
| --- | --- | --- |
| `reasoning` | 1,467 | `id`, `summary`, `encrypted_content` |
| `custom_tool_call` | 1,267 | `id`, `status`, `call_id`, `name` (`exec` 1,216, `apply_patch` 51), `input` (string) |
| `custom_tool_call_output` | 1,267 | `id`, `call_id`, `output` (array of `input_text` / `input_image` blocks, or a string for 58) |
| `message` | 1,042 | `id`, `role` (`developer` 263, `user` 271, `assistant` 508), `content`, optional `phase`, `status` |
| `function_call` | 360 | `id`, `name`, optional `namespace` (177), `arguments` (JSON string), `call_id`, optional `status` |
| `function_call_output` | 360 | `id`, `call_id`, `output` (string for 316, array for 44) |
| `agent_message` | 181 | `author`, `recipient` (agent paths such as `/root/<name>`), `content` |
| `additional_tools` | 4 | `role`, `tools` |

Every `response_item` payload also has `internal_chat_message_metadata_passthrough` with `turn_id` (messages and tool items), `create_time` and, on messages, `content_item_kinds`. Content blocks: `input_text` and `input_image` (user, developer, tool output), `output_text` (assistant).

`function_call` names: `exec_command` 112 (arguments `cmd`, `max_output_tokens`, optional `yield_time_ms`), `write_stdin` 20, `request_user_input_async` 42, `wait` 8, namespaced `collaboration::{spawn_agent, followup_task, send_message, wait_agent, list_agents}`, `clock::sleep`, one MCP-namespaced `js`, `untrusted_input`. No `local_shell_call`, no `event_msg` `user_message`, `agent_message` or `exec_command_end` appeared in any version here: the pre-0.162 shapes are not in this data.

Line types by `cli_version` (all versions share one generation; `ordinal`, `world_state` and `token_usage_record` are in every file):

| `cli_version` | Files | Differences |
| --- | --- | --- |
| `0.162.0-alpha.17.2` | 10 | Only version with `compacted` lines. |
| `0.162.1` | 1 | No `agent_message` or `inter_agent_communication_metadata` lines. |
| `0.163.0-alpha.10.2` | 4 | Only version with `additional_tools`. |
| `0.163.0` | 46 | Newest. Latest installed. |

## Session meta

The first line of every file is `session_meta`. Keys of `payload`:

| Key | Kind | Notes |
| --- | --- | --- |
| `id` | string | Thread id; equals the id in the file name. |
| `session_id` | string | Equals `id` in 45 files; differs in the 16 sub-agent files (the root thread's id, not checked against a parent file). |
| `timestamp`, `cwd`, `model_provider` (`openai`), `cli_version` | string | `cwd` is an absolute Windows path. |
| `runtime_workspace_roots` | array | 1 or 2 paths. |
| `originator` | string | See below. |
| `source` | string or object | See below. |
| `thread_source` | string | `user` 44, `subagent` 14, `guardian_review` 2, `chatgpt_handoff` 1. |
| `history_mode` | string | `paginated` in all 61. |
| `base_instructions` | object | `text`, `provenance`. Large; do not display. |
| `context_window` | object | `window_id`. |
| `git` | object | `commit_hash`, `branch`, `repository_url`; absent in 5 of the 62 `session_meta` lines. |
| `creator_user_id`, `creator_account_id` | string | Account identifiers; never copy into the archive's metadata. |
| `parent_thread_id`, `agent_nickname`, `agent_path`, `multi_agent_version` | | Sub-agent files only. |
| `forked_from_id`, `subagent_history_start_ordinal` | | One file (a forked sub-agent). |

Interactive versus headless (the combination of three fields; `originator` is enough on its own):

| `originator` | `source` | `thread_source` | Files | Meaning |
| --- | --- | --- | --- | --- |
| `codex-tui` | `"vscode"` | `user` | 2 | The terminal TUI (interactive). Turn trigger `user`. |
| `codex_exec` | `"exec"` | `user` | 37 | `codex exec` (headless). Turn trigger `exec`, one turn per file, approval `never` (and `on-request` in some). |
| `Codex Desktop` | `"vscode"` | `user` | 5 | The desktop app. Turn trigger `composer`. |
| `codex_work_desktop` | `"vscode"` | `chatgpt_handoff` | 1 | A hand-off from ChatGPT. |
| `Codex Desktop` or `codex_exec` | `{"subagent": ...}` | `subagent` | 14 | A spawned agent. `source.subagent.thread_spawn` = `{parent_thread_id, depth, agent_path, agent_nickname, agent_role}`. |
| `codex_exec` | `{"subagent": {"other": "guardian"}}` | `guardian_review` | 2 | An auto-review thread. |

- `source: "vscode"` is not a VS Code marker: the terminal TUI and the desktop app both write it, probably because they talk to the local app-server. Use `originator` to tell clients apart. The VS Code extension's own `originator` value was not observed.
- Sub-agent and guardian threads have their own rollout files. A session list should show only `thread_source` of `user` (and `chatgpt_handoff`) and link the rest through `parent_thread_id`. The `thread_spawn_edges` table has the same links.

## Prompts and non-prompt user messages

Three places hold a user prompt:

1. `event_msg` / `item_completed` with `item.type == "UserMessage"` (160). `content` is an array of `{type: "text", text, text_elements: []}` or `{type: "local_image", path}`. This is the one to use: it appears once per prompt, inside its turn.
2. `response_item` / `message` with `role == "user"` and `content_item_kinds` of only `user.text` / `user.image` (170, including copies inherited by sub-agents). These also carry `metadata.user_input_order` (a number). `metadata.client_authored` is `false` on them, so do not use it as a flag.
3. `history.jsonl` (what looks like the composer's prompt history; its meaning is inferred from the key names).

`response_item` messages that are not prompts. They are told apart by `content_item_kinds` (present on every message) and by the absence of `metadata.user_input_order`:

| Role | `content_item_kinds` family | First-block marker | Count |
| --- | --- | --- | --- |
| `user` | `agents_md`, or `agents_md` + `environments` | plain text starting `# AGENTS.md instructions for`, then `<INSTRUCTIONS>`; the second block starts `<environment_context>` | 50 |
| `user` | `environments` | `<environment_context>` | 19 |
| `user` | `additional_content` | `<external_codex_apps_open_page>` | 32 |
| `developer` | `skills_instructions`, `permissions`, `collaboration_mode`, `multi_agent_role`, `multi_agent_mode`, `model_switch`, `app-context`, `codex_apps_client_time_context`, `codex_apps_open_page_instructions` | XML-style tag of that name | 263 (all developer messages) |

- Inside `UserMessage` items, 41 start with `<send_user_message_question_reply>` (the answer to a `request_user_input_async` question; show as a reply, not as a typed prompt) and 3 contain `<in-app-browser-context>` (desktop context attached to a prompt; strip or fold).
- A turn's first `UserMessage` is not always a typed prompt: sub-agent threads receive their task as an `agent_message` or as an inherited user message (`metadata.inherited_user_message: true`).
- `compacted.replacement_history` holds copies of earlier messages; do not scan it for prompts.
- Images: a pasted image is `local_image{path}` in the item and an `input_image` block with a `data:` URL in the `response_item`. Do not copy the URL into search indexes.

## Tool calls and apply_patch

A shell command is a `CommandExecution` item. Its id is `exec-<uuid>` when the command ran from inside the `exec` code tool (1,155) and the `call_...` id of an `exec_command` function call otherwise (112). Pair it with a call through the id only in the second case; for the first, the item is self-contained (`command`, `cwd`, `aggregated_output`, `exit_code`, `duration`).

`apply_patch` reaches the rollout in two ways:

| Route | Where the patch text is | Where the result is |
| --- | --- | --- |
| Top-level custom tool call, `response_item` / `custom_tool_call` with `name == "apply_patch"` (51) | `payload.input`, a string | `custom_tool_call_output` with the same `call_id`; `output` is a string `Exit code: 0\nWall time: <n> seconds\nOutput:\nSuccess. Updated the following files:\nM <path>` (letters `A`, `M`, `D` before each path). All 51 succeeded; no failure was seen. |
| Called inside the `exec` JavaScript tool as `tools.apply_patch(...)` with the envelope inline (254 `exec` inputs mention it) | In the `exec` call's `input`, mixed with code | Only in the `FileChange` item |

Both routes also produce a `FileChange` item (299 = 51 + 248). This is the source to use for the Files changed tab:

- Item id: the call's `call_id` for route 1, `exec-<uuid>` for route 2.
- `changes`: an object keyed by absolute Windows path (backslashes, 557 keys), each value one of:
  - `{type: "add", content}` (122),
  - `{type: "update", unified_diff, move_path}` (432; `unified_diff` starts with `@@`, is not git-style and has no `*** ` markers; `move_path` was null every time),
  - `{type: "delete", content}` (3).
- `status`: `completed` in all 299. `stdout` repeats the `Success. Updated the following files:` text, `stderr` is empty. Failed, declined and in-progress statuses were not seen.
- 1 to 8 files per item (1 file in 158 items).

The patch envelope in route 1 inputs: first line `*** Begin Patch`, last line `*** End Patch`, a trailing newline, per-file headers `*** Update File: <path>` (82) and `*** Add File: <path>` (8), hunks introduced by `@@`, and body lines prefixed ` `, `+` or `-`. The Codex binary also contains `*** Delete File: `, `*** Move to: ` and `*** End of File`, which this data does not show. A patch that fails to apply reports `apply_patch verification failed: ...` (string in the binary; not seen in a rollout).

Other tools: `McpToolCall` items give `server`, `tool`, `arguments` and `result`; web search is an `Extension` with `kind: "web.search"` and an `action` of `{type, query, queries}`, `{type, url}` or `{type, url, pattern}`.

## Turns

- A turn runs from `event_msg/task_started` to `event_msg/task_complete`, joined by `turn_id`. 140 starts, 139 completes; one file ends with an open turn (the CLI was closed or the turn was interrupted; no abort event exists).
- Every `UserMessage` item (160) falls between a `task_started` and its `task_complete`, and all 140 started turn ids have `response_item` lines.
- Turns without a user message exist (sub-agent and guardian threads; in 8 files the first two turn events are a bare `task_started`, `task_complete`).
- `task_started.turn_attribution`: `turn_trigger` (`user` 5, `composer` 83, `exec` 43, `send_user_message_async_question` 5, `chatgpt_handoff` 1, `guardian_review` 3), `parent_turn_id` (string or null), `initiating_agent_path` (string or null), `root_turn_id`. `root_turn_id` equals `turn_id` for root turns.
- A failed turn is a `task_complete` with `error` `{message, codex_error_info}` and `last_agent_message: null`. All 21 had `codex_error_info == "usage_limit_exceeded"`. `thread_turns.status` calls these `failed`.
- `turn_context` has one line per turn, sometimes more (146 lines for 140 starts): `turn_id`, `cwd`, `model`, `effort` (`low`, `medium`, `high`; absent in 6), `approval_policy` (`on-request`, `never`), `sandbox_policy.type` (`workspace-write`, `read-only`, `danger-full-access`), `collaboration_mode`, `summary`, `personality`.
- Compaction: a `compacted` line (with `window_number` 1 to 3, `window_id`, `previous_window_id`) and a `ContextCompaction` item. The conversation view can show one divider.
- An assistant text is an `AgentMessage` item. `phase: "commentary"` is a mid-turn note, `phase: "final_answer"` ends the turn (it equals `task_complete.last_agent_message`). Group a turn's items by `item.turn_id` (or by position between start and complete).

## Live writes and resume

- No Codex process was running during the survey, so a live append could not be watched. The evidence is indirect:
  - Codex's own projection tails each rollout by byte offset, and the offset equals the file size for all 54 uncompressed files: the files are append-only.
  - File modification times are not a reliable "last activity": for 44 of 49 short files the mtime equals the first line's timestamp and is up to 814 s before the last line's; only 5 long-lived files have mtime equal to their last line. Use the last line's `timestamp`, or the size, to detect growth.
- No session id appears in two files (0 duplicates among 61).
- Seven Codex Desktop files contain gaps of more than 30 minutes (up to 119 hours) between a `task_complete` and the next `thread_settings_applied`, `task_started` or `item_completed`; five files span 11 hours to 5 days. A continued desktop thread appends to its file; it does not start a new one. Whether `codex resume <id>` from the terminal does the same could not be tested without running it.
- A fork creates a new file: the one fork seen (a forked sub-agent) has two `session_meta` lines. The first (ordinal 0) is the new thread, with `forked_from_id`, `parent_thread_id` and `subagent_history_start_ordinal`; the second (ordinal 1) is the source thread's meta, followed by its copied history. A parser must take identity from the first `session_meta` only and treat the later one, and the lines before `subagent_history_start_ordinal`, as inherited.
- Resume relies on Codex's own store (`state_5.sqlite` locates the rollout), so a copy in Marshell's archive is for reading only. `codex resume <id>` needs `~/.codex` to still hold the thread.

## Claude Code entrypoints

`entrypoint` is on every `user`, `assistant` and `system` line, and it can change inside one file.

| `entrypoint` | Lines | Files (by the file's first such line) | Meaning |
| --- | --- | --- | --- |
| `cli` | 91,747 | 102 only, 2 more mixed | Interactive terminal. |
| `sdk-py` | 1,094 | 157 | Agent SDK for Python (headless). |
| `sdk-cli` | 223 | 33 only, 1 more mixed | `claude -p` / print mode (headless). |
| `claude-desktop` | 1 | 1 mixed | The desktop app (one line, after a resume). |
| `claude-vscode` | 0 | 0 | Not on this machine; the VS Code extension is expected to write a value of its own. Unverified. |

- Not markers on this machine: `userType` is `external` on all 93,065 lines, `isSidechain` is `false` on all of them (subagents live in other files).
- Other signals:
  - `turnOrigin`: `human` (874, `cli`) and `sdk` (183, `sdk-py` and `sdk-cli`); also `task_notification`, `scheduled`, `peer`, `system`, `auto_continuation`. Only on lines that start a turn.
  - `origin.kind` (`cli` lines): `human`, `task-notification`, `peer`, `plugin`, `auto-continuation`.
  - `sessionKind: "bg"` on 3,504 `cli` lines: a background session.
  - File head: all 191 files that start with a `queue-operation` `enqueue` then `dequeue` line are headless (`sdk-py` 157, `sdk-cli` 33, plus one `sdk-cli` then `cli` file). Interactive files start with `mode`, `last-prompt`, `permission-mode`, `file-history-snapshot`, `agent-color` or `ai-title` lines; their `queue-operation` lines come later.
  - `sdk-cli` and `sdk-py` lines have no `session_id` key, `slug`, `permissionDecision` (sdk-py) or `sessionKind`.
- Two files change entrypoint: `cli` then `claude-desktop`, and `sdk-cli` then `cli` (a headless run later resumed in the terminal). Classify a transcript by the entrypoint of its first `user` or `assistant` line, and keep the last one as the current client. One file holds one `sessionId` and its name stem is that id.
- Transcript count 294 (the 5a note counted 275 on 2026-10-23), oldest first line 2026-09-20, newest last line 2026-10-24, versions 2.1.278 to 2.1.309.

## Contradictions with codex-cli.md

- The note says the rollout format, compression and retention were still to be verified. This survey describes the format (above) and finds `.zst` rollouts, but not what decides when a file is compressed or removed.
- The note says rollout JSONL "is being migrated into `thread_history_1.sqlite`". In 0.163.0 every session still has a JSONL rollout and `history_mode: "paginated"` marks sessions whose database projection is built from it by byte offset. `thread_history_1.sqlite` is a derived index, not a replacement. `codex migrate-rollouts` is for sessions older than this format, none of which exist here.
- The note's installed version, `0.162.1`, is now `0.163.0` (the survey includes one `0.162.1` file).
- `session_meta.payload.source` (`vscode`, `exec`, `{"subagent": ...}`) is not the hook `source` (`startup`, `resume`, `clear`, `compact`) the note describes for `SessionStart`. Do not conflate them.
- The note's permission modes (`default`, `acceptEdits`, `plan`, `dontAsk`, `bypassPermissions`) are the hook vocabulary. Rollouts use `approval_policy` (`on-request`, `never`) with a `sandbox_policy.type`, and no `permission_mode`.
- The note lists "session resume/fork behavior" as unknown. Now known: the id is the file's uuid, forks write a new file with `forked_from_id`, continued desktop threads append to the same file. Terminal `codex resume` appending is untested.

## Open questions

- Does `codex resume <id>` append to the original rollout, and does it rewrite `cli_version` or add a new `session_meta`? Needs a live check (a scratch `CODEX_HOME`, a throwaway thread).
- What does the VS Code extension write for `originator` and `source`? And what does Claude Code's VS Code extension write for `entrypoint`?
- What does a failed or declined `apply_patch` look like in a `FileChange` item and in a `custom_tool_call_output`? Which `status` values besides `completed` exist?
- What triggers `.zst` compression, and does Codex rename the file in place (leaving `threads.rollout_path` stale, as seen)? Does `codex resume` read a `.zst` file?
- Is the interrupted turn (one open turn at the end of a file) an interruption by the user, and is there an event for it in other versions?
- Why are file modification times stale on appended files on Windows (appends without a metadata update, or a timestamp restored by Codex)? Marshell's watcher should not depend on them.
- Does an older `cli_version` (before 0.162) have a different shape? The docs' `event_msg` `user_message` / `agent_message` shapes were not seen, so the parser needs a version gate with a fixture per generation.
