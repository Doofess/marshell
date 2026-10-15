# Local install check: Codex CLI and Antigravity CLI (2026-10-15)

Read-only check of the two CLIs installed on the Windows 11 dev machine, to confirm or contradict `docs/research/codex-cli.md` and `docs/research/antigravity-notes/`. Method: `--version`/`--help` only, directory listings (names and sizes), non-secret config files with values redacted, and string scans (`rg -a`) of the two binaries. No interactive session, no model call, no hook capture, nothing written under any user config directory. Secrets (`auth.json`, `oauth_creds.json`, `google_accounts.json`, `installation_id`, `*.sqlite`/`*.db`, history, session, rollout, transcript and log files) were listed by name only, never read.

"Verdict" compares the existing research doc with what the install shows: **confirmed** / **contradicted** / **not determinable locally**. Anything not directly observed is marked *not verified*.

## Binaries found

| CLI | Path | Version | Notes |
|---|---|---|---|
| Codex | `C:\Users\Home Office\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe` (333 MB) | `codex-cli 0.162.1` | On the Windows PATH (`where.exe codex`). Sibling `codex-code-mode-host.exe`. |
| Codex (desktop runtime copy) | `C:\Users\Home Office\AppData\Local\OpenAI\Codex\bin\9614f1d9b40d9c4f\codex.exe` | `codex-cli 0.162.0-alpha.17.2` | Referenced by `config.toml` (`CODEX_CLI_PATH` in an MCP server env). Siblings: `codex-command-runner.exe`, `codex-windows-sandbox-setup.exe`, `..\77fbd4d52e601e0a\rg.exe`. |
| Codex (MSIX aliases) | `%LOCALAPPDATA%\Microsoft\WindowsApps\OpenAI.Codex_2p2nqsd0c76g0\` | - | `codex-chrome-native-host.exe`, `codex-core-command-runner.exe` app-execution aliases. No npm, scoop, volta, pnpm or winget-links install; no Start-menu or registry uninstall entry. |
| Antigravity | `C:\Users\Home Office\AppData\Local\agy\bin\agy.exe` (193 MB, Go binary) | `1.3.3` | On the Windows PATH (`where.exe agy`). Only file in `%LOCALAPPDATA%\agy`. `~\.gemini\antigravity-cli\bin\agentapi.bat` wraps `agy.exe agentapi %*` (subcommand not listed in `--help`). No Start-menu or registry entry. |

`CODEX_HOME` is not set in the environment (`$env:CODEX_HOME` empty), so Codex uses the default `C:\Users\Home Office\.codex`.

## Codex

### 1. Version, `config.toml`, hooks present

- Version: `codex-cli 0.162.1` (`codex --version`).
- `C:\Users\Home Office\.codex\config.toml` exists (3,017 bytes). Top-level keys and tables (values redacted): `model`, `model_reasoning_effort`, `notify` (array: path to a bundled `codex-computer-use.exe` helper plus the literal `"turn-ended"`), `[tui]`, `[windows]`, `[projects.'<path>']` x2 (`trust_level`; one is the Marshell repo), `[desktop]`, `[marketplaces.<name>]` x2, `[plugins."<name>@<marketplace>"]` x12 (`enabled`), `[mcp_servers.node_repl]` with `[mcp_servers.node_repl.env]`.
- No `[hooks]` table and no `[[hooks.<Event>]]` entries. No `C:\Users\Home Office\.codex\hooks.json`. The only `hooks.json` under `.codex` is a plugin cache file, `.codex\.tmp\plugins\plugins\figma\hooks.json`, whose shape is `{"hooks": {"PostToolUse": [{"matcher": "Write|Edit", "hooks": [{"type": "command", "command": "..."}]}]}}` - the event-nested JSON shape with matcher groups.
- `.codex\version.json`: `latest_version` = 0.162.1, `last_checked_at`, `dismissed_version` (update-check state only).
- Note for the onboarding plan: the user's `notify` key is already in use by the desktop app's computer-use plugin. Marshell must not treat `notify` as free.

### 2. Hook events, config shape, trust, `project_doc_max_bytes`, `CODEX_HOME`

No JSON schema, README or docs are shipped in the install folder (`AppData\Local\Programs\OpenAI\Codex` holds only `bin\`; the only `README.md` under `AppData\Local\OpenAI\Codex` belongs to the bundled node runtime). Evidence below is from `--help` and strings embedded in `codex.exe`.

- Hook event names (binary, `HookEventsToml` enum): `PreToolUse PermissionRequest PostToolUse PreCompact PostCompact SessionStart SessionEnd UserPromptSubmit SubagentStart SubagentStop Stop Interrupt`. Twelve events; `PermissionRequest` is present. **Confirmed** against `codex-cli.md`.
- Hook config shape (binary): matcher-group fields `matcher`, `hooks`; per-event state `enabled`, `trusted_hash` (`HookStateToml`). Handler is an internally tagged enum `HookHandlerConfig` with variants `command` (fields `command`, `commandWindows`, `timeout`, `async`, `statusMessage`, `additionalContextLimit`) and `mcp_tool` (fields `server`, `tool`, `input`, `timeout`, `statusMessage`). `commandWindows` **confirmed**. `mcp_tool`, `async`, `statusMessage`, `additionalContextLimit` are *not in the research doc* (new observation; semantics not verified).
- Hook sources (binary strings): `hooks.json`, `hooks/hooks.json`, `plugin.json#hooks[`, "failed to parse TOML hooks in ...", plus `CLAUDE_PLUGIN_ROOT`/`CLAUDE_PLUGIN_DATA` adjacent to the plugin hook loader. Both `hooks.json` and TOML hook parsing exist. The exact user-level path (`~/.codex/hooks.json`) was not found as a literal; **not determinable locally** beyond the generic parser strings.
- Common hook input fields (binary, `hooks\src\registry.rs`): `session_id transcript_path cwd hook_event_name permission_mode source agent_transcript_path stop_hook_active agent_id last_assistant_message tool_input reason turn_id model`. Permission-mode values seen: `default`, `acceptEdits`, `dontAsk`, `bypassPermissions` (`plan` not grepped). **Confirmed**.
- `PermissionRequest` output (binary, `PermissionRequestDecisionWire`): `behavior`, `updatedInput`, `updatedPermissions`, `interrupt`, `message`; error strings "PermissionRequest hook returned unsupported updatedPermissions", "...unsupported interrupt:true", "PermissionRequest hook denied approval". Also `hookSpecificOutput` key, and `UserPromptSubmit`/`PreToolUse`/`PostToolUse`/`SessionStart`/`SubagentStart` `HookSpecificOutputWire` types with an embedded JSON schema (`"const": "PermissionRequest"` etc.). **Confirmed** (allow/deny contract; `updatedPermissions`/`interrupt` rejected).
- Hook trust: `codex --help` and `codex exec --help` list `--dangerously-bypass-hook-trust` ("Run enabled hooks without requiring persisted hook trust for this invocation. DANGEROUS."). Binary: `trusted_hash`, "Failed to trust hooks:", "config/batchWrite failed while updating hook trust in TUI", "skipping materialized plugin hook trust after account changed". Trust-by-hash **confirmed**. The `/hooks` slash-command label itself was not located in strings: *not verified* that the command is spelled `/hooks` in this build.
- `project_doc_max_bytes`: binary contains the default literal `project_doc_max_bytes = 32768` and `project_doc_fallback_filenames = []`, plus `AGENTS.override.md` / `AGENTS.md` precedence strings (`core\src\agents_md.rs`, "respecting normal project-document precedence (`AGENTS.override.md`, `AGENTS.md`, then configured fallback filenames)"). Not set in the local `config.toml`, so the effective value is the 32 KiB default. **Confirmed**.
- `CODEX_HOME`: `--help` says `-p/--profile` layers `$CODEX_HOME/<name>.config.toml`; binary has "failed to resolve CODEX_HOME", "enable tracing in $CODEX_HOME/config.toml". **Confirmed**. New: "CODEX_HOME/managed_config.toml is no longer supported on Windows. Use %ProgramData%\OpenAI\Codex\requirements.toml for enforced settings" - the managed/MDM layer moved on Windows (relevant to the config service's managed-file guard).
- Hook timeouts (600 s default; 1 s/3 s for `SessionEnd`/`Interrupt`): only "hook timeout to " and a `hook.timeout_sec` tracing field found. **Not determinable locally**.
- `codex doctor --json` emits "a redacted machine-readable report" (help text) - a candidate for the Marshell doctor, not run here. `codex app-server generate-json-schema --out <DIR> [--experimental]` exists for the app-server protocol (not run).

### 3. Directory layout (`C:\Users\Home Office\.codex`, names only)

- Folders: `.sandbox`, `.sandbox-bin`, `.sandbox-secrets`, `.tmp` (`bundled-marketplaces`, `marketplaces`, `plugins`, `plugins.sha`, `rollout-compression.lock`, `rollout-maintenance.lock`), `ambient-suggestions`, `app-server-control` (`.sock`, startup lock), `app-server-daemon` (`daemon.pid`, `daemon.stderr.log`, updater pid/sock), `browser`, `cache`, `code-review-plugin`, `computer-use`, `node_repl`, `packages` (`app-server-daemon`, `standalone`), `pets`, `plugins` (`cache`, `.plugin-appserver`, staging), `project-metadata-locks`, `rules` (`default.rules`), `sessions`, `skills` (`.system`), `sqlite`, `thread-writer-locks` (`<thread-uuid>.lock`), `tmp`, `tui-thread-reference-capabilities` (`<thread-uuid>` marker files), `vendor_imports`, `visualizations`.
- Files: `.codex-global-state.json` (+ `.bak`), `auth.json`, `cap_sid`, `chrome-native-hosts-v2.json`, `config.toml`, `goals_1.sqlite`, `history.jsonl`, `installation_id`, `logs_2.sqlite` (+ `-shm`/`-wal`), `memories_1.sqlite`, `models_cache.json`, `queue_1.sqlite` (+ wal), `session_index.jsonl`, `state_5.sqlite` (+ wal), `thread_history_1.sqlite`, `version.json`.
- Sessions: `sessions\YYYY\MM\DD\rollout-YYYY-MM-DDTHH-MM-SS-<uuid>.jsonl` (6 files, 1-10 MB each). **Confirmed** layout pattern. Caveat: `codex migrate-rollouts --help` reads "Inspect or migrate legacy local sessions to paginated thread history", `thread_history_1.sqlite` exists, and the binary has `.zst` next to `rollout-`/`.jsonl` and a `rollout-compression.lock` - rollout JSONL is a *legacy* format being migrated into SQLite, possibly zstd-compressed. Treat transcript-file parsing as version-fragile.

### Verdict on `docs/research/codex-cli.md`

Confirmed: 12 event names incl. `PermissionRequest`; common payload fields and `permission_mode`; `PermissionRequest` allow/deny wire and rejection of `updatedPermissions`/`interrupt`; `commandWindows`; trust by hash and the bypass flag; `AGENTS.override.md` > `AGENTS.md` > fallback filenames; default `project_doc_max_bytes` 32768; `CODEX_HOME`; `sessions/YYYY/MM/DD/rollout-*.jsonl`. Not determinable locally: timeout defaults, exact `/hooks` UX, user-level `hooks.json` path, Windows quoting. New since the doc: `mcp_tool` handler type and `async`/`statusMessage`/`additionalContextLimit` fields; Windows managed config now `%ProgramData%\OpenAI\Codex\requirements.toml`; rollout-to-SQLite migration.

## Antigravity (`agy`)

### 4. Version, path, `--help`

- `agy --version` -> `1.3.3`; path `C:\Users\Home Office\AppData\Local\agy\bin\agy.exe`.
- `agy --help` flags: `--add-dir`, `--agent`, `-c`/`--continue`, `--conversation <id>`, `--dangerously-skip-permissions`, `--disable-slash-commands`, `--effort (low|medium|high|xhigh|max)`, `-i`/`--prompt-interactive`, `--input-format (text|stream-json)`, `--json-schema`, `--log-file`, `--mode (accept-edits|plan)`, `--model`, `--new-project`, `--output-format (text|json|stream-json)`, `-p`/`--print`/`--prompt`, `--print-timeout`, `--project`, `--remote-control`, `--sandbox`.
- Subcommands: `agent`/`agents`, `changelog`, `help`, `install` (`--dir`, `--skip-aliases`, `--skip-path`), `mcp` (`add|remove|list|enable|disable`), `mic-serve`, `models`, `plugin`/`plugins` (`list|import [gemini|claude]|install|uninstall|enable|disable|validate|link`), `remote-control` (`start|status|stop`), `update`.

### 5. Config files, keys, status line, hooks

Files present (names):
- `~\.gemini\`: `settings.json` (82 B), `projects.json`, `state.json`, `trustedFolders.json`, `google_accounts.json` (skipped), `oauth_creds.json` (skipped), `installation_id` (skipped), `history\`, `tmp\`, `antigravity-cli\`, `config\`.
- `~\.gemini\config\`: `.migrated` (0 B), `mcp_config.json` (0 B), `projects\default-cli-project.json` (keys `id`, `name`, `projectResources`). No `hooks.json` here yet.
- `~\.gemini\antigravity-cli\`: `settings.json`, `history.jsonl`, `cli.log`, `conversation_summaries.db`, `jetbox_summaries_proto.pb`, `jetski_state.pbtxt`, `installation_id`, `last_check.timestamp`; folders `annotations\<conv-uuid>.pbtxt`, `bin\` (`agentapi.bat`, `webm_encoder.exe`), `brain\<conv-uuid>\`, `builtin\skills\`, `cache\` (`marketplaces\antigravity-plugins-official\{marketplace,meta}.json`, `default_project_id.txt`, `last_conversations.json`, `onboarding.json`), `conversations\<conv-uuid>.db`, `crashes\`, `implicit\*.pb`, `log\cli-YYYYMMDD_HHMMSS.log`, `presence\<conv-uuid>.lock`, `updater\update_status.json`.

Keys of non-secret settings (values redacted):
- `~\.gemini\settings.json`: `security.auth.selectedType`.
- `~\.gemini\antigravity-cli\settings.json`: `colorScheme`, `dangerouslySkipPermissions` (currently `true` on this machine - with it on, a hook `ask` decision may never prompt; keep in mind for fixtures), `model`, `permissions.allow[]` (command allow-list strings, not quoted), `trustedWorkspaces[]` (includes the Marshell repo path). **No `statusLine` key and no hooks key** in this file.

Status line (binary evidence, `agy.exe`):
- Settings struct field `StatusLine json:"statusLine,omitempty"` and config field `StackWithDefault json:"stack_with_default,omitempty"`; types `StatusLineConfig`, `StatusLineRunner`, `StatusLineData`, `StatusLineModel`, `StatusLineQuotaBucket`, `StatusLineContext`, `StatusLineCost`, `StatusLineSandbox`, `StatusLineAgent`, `StatusLineSubagent`, `StatusLineVCS`, `StatusLineWorkspace`, `StatusLineVim`.
- Embedded changelog lines: "Added a `stack_with_default` flag to the `statusLine` configuration to render both the default Antigravity status line and custom status line output vertically stacked."; "Added a `conversation_title` field to the JSON payload passed to custom status line and window title scripts"; "Added a `cost` field to the status line data model"; "Added display of quota usage and execution mode in the status line."
- So a custom status-line script fed JSON (model, context, quota, cost, title) exists in 1.3.3. The exact `statusLine` sub-keys (`command`, etc.) and the full stdin schema were not isolatable from strings: *not verified*.

Hooks (bundled doc `~\.gemini\antigravity-cli\builtin\skills\agy-customizations\docs\hooks.md`, plus `SKILL.md`, `plugins.md`, `json_configs.md`):
- Events: `PreToolUse`, `PostToolUse` (grouped: `matcher` regex + `hooks[]`), `PreInvocation`, `PostInvocation`, `Stop` (flat handler lists). No `PermissionRequest`, `BeforeTool`, `AfterTool`, `Notification`, `UserPromptSubmit` hook events (binary matches for those names are only unrelated protos: `FilePermissionRequest` step types, MCP notifications). Binary has `exa.hooks_pb.SessionStartHookArgs` and the string "5 hook types" - `SessionStart` is undocumented for the CLI; *not verified*.
- File shape: `hooks.json` = `{ "<hook-name>": { "enabled": bool, "PreToolUse": [{ "matcher": "...", "hooks": [{ "type": "command", "command": "...", "timeout": 30 }] }], "Stop": [{ "type": "command", "command": "..." }] } }`. Handler fields `type` (only `"command"` documented), `command` (run via `cmd /c` on Windows, `~` expanded, cwd = directory containing `hooks.json`), `timeout` (seconds, default 30). Hooks run synchronously and block the loop. Matcher = regex on tool name (lowercased step type minus `CORTEX_STEP_TYPE_`, e.g. `run_command`, `view_file`).
- Locations: customization root `.agents/` (also `.agent/`, `_agents/`, `_agent/`) walking CWD -> repo root, and global `~/.gemini/config/` (`SKILL.md` "Global Configuration ... Path: `~/.gemini/config/`"). Changelog: "Fixed ... `/hooks` command wrote configurations to `~/.gemini/antigravity-cli/hooks.json` instead of the shared `~/.gemini/config/hooks.json`" and "workspace-local hooks defined in `<workspace>/.agents/hooks.json` not loading after trusting a folder". Plugins may ship `plugins/<name>/hooks.json`; disabled plugins' hooks are skipped (changelog).
- Payload: "All JSON keys in the hook payloads use camelCase (protojson encoding)". Common fields `conversationId`, `workspacePaths`, `transcriptPath`, `artifactDirectoryPath`, `modelName`. No event-name field - the command is not told which event fired.
- `PreToolUse` input adds `toolCall{name,args}`, `stepIdx`; output `decision` (`allow|deny|ask|force_ask`), `reason`, `permissionOverrides[]`, `overwrite{}` (shallow merge into tool args). Binary JSON-schema enum also lists `deny_unless_prior_grant` (undocumented; env `ANTIGRAVITY_PERM_GRANTS` nearby). `PostToolUse` input adds `stepIdx`, `error`; output `{}`. `PreInvocation`/`PostInvocation` input `invocationNum`, `initialNumSteps`; output `injectSteps[]`, `terminationBehavior` (`force_continue|terminate`). `Stop` input `executionNum`, `terminationReason` (`model_stop|max_steps_exceeded|error`), `error`, `fullyIdle`; output `decision: "continue"` + `reason` to block the stop (changelog: capped after N consecutive continuations).
- Binary also has "prompt hooks are not supported for the PostToolUse event; use a command hook instead", "prompt hook must specify 'prompt'" - a `prompt` hook type exists in code while the doc says command-only; treat `command` as the only safe type.
- Changelog: in print mode, `-p "/hooks"`, `/permissions`, `/help`, `/changelog`, `/config` answer without an agent turn (TSV, or structured under `--output-format json|stream-json`). Not run here (rule), but it is a read-only doctor probe candidate.

Transcripts (names only): per conversation `~\.gemini\antigravity-cli\brain\<conversationId>\.system_generated\logs\transcript.jsonl` and `transcript_full.jsonl`, with `chunks\transcript\0000000N.jsonl` and `chunks\transcript_full\0000000N.jsonl`; `.system_generated\steps\<n>\output.txt`; `scratch\`; `.user_uploaded\`. Content not read; schema *not verified*.

### 6. Contradictions with `docs/research/antigravity-notes/`

| Claim in notes | Local evidence | Verdict |
|---|---|---|
| README: notes say no status line (`statusline` capability false); vendor pages show one | `statusLine` settings field, `StatusLineConfig`/`StatusLineData` types, changelog lines above | **Contradicted** (vendor pages right) |
| 03: `PreToolUse` grants permission with `allow|deny|ask|force_ask`; camelCase `conversationId`, `stepIdx`, `toolCall` | Bundled `hooks.md` says exactly this; binary enum adds `deny_unless_prior_grant` | **Confirmed** |
| 02: `hooks.json` shape = named hooks with `enabled`, grouped `PreToolUse`/`PostToolUse`, flat `Stop` | `hooks.md` file-format section | **Confirmed** |
| 02: install at `~/.gemini/config/hooks.json` | `SKILL.md` global root + changelog fix naming that path | **Confirmed** |
| 01: hooks go in `~/.gemini/antigravity-cli/settings.json` or `.agents/hooks.json` | settings.json has no hooks key; docs put hooks only in `hooks.json` | **Contradicted** for the settings.json half; `.agents/hooks.json` confirmed |
| 01/04: transcript at `~/.gemini/antigravity-cli/transcript.jsonl` | Actual: `antigravity-cli\brain\<conversationId>\.system_generated\logs\transcript.jsonl` (+ `_full`, `chunks\`) | **Contradicted** (use `transcriptPath` from the payload) |
| 04: transcript JSONL fields (`step_index`, `source`, `truncated_fields`, ...) | Content not read | **Not determinable locally** |
| 04: `agy -p` headless with `--dangerously-skip-permissions` | Both flags in `agy --help`; also `--output-format json|stream-json`, `--print-timeout` | **Confirmed** |
| 03: `Stop` payload (`executionNum`, `terminationReason`, `fullyIdle`) and `{"decision":"continue"}` | `hooks.md` Stop contract | **Confirmed** |
| 01: Gemini CLI deprecated in favour of `agy` | Nothing local proves or disproves it; `agy plugin import gemini` exists | **Not determinable locally** |
| 05: brand colour | Out of scope here | Not checked |

Hook timeout default is 30 s (doc), not the 600 s the notes propose for `PreToolUse`; 600 must be set explicitly.

## What a live capture would need

Not attempted; it requires writing config. The isolated setup would be:

- **Codex**: set `CODEX_HOME` to a throwaway directory holding a minimal `config.toml` with `[[hooks.<Event>]]` entries for all 12 events, each `command` a tee script (PowerShell: read stdin, append to `<event>.jsonl`, print `{}`; for `PermissionRequest` print nothing so the normal prompt stays). Isolation also isolates `auth.json`, so log in inside that home. Trust the hooks in the TUI (or use `--dangerously-bypass-hook-trust` only in the throwaway home), then one prompt that runs a shell command needing approval and one `/compact`. Pair with `codex app-server generate-json-schema --out <dir>` from the same binary for the hook/app-server wire types, and `codex doctor --json` for the doctor format.
- **Antigravity**: `agy` has no documented home override; isolate with a separate Windows user profile (or an unverified `USERPROFILE`/`HOME` override). Put `.agents/hooks.json` in a throwaway workspace with one tee handler per event (`PreToolUse` matcher `*`, `PostToolUse`, `PreInvocation`, `PostInvocation`, `Stop`), trust the folder, run `agy -p "<prompt that runs one command>" --output-format stream-json` so stdout is also captured. For the status line, add a `statusLine` entry to that profile's `antigravity-cli\settings.json` whose command tees stdin, and run one interactive turn. Also run `agy -p "/hooks" --output-format json` to see how the CLI reports active hooks.

## Open items

- Codex hook timeout defaults and the `SessionEnd`/`Interrupt` caps - only a live run or the docs can confirm.
- Exact Codex `/hooks` trust UX wording and whether trust is per-file hash or per-handler hash.
- Codex user-level `hooks.json` path and precedence versus `config.toml` hooks on Windows; Windows quoting of `commandWindows` with spaces in the path.
- Codex rollout -> SQLite migration (`migrate-rollouts`, `thread_history_1.sqlite`, possible `.zst` compression): what the adapter should read for history, and from which version.
- Codex `mcp_tool` hook handler and `async` hooks: semantics and whether Marshell should use them.
- Antigravity `statusLine` sub-keys and the full status-line stdin schema; `window title` script hook mentioned in the changelog.
- Antigravity `deny_unless_prior_grant` decision and `ANTIGRAVITY_PERM_GRANTS`; `SessionStart` hook args in the proto; `prompt` hook type.
- Antigravity transcript JSONL schema and `truncated_fields`; `conversations\<id>.db` vs `brain\<id>\...\transcript.jsonl` as the durable source.
- Whether `agy` honours any home-directory override for an isolated capture.
- `docs/research/antigravity-notes/01-overview.md` claim that Gemini CLI is deprecated.
