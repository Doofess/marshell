# Usage history and MCP server start-up in the four CLIs (slice 7c2)

Read on 2026-11-06 for slice 7c2 of the Doctor: claude 2.1.318, codex-cli 0.164.0, agy 1.4.12 and qwen 0.26.5. Nothing was run or written in a CLI's home: SQLite files were copied to a scratch folder and read there, and no CLI or MCP server was started. It adds to `2026-11-06-mcp-and-skills.md` (7c1), which has the config files, off switches and listing caps.

Marks:
- **local:** seen on this PC, with counts.
- **source:** read in the CLI's source or shipped bundle at that version.
- **docs:** the CLI's documentation, with the page named.
- **unknown:** nothing found either way.

## Claude Code

- **Records** (local, every file). Each user, assistant, attachment and system record has `timestamp` (ISO UTC with `Z`) and `cwd`. Bookkeeping records (queue-operation, last-prompt, ai-title and the like) have neither.
  - Records aren't strictly in time order.
  - `cwd` drifts: 69 of about 390 main files have 2 to 16 cwds, so the record's cwd is the place, not the project folder's name.
  - Subagent files (`<folder>/<session>/subagents/*.jsonl`) have the same records, with `isSidechain: true`.
  - Aside files `*.orphaned-….jsonl` and `*.jsonl.superseded-…` exist; their records repeat others'.
- **Volume** (local): about 1,600 files modified in the last 30 days, 607 MB. The oldest transcript is from 2026-10-09: `cleanupPeriodDays` is unset, so 30 days.
- **An MCP call** is `{"type":"assistant","message":{"content":[{"type":"tool_use","name":"mcp__<server>__<tool>",…}]}}` (local: 2,842 calls in main files, 197 in subagent files).
  - `<server>` is the display name with every character outside `[A-Za-z0-9_-]` turned into `_`. A plugin server's display name is `plugin:<plugin>:<server>`, so it reads `plugin_<plugin>_<server>` (docs `mcp`, and local against the announced names).
  - The mapping loses information, so two names can collide.
  - `mcp__` names also appear outside `tool_use`, in `deferred_tools_delta` and `api-request-*` records. Only `tool_use` blocks are calls.
  - claude.ai connectors are `mcp__claude_ai_<Name>__…`. They aren't in any config file.
- **A skill use:**
  - The `Skill` tool's `input` is `{"skill": "<name>"}` or `{"skill", "args"}` (local: 454 calls, about 80 in subagent files).
  - Names come bare (`marshell-ship`), plugin-qualified (`superpowers:writing-plans`), or scoped to a worktree (`.claude/worktrees/phase-6d:marshell-ship`, 12 calls).
  - A typed command is a `type: "user"` record whose `message.content` is a string starting with `<command-`. The name is in `<command-name>/<name></command-name>`. Built-ins (`/compact`, `/mcp`) look the same, so a match must be checked against the skills. The same tags also appear inside tool results, which aren't string content.
  - `~/.claude.json` has `skillUsage: {"<name>": {usageCount, lastUsedAt}}` (45 names, `lastUsedAt` in epoch ms), with no project.
- **Availability** (local). These are `type: "attachment"` records, also present in subagent files.
  - `deferred_tools_delta.addedNames` lists the tool names a session had, `mcp__…` ones included. It is written at session start (355 of about 389 main files) and again when the tool set changes.
  - `skill_listing.names` lists the skill names, with `isInitial: true` at session start.
  - `mcp_instructions_delta.addedNames` lists the display names of servers that send instructions.
  - `deferred_tools_record` holds definitions surfaced later and isn't an availability list.
- **Approving a project's `.mcp.json` server** (docs `mcp`, `settings`):
  - The person approves it at a prompt.
  - `enableAllProjectMcpServers` and `enabledMcpjsonServers` count from `~/.claude/settings.json`, managed settings and `.claude/settings.local.json`.
  - They count from the committed `.claude/settings.json` only in a folder whose trust dialog was accepted, which `~/.claude.json` records per project as `hasTrustDialogAccepted`.
  - `disabledMcpjsonServers` anywhere rejects the server.
  - Where the prompt's answer is stored isn't documented. On this PC, each project entry in `~/.claude.json` has `enabledMcpjsonServers` (all empty), and Airport's `.claude/settings.json` holds `enabledMcpjsonServers` (local).
- **Start-up** (docs `mcp`):
  - `${VAR}` and `${VAR:-default}` are expanded in `command`, `args`, `env`, `url` and `headers`. An unset variable with no default stays as written, with a warning. Airport's `.mcp.json` uses `${CLAUDE_PROJECT_DIR:-.}`.
  - `MCP_TIMEOUT` is the start-up timeout in ms; no default is documented.
  - A plain `npx` command works: the playwright plugin's `.mcp.json` uses it, with 291 calls (local). The docs don't say how.
- **What loads up front** (docs `mcp`): with tool search on (the default; `ENABLE_TOOL_SEARCH` unset), only tool names and server instructions.
  - `ENABLE_TOOL_SEARCH=false` loads every definition, and `auto` or `auto:N` loads them while they total under N% (default 10) of the context window.
  - A server's `alwaysLoad: true`, or a tool's `_meta["anthropic/alwaysLoad"]`, loads that one up front.

## Codex

Source: `openai/codex` at tag `rust-v0.164.0`, under `codex-rs/`.

- **`~/.codex/thread_history_1.sqlite`** (local, read from a copy):
  - Tables: `_sqlx_migrations` (8 rows), `thread_turns`, `thread_items` (6,377 rows), `thread_history_projection_state` and `thread_realtime_items`.
  - `thread_items`: `thread_id`, `turn_id` and `item_id` form the key. The other columns are `rollout_ordinal`, `created_at_ms` (epoch ms, UTC), `item_json`, `item_type`, `updated_at_ordinal`, `started_at_ms` and `completed_at_ms`.
  - Rows run from 2026-10-10T19:46Z to 2026-11-03T19:57Z.
  - It outlives the rollouts: items for 100 threads, but only 32 rollout files.
  - `PRAGMA user_version` is 0. The schema comes from sqlx migrations 0001–0008 (`state/src/migrations.rs`). `state/src/runtime.rs:364` calls it a "rebuildable paginated thread-history database", written by `thread-store/src/local/thread_history.rs:447`.
  - All 101 threads are `history_mode = 'paginated'`. Whether `legacy` threads reach this database is unknown.
- **A thread's folder:** `thread_items.thread_id` is `threads.id` in `~/.codex/state_5.sqlite`, whose `threads.cwd` is NOT NULL and starts with `\\?\` (local, 100 of 100).
- **An MCP call** is a row with `item_type = 'mcpToolCall'`. Its `item_json` keys are `type`, `id`, `server`, `tool`, `status`, `arguments`, `appContext`, `mcpAppUi`, `pluginId`, `readOnlyHint`, `result`, `error` and `durationMs` (local, 413 rows).
  - `server` is the config key or the plugin `.mcp.json` key (local for plugins; source `codex-mcp/src/mcp/mod.rs:85` for config servers, with no local call).
  - The built-in `codex_apps` server has 365 calls and isn't in any config.
- **A skill use** has no event of its own. It is a `commandExecution` row whose `command` string reads a `SKILL.md`: `Get-Content 'C:\…\SKILL.md'`, or `-LiteralPath 'C:/…/SKILL.md'` with forward slashes (local: 208 rows in 66 threads).
  - 62 other rows mention `SKILL.md` only in their output, which isn't use.
  - The explicit `$skill` message `<skill><name>…</name><path>…</path></skill>` had no row here (source `ext/skills/src/fragments.rs:105`).
- **Rollouts.** Each line is `{"timestamp","ordinal","type","payload"}`. `session_meta.payload.cwd` and `turn_context.payload.cwd` hold the folder, without `\\?\`.
  - An MCP call is an `event_msg` `item_completed` whose item has `type: "McpToolCall"`; the remaining 32 rollouts have none.
  - A skill read is an `event_msg` `CommandExecution` item (`command` as an argv list), or a `response_item` `custom_tool_call` whose input runs `Get-Content …SKILL.md`.
  - Every rollout lists all skills in `<skills_instructions>`, so a `SKILL.md` path there isn't use.
- **Availability.** Rollouts list the skills, and directly exposed tools in `additional_tools`. Deferred servers aren't listed: the `tools` world-state section is feature-gated and absent here. So there is no history record of a deferred server being available.
- **Trust** (source):
  - `[projects.'<path>'] trust_level = "trusted"` (`config/src/config_toml.rs:612`). Keys match case-insensitively on Windows, with `\\?\` stripped.
  - The lookup tries the folder, then the project root, then the git root (`config/src/loader/mod.rs:1080-1117`).
  - An untrusted project's `.codex/config.toml` is read but disabled (`:1159-1170`).
- **Starting a stdio server** (source `rmcp-client/src/utils.rs:16-58`, `stdio_server_launcher.rs:262-283`):
  - The environment starts empty.
  - It then gets `WINDOWS_CORE_ENV_VARS` from Codex's own environment: PATH, PATHEXT, SHELL, COMSPEC, SYSTEMROOT, WINDIR, SYSTEMDRIVE, USERNAME, USERDOMAIN, USERPROFILE, HOMEDRIVE, HOMEPATH, PROGRAMFILES, PROGRAMFILES(X86), PROGRAMW6432, PROGRAMDATA, LOCALAPPDATA, APPDATA, TEMP, TMP, TMPDIR, POWERSHELL and PWSH.
  - Next come the names listed in `env_vars`, and the `env` table, which overrides.
  - The program is resolved with `which` on that PATH, so `npx` finds `npx.cmd`.
  - The working folder is the entry's `cwd`, else the session's.
  - The start-up timeout is `startup_timeout_sec` (or `startup_timeout_ms`), else 30 s.
  - `initialize` sends `protocolVersion` `"2025-06-18"`.
- **What loads up front** (source):
  - A server is deferred when the model supports tool search (`core/src/mcp_tool_exposure.rs:158`).
  - A plugin `.mcp.json` with `omit_tools_from: ["deferred"]` is listed directly. `cua_repl` does that, and lists only its `enabled_tools`.
  - A deferred server adds at most a namespace line, `- mcp__<server>: <first line of a description, up to 250 characters>`, and only when the `DeferredToolWorldState` feature is on (`core/src/context/world_state/tools.rs`). Where that description comes from wasn't traced.

## Antigravity

- **Transcripts** (local):
  - `brain/<id>/.system_generated/logs/transcript_full.jsonl` holds the same lines as `transcript.jsonl`, without truncation.
  - Each line has `type`, `source`, `status`, `step_index` and `created_at` (UTC), and model lines add `tool_calls[{name, args}]`.
  - There are 5 conversations here, all from 2026-10-10.
- **The workspace isn't in the transcript.** `~/.gemini/antigravity-cli/history.jsonl` has `{display, timestamp, workspace, conversationId?}` per prompt; 2 of its 15 lines have no `conversationId` (local). `conversation_summaries.db` has `workspace_uris` too.
- **MCP calls:** none seen; the user `mcp_config.json` is empty.
  - The binary's meta-tool is `call_mcp_tool`, with required `ServerName`, `ToolName` and `Arguments` (binary strings).
  - How the call appears in a transcript is unknown.
- **No availability record:** a transcript doesn't list the skills or tools a conversation had.
- **Workspace servers:**
  - The shipped docs don't mention `<workspace>/.agents/mcp_config.json`, and whether trust gates them is unknown.
  - `settings.json` has `trustedWorkspaces` (local, 3 paths), and the CHANGELOG says workspace hooks load after the folder is trusted.
  - The CHANGELOG says plugin servers expand variables in command, args, env and headers. For a plain `mcp_config.json` this is unknown.
  - The connection timeout is 60 s (CHANGELOG).
- **What loads up front:** MCP tools load on demand (CHANGELOG), and `_meta {"antigravity/alwaysLoad": true}` opts a tool out. What else is listed up front is unknown.

## Qwen Code

Source: the JS chunks in `…\AppData\Local\qwen-code\qwen-code\lib\chunks\`. There is no `~/.qwen` on this PC, so nothing here is local.

- **Chat records** (`chunk-XBXDDFWT.js`, `createBaseRecord`): `{uuid, parentUuid, sessionId, timestamp (ISO), type, provenance, cwd, version, gitBranch}`.
  - `type` is `user`, `assistant`, `tool_result` or `system`.
  - A call is a part `functionCall: {id, name, args}`.
  - The folder name is `cwd` lowercased on Windows, with every non-alphanumeric character turned into `-`. That loses information, so use the record's `cwd`.
  - The base folder is `QWEN_RUNTIME_DIR`, else `QWEN_HOME`, else `~/.qwen`. Skip `*.ledger.jsonl`.
- **A skill use** is a call to the tool `skill` with args `{skill, args?}` (`skill-O6SRMNJM.js`). A typed `/name` is sent as a plain prompt and leaves no call (bundled `skills.md`).
- **MCP tool names** (`chunk-7DOOXUDG.js`, `normalizeToolNameForProvider`):
  - `mcp__<server>__<tool>`, kept as is when it is at most 63 characters and matches `^[A-Za-z][A-Za-z0-9_-]*$`.
  - Otherwise, characters outside `[A-Za-z0-9_-]` become `_`, a leading non-letter gets `tool_`, and the name is cut, with a 7-character hash suffix.
  - A deferred tool is called through a bridge tool that takes `{name, arguments}`. That this shows as a `functionCall` named `tool_call` with the MCP name in `args.name` is inferred, not seen.
- **Approval** (`chunk-CPBGKA77.js`):
  - Servers in `<project>/.mcp.json` (scope `project`) and in `<project>/.qwen/settings.json` (scope `workspace`) need approval. User-level ones don't.
  - Approvals are kept in `~/.qwen/mcpApprovals.json` (or `$QWEN_CODE_MCP_APPROVALS_PATH`) as `{"<project root, lowercased on Windows>": {"<server>": {hash, status}}}`, with `status` `approved` or `rejected`.
  - `hash` is the SHA-256 of the entry's JSON with `scope`, `extensionName` and `description` left out and keys sorted. Editing an entry makes it pending, and a pending server doesn't start.
  - With `security.folderTrust.enabled` (off by default), an untrusted folder starts no server at all.
- **Starting a stdio server:**
  - It inherits the whole process environment, then the entry's `env`.
  - `$VAR` and `${VAR}` are expanded in every string of a `settings.json` entry, but not in `.mcp.json`. Unknown names stay literal.
  - `cross-spawn` with `shell: false` runs it, in the entry's `cwd`.
  - Discovery times out after 30 s for stdio.
- **What loads up front** (`chunk-B3ZFUGTC.js`, `buildDeferredToolsReminder`): with `tools.toolSearch.enabled` (the default), a `#### <server>` heading, then one `- "<tool>": "<first line of its description, at most 160 characters>"` line per tool. A server's `alwaysLoadTools: true` loads its full schemas.

## What this means for 7c2

- **Claude Code:**
  - Its calls, typed skills and availability records are all in its transcripts, each record with its own time and folder.
  - Its 30-day cleanup leaves under 30 days on disk, so coverage needs Marshell's own memory (decision 14).
- **Codex:**
  - Its complete history is in `thread_history_1.sqlite`, an internal database with no stable contract. It is read only, from a schema that is checked first, and rollouts are read too.
  - Skill use is a `SKILL.md` read, matched by path.
  - A deferred server has no availability record, so its "added" date is Marshell's first sight (decision 15).
- **Antigravity:** its skills are judged by `SKILL.md` reads, and its servers aren't (decision 16).
- **Qwen Code:** its format is from source only.
- **Start-up differs per CLI:**
  - environment: Codex starts from an allow-list, the others inherit;
  - variable expansion: Claude Code's `${VAR:-default}`, Qwen Code's `$VAR`;
  - approval: Claude Code's `enabledMcpjsonServers`, Codex's trusted projects, Qwen Code's `mcpApprovals.json` with a hash.
