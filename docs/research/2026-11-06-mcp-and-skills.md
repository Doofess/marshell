# MCP servers and skills in the four CLIs (slice 7c)

Read on 2026-11-06 for slice 7c of the Doctor: claude 2.1.318, codex-cli 0.164.0, agy 1.4.12 and qwen 0.26.5. Nothing was written to a real CLI config. The `agy` and `qwen` write tests ran with their homes pointed at a scratch folder.

Marks:
- **local:** seen on this PC.
- **source:** read in the CLI's source or shipped bundle at that version.
- **docs:** the CLI's documentation, with the page named.
- **unknown:** nothing found either way.

## Claude Code

Docs pages: `code.claude.com/docs/en/` `mcp`, `managed-mcp`, `skills`, `settings-reference`, `claude-directory`, `plugins/loading`, `tools-reference`.

- **Servers** (docs, `mcp`).
  - Where they live:
    - Local scope, the default: `~/.claude.json` under `projects["<path>"].mcpServers`.
    - User scope: top-level `mcpServers` in `~/.claude.json`.
    - Project scope: `<project>/.mcp.json`, under `mcpServers`.
    - Plugins: the plugin's `.mcp.json`, or inline in its `plugin.json`.
    - `managed-mcp.json` (`C:\Program Files\ClaudeCode\`, `/etc/claude-code/`) takes exclusive control when present.
  - Precedence: local, then project, then user, then plugin, then claude.ai connectors. Entries aren't merged.
  - Entry shapes:
    - stdio: `command`, `args`, `env`, with an optional `type: "stdio"`.
    - remote: `type: "http"` (or its alias `streamable-http`) or `type: "sse"`, with `url` and `headers`.
    - An entry with a `url` and no `type` is read as stdio, which is a config error.
- **Turning one server off** (docs, `mcp`):
  - Per project, `disabledMcpServers` in that project's entry in `~/.claude.json`. The `/mcp` toggle writes it. It covers user, local, plugin and managed servers.
  - A user-scope server has no global off switch, only this per-project one.
  - A `.mcp.json` server is turned off with `disabledMcpjsonServers` (an array of names) in a settings file. The project's `.claude/settings.local.json` is the per-user one.
  - unknown: whether Claude Code overwrites a `disabledMcpServers` another program added to `~/.claude.json` while a session runs. Claude Code rewrites that file often.
- **Turning one skill off** (docs, `skills`): `skillOverrides: {"<name>": "off"}` in a settings file.
  - The other values are `"on"` (the default), `"name-only"` and `"user-invocable-only"`.
  - `/skills` writes it to `.claude/settings.local.json`.
  - `skillOverrides` doesn't reach plugin skills.
- **Skill folders** (docs, `skills`):
  - `~/.claude/skills/<name>/SKILL.md` and `<project>/.claude/skills/<name>/SKILL.md`. Project skills are also found in parent folders up to the repo root.
  - Plugin skills are `<plugin>/skills/<name>/SKILL.md`, named `plugin:skill`.
  - `CLAUDE_CONFIG_DIR` moves `~/.claude` and `.claude.json`.
  - Claude Code doesn't read `.agents/skills` (`2026-10-30-cli-skill-folders.md`).
- **What loads up front** (docs, `skills` and `mcp`):
  - Each skill's `description` plus `when_to_use`, cut at 1,536 characters (`skillListingMaxDescChars`).
  - The whole listing is capped at 1% of the context window. Past it, descriptions are dropped, least-used first; names stay.
  - `disable-model-invocation: true` keeps a skill out of the listing.
  - MCP tool search is on by default: only tool names and server instructions load at the start.
    - `ENABLE_TOOL_SEARCH=false` loads every definition.
    - `auto` loads them while they total under 10% of the context window.
    - A server's `alwaysLoad: true`, or a tool's `_meta["anthropic/alwaysLoad"]`, loads that one up front.
  - Descriptions loaded up front are cut at 4,096 characters.
- **Plugins** (docs, `plugins/loading`):
  - Installed under `~/.claude/plugins/cache/<marketplace>/<plugin>/<version>/`, and recorded in `installed_plugins.json`.
  - Enabled per settings file as `enabledPlugins: {"<name>@<marketplace>": true}`.
- **Transcripts.**
  - Files: `~/.claude/projects/<folder>/<session>.jsonl`, with subagents under `<session>/subagents/`. `cleanupPeriodDays` defaults to 30.
  - An MCP call is a `tool_use` named `mcp__<server>__<tool>`:
    - characters such as `.` and spaces in the server name become `_`;
    - a plugin server is `mcp__plugin_<plugin>_<server>__<tool>`.

    (local, docs)
  - A skill the model uses is a `tool_use` named `Skill` with `input: {"skill": "<name>"}`. A plugin skill reads `plugin:skill`, for example `superpowers:writing-plans` (local).
  - A skill typed as a slash command shows as `<command-name>/<name></command-name>` in a user line (local). Built-in commands such as `/compact` look the same, so a match must be checked against the skill list.
  - Size: 1,578 transcript files, 3.2 GB, were written in the last 30 days on this PC (local).
- **Claude Code's own tools:** `/doctor` estimates the skill listing's cost, and `/skill-doctor` (2.1.252+) reports skill use and cost (docs, `skills`).

## Codex

Source: `openai/codex` at tag `rust-v0.164.0`, under `codex-rs/`. Docs: `learn.chatgpt.com/docs/extend/mcp` and `/docs/build-skills`.

- **Servers.**
  - Where they live: `~/.codex/config.toml` and, for trusted projects only, `<project>/.codex/config.toml`, as `[mcp_servers.<name>]`.
  - Entry shapes (source, `config/src/mcp_types.rs` l.229-311):
    - stdio: `command`, `args`, `env`, `env_vars`, `cwd`.
    - remote: `url`, `bearer_token_env_var`, `http_headers`, `env_http_headers`, `auth`.

    This PC has `node_repl` (local).
  - Plugin servers come from the plugin's `.mcp.json`, found through `.codex-plugin/plugin.json`. They have no config.toml entry, for example `cua_repl` and `codex_app` (local).
- **Turning one server off:** `enabled = false` in its table (source, `mcp_types.rs:240`).
  - `codex mcp list --json` reports `enabled` and `disabled_reason`, without connecting and without tools (local).
  - A whole plugin: `[plugins."<id>@<market>"] enabled = false`.
- **Turning one skill off:** a `[[skills.config]]` entry with `path = "<absolute SKILL.md>"` or `name = "<name>"`, and `enabled = false` (docs, `build-skills`). One selector per entry; the last match wins; it takes effect at restart. A user-level entry can name a project skill by its path.
- **Skill folders:**
  - repo `.agents/skills` (from the cwd up to the repo root);
  - `~/.agents/skills`;
  - `/etc/codex/skills`;
  - `$CODEX_HOME/skills` (deprecated);
  - bundled system skills;
  - plugin skills at `~/.codex/plugins/cache/<market>/<plugin>/<ver>/skills/`.
- **What loads up front:** name, description and path for each skill (source, `ext/skills/src/render.rs:24`).
  - A description is cut at 1,024 characters, and a name at 64.
  - The list is capped at 2% of the context window (`skills.max_context_tokens`, at most 10,000), or 8,000 characters when the window is unknown.
- **MCP tool definitions** (source, `core/src/mcp_tool_exposure.rs:158`):
  - They are deferred behind `tool_search` when the model supports it; every model in this PC's `models_cache.json` does.
  - But every local rollout lists the `cua_repl` plugin server's two tools (about 4.5k characters) directly, with no deferral marker (local). So deferral isn't universal, and the cause is unknown.
- **History.**
  - Rollouts are `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`.
  - An MCP call there is an `event_msg` `item_completed` whose `item` is `{"type":"McpToolCall","server","tool","status","pluginId",…}` (source, `protocol/src/items.rs:457`; `rollout.rs` l.231 reads it).
  - Older rollouts carry `mcp_tool_call_begin` / `mcp_tool_call_end`.
  - No MCP call is in this PC's rollouts. Only 32 rollout files remain, while 69 of the 101 threads in `state_5.sqlite` have no rollout file (local; the cause is unknown).
  - `~/.codex/thread_history_1.sqlite` table `thread_items` holds the complete item history (local):
    - 6,306 rows from 2026-10-10 to 2026-11-03;
    - 413 rows with `item_type = 'mcpToolCall'` and `item_json` `{"type":"mcpToolCall","server":"codex_apps","tool":…,"pluginId":…}`;
    - by server, `codex_apps` (built in, absent from `codex mcp list`) has 365, `cua_repl` 35 and `codex_app` 13.
- **Skill use** has no event of its own:
  - It shows as a `commandExecution` that reads `…/SKILL.md`; 267 rows in `thread_items` mention it (local).
  - An explicit `$skill` injects a user message wrapped in `<skill><name>…</name><path>…</path>…</skill>` (source, `ext/skills/src/fragments.rs:105`).
  - Every rollout lists all skills in its instructions, so a skill's name in a rollout isn't use.
- **Retention:** no setting deletes sessions by age (`2026-11-05-transcript-retention.md`).

## Antigravity

Sources: the docs agy ships in `~/.gemini/antigravity-cli/builtin/skills/agy-customizations/docs/`, its `CHANGELOG.md`, strings in `agy.exe`, and `agy mcp` run against a scratch home.

- **Servers:** `~/.gemini/config/mcp_config.json`, `<workspace>/.agents/mcp_config.json` and `plugins/<name>/mcp_config.json`, each with `{"mcpServers": {...}}` (docs).
  - Entry shapes:
    - stdio: `command`, `args`, `env`, `cwd`;
    - remote: `serverUrl` or `url`, `headers`, `oauth`; Streamable HTTP only.
  - Comments and trailing commas are accepted.
  - The user file on this PC exists and is empty (local).
- **Turning one server off:** `"disabled": true` on the entry. `agy mcp disable <name>` wrote exactly that in the scratch home (local). Plugin servers are named `<plugin>_<server>` (CHANGELOG).
- **Skills:**
  - Folders:
    - `<root>/.agents/skills/<name>/SKILL.md` (and `.agent/skills`);
    - `~/.gemini/config/skills/`, `~/.gemini/antigravity-cli/skills/` and `~/.gemini/skills/`;
    - `plugins/<p>/skills/`;
    - built-ins in `~/.gemini/antigravity-cli/builtin/skills` (docs, local).
  - No key turns one skill off. A `skills.json` entry takes `exclude` patterns, and `disable-slash-command` hides a skill from the `/` menu only (docs).
  - No description limit is documented. The `SKILL.md` body is limited to 24 KB.
- **MCP tool definitions** are found on demand by default; `_meta {"antigravity/alwaysLoad": true}` opts a tool out (CHANGELOG; the binary has `tool_search`).
- **Transcripts:** `~/.gemini/antigravity-cli/brain/<convId>/.system_generated/logs/transcript.jsonl`, with `tool_calls[{name, args}]` per step (local).
  - A skill use is a `view_file` call whose `args.AbsolutePath` ends in `SKILL.md` (local, 2 cases).
  - unknown: an MCP call's line. The binary has a `call_mcp_tool` meta-tool with `ServerName` and `ToolName`. `request.rs` l.70's `mcp_<server>_<tool>` has no evidence either way.
  - Retention is unknown.

## Qwen Code

Sources: the docs bundled in `…\AppData\Local\qwen-code\qwen-code\lib\bundled\qc-helper\docs\`, and the JS chunks in `lib\chunks\`. `~/.qwen` doesn't exist on this PC.

- **Servers:** `mcpServers` in `~/.qwen/settings.json` and `<project>/.qwen/settings.json` (`QWEN_HOME` moves the base) (docs, local).
  - A project `.mcp.json` is read too, after approval.
  - Extensions bring their own servers.
  - Entry shapes:
    - stdio: `command`, `args`, `env`, `cwd`;
    - remote: `httpUrl` (streamable HTTP) or `url` (SSE), and `headers`.
  - Other keys: `includeTools`, `excludeTools`, `alwaysLoadTools`, `trust`, `timeout`.
- **Turning one server off:** its name in `mcp.excluded` in settings (source, `isMcpServerDisabled`). The list takes globs and wins over `mcp.allowed`. There's no per-entry `disabled` field. `qwen mcp list` connects to every server and ignores `mcp.excluded` (local).
- **Turning one skill off:** `skills.disabled: ["<name>"]` (docs). It's a union across scopes, case-insensitive, and matches an extension skill as `ext:name` or `name`.
- **Skill folders:**
  - `~/.qwen/skills/` and `~/.agents/skills/`;
  - project `.qwen/skills/` and `.agents/skills/`;
  - extension skills, named `<ext>:<name>`;
  - bundled skills.
- **What loads up front:** an `<available_skills>` block with each name and description (source).
  - Past 8,000 characters (`MAX_SKILL_LISTING_CHARS`), non-bundled descriptions are cut to their first line.
  - A skill whose frontmatter has `paths:` globs is hidden until a matching file is touched.
- **MCP tool definitions** are deferred by default (`tools.toolSearch.enabled = true`); a server's `alwaysLoadTools: true` declares its tools up front (docs).
- **Chats:** `<QWEN_HOME>/projects/<sanitized cwd>/chats/<session>.jsonl`, kept by default (source).
  - A tool call is `message.parts[].functionCall{name, args}`.
  - An MCP tool is named `mcp__<server>__<tool>`, with characters outside `[A-Za-z0-9_-]` turned into `_`.
  - A skill use is a call to the tool `skill` with the name in its args; the arg's key is from the docs only.
  - A skill typed as `/name` leaves no tool call.
  - No local sample exists.

## What this means for 7c

- All four CLIs defer MCP tool definitions by default. So a server's up-front cost is mostly its tool names and instructions, unless the person opted out (`ENABLE_TOOL_SEARCH`, `alwaysLoad`, `alwaysLoadTools`), or the CLI lists the server directly, as Codex does for `cua_repl`.
- The per-skill and listing caps differ: Claude Code 1,536 per entry and 1% of context; Codex 1,024 and 2%; Qwen Code 8,000 for the whole listing; Antigravity none documented.
- Each CLI's off switch, and which ones only a shared project file can hold, are in PLAN §9's slice 7c decisions.
- 7c2's usage scan:
  - Claude Code's transcripts are large and need an incremental scan.
  - Codex's complete history is in `thread_history_1.sqlite`, not in the rollouts.
  - Antigravity's MCP call line still has to be seen once.
  - Qwen Code's needs a sample chat.
