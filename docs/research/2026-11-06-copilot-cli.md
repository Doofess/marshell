# GitHub Copilot CLI 1.0.106: hooks, sessions and surfaces

Research for the Copilot CLI slices (PLAN §9, "Slice 6n1 decisions (2026-11-06)"). Two read-only passes on 2026-11-06 over Copilot CLI 1.0.106 as installed by `winget install GitHub.Copilot`: one for the adapter (6n1), one for the surfaces and the Doctor (6n2, 6n3). 6n1's spike ran a real Copilot against a fake model on 2026-11-06. Its findings open Part 1, and where they differ from the read-only passes below, they win.

Where the source is:
- The winget package puts a Node single-executable `copilot.exe` (184 MB) in `%LOCALAPPDATA%\GitHubCopilotCLI\`, beside `copilot-taskbar-activator.exe` and `GitHubCopilotCLI.sparse.msix`. Its `sea-loader.js` unpacks an embedded `copilot.tgz` on first run into the first writable of `COPILOT_PKG_CACHE_HOME`, `COPILOT_CACHE_HOME`, the platform cache (`%LOCALAPPDATA%\copilot\pkg` here), `COPILOT_HOME` and `~/.copilot/pkg`.
- The readable bundle is therefore `%LOCALAPPDATA%\copilot\pkg\win32-x64\1.0.106\`: `cli-main.js` (minified, 9 MB), `schemas/session-events.schema.json`, `schemas/api.schema.json`, `changelog.json`, `builtin-skills/`, `definitions/`, `plugins/`.
- Hook execution lives in native code (`hookSessionCreate` in the `.node` runtime, which gets `userHooksDir: <config dir>/hooks` and the user settings JSON), so some hook details below are read from strings and docs, not code.
- The npm package `@github/copilot` is only a loader; per-platform packages carry the binary. Copilot's licence bars bundling it (`2026-10-25-agent-and-tool-cli-manifests.md`).

Corrections made by the main thread after the passes:
- **PATH.** The user PATH in the registry holds `%LOCALAPPDATA%\GitHubCopilotCLI\` after VS Code's `copilot-shim` folder. The adapter pass saw a session environment from before the install, so it reported the folder as missing from PATH.
- **The shim forwards.** A bare `copilot --version` through VS Code's shim printed "GitHub Copilot CLI 1.0.106." on this PC, so the shim runs the real CLI once it's installed. Before the install it printed "not found" (`2026-10-25-agent-and-tool-cli-manifests.md`, finding 8).
- **Spot checks.** `userHooksDir: <config dir>/hooks` and the help text for `COPILOT_PROVIDER_BASE_URL` ("GitHub authentication is not required") were confirmed in `cli-main.js`. The `permissionRequest` reply shape was not found in the JS and stays for the spike.

Evidence tags: [help] CLI help output; [src] a string or code in 1.0.106; [docs] GitHub docs (main branch, may be newer than 1.0.106); [chg] a changelog entry.

## Part 1: the adapter (6n1)

### What 6n1's spike saw (2026-11-06)

Method:
- Copilot CLI 1.0.106 (winget, `%LOCALAPPDATA%\GitHubCopilotCLI\copilot.exe`) ran against `fake-openai.mjs` through BYOK, with `COPILOT_HOME`, `COPILOT_CACHE_HOME`, `USERPROFILE` and `HOME` in scratch folders.
- Headless runs a1 to a36 (`-p`) used `C:\dev\scratch-6n1-spike-1`. The interactive runs used two PowerShell tabs in the dev app (`live-check.ps1 start -CopilotFake`, scratch `C:\dev\scratch-6n1-lc-1`). Text went in through the scratch core's input API, and Enter, arrows and Esc as CDP keys into the tab's terminal.
- A recorder, `rec.mjs`, run by `node.exe` from a folder with a space in its name, saved each hook's stdin, its chosen variables, the names of the rest, and its cwd. A second recorder was the status line.
- Afterwards the real `~/.copilot` (10 entries) and `~/.claude/settings.json` had the same modified times as before.
- Fixtures: `crates/core/tests/fixtures/copilot/hooks/1.0.106/` and `statusline/1.0.106/`, scrubbed.

The hooks file that ran, `<COPILOT_HOME>\hooks\rec.json`, had one entry per event like this one: `timeoutSec` 5, 2 for `sessionEnd` and 600 for `permissionRequest`, and no `matcher`:

```json
{"version": 1, "hooks": {"sessionStart": [{"type": "command", "exec": "C:/Program Files/nodejs/node.exe", "args": ["C:/dev/scratch-6n1-spike-1/rec dir/rec.mjs", "sessionStart"], "timeoutSec": 5}]}}
```

The events were `sessionStart`, `userPromptSubmitted`, `preToolUse`, `postToolUse`, `postToolUseFailure`, `notification`, `agentStop`, `errorOccurred`, `sessionEnd` and `permissionRequest`. The `settings.json` that ran:

```json
{"autoUpdate": false, "taskbarPresence": false, "statusLine": {"type": "command", "command": "node \"C:/dev/scratch-6n1-lc-1/rec dir/sl.mjs\""}}
```

- **Q1. Loading.** A `*.json` file in `<COPILOT_HOME>\hooks\` holding `{"version":1,"hooks":{...}}` loads, headless and interactive, in a trusted and an untrusted folder. Hooks run with the project folder as their cwd.
- **Q2. Entry form.** `exec` + `args` runs the program with no shell, from a folder with a space. A `powershell` entry for the same event started about 600 ms later (631 ms and 576 ms).
- **Q3. Bodies.**
  - Every body has `sessionId`, an epoch-ms `timestamp` and `cwd`.
  - Only `notification` names its event: `"hook_event_name": "Notification"`, with `notification_type` (`permission_prompt`), `title` ("Permission needed") and `message` ("Run command: New-Item ...", "Edit file: <path>"). `permissionRequest` has `"hookName": "permissionRequest"` instead.
  - `sessionStart` has `source` (`new` or `resume`) and `initialPrompt`. In an interactive session it fires at the first prompt, not at launch.
  - `userPromptSubmitted` has `prompt`.
  - `preToolUse` has `toolCallId` (`call_<n>`), `toolName` and `toolArgs`, which is an object. The Windows tool names are:
    - `powershell` `{command, description}`;
    - `create` `{path, file_text}`;
    - `edit` `{path, old_str, new_str}`;
    - `view` `{path}`.
  - `postToolUse` adds `toolResult {resultType: "success", textResultForLlm}`. A shell command that fails is still a `postToolUse`; its exit code is in the text. `postToolUseFailure` (a `view` of a missing file) adds `error`.
  - `permissionRequest` has `toolName`, `toolInput` and `permissionSuggestions: []`. Its `toolName` is the kind of permission, not the tool: `powershell` with `{command}`, or `edit` with `{file_path, diff}` for a `create`. It has no tool call id. `requestSandboxBypass` never appeared.
  - `agentStop` has `transcriptPath` (`<COPILOT_HOME>\session-state\<id>\events.jsonl`), `stopReason: "end_turn"` and `stop_hook_active: false`.
  - `sessionEnd` has `reason`: `complete` in `-p`, and `user_exit` for `/exit` and `/clear`.
  - Every hook process gets these variables:
    - `COPILOT_HOOK_DIR` (`<COPILOT_HOME>\hooks`);
    - `COPILOT_PROJECT_DIR` and `CLAUDE_PROJECT_DIR`, both the project;
    - `COPILOT_HOOK_DEPTH=1` and `COPILOT_CLI=1`;
    - `COPILOT_HOME`, `MARSHELL_TAB_ID` and `MARSHELL_HOME`, inherited.

    Two tabs reported their own tab ids.
- **Q4. When `permissionRequest` fires.** It fires before Copilot's own rules, for every shell call and file write, whether Copilot then asks or not:
  - Commands Copilot ran by itself after an empty answer still fired it: `echo`, `git status`, `git log -1`, `Get-ChildItem`, `Get-Content` and `Select-String`. `dir`, `npm --version` and `New-Item` fired it and then asked.
  - It fired after "Yes, and don't ask again for file operations when in this directory".
  - It fired under `--allow-tool=shell(New-Item)`, `--allow-all-tools`, `COPILOT_ALLOW_ALL=true`, `/allow-all on`, `"defaultPermissionMode": "allow-all"`, `--autopilot`, and an `echo` in plan mode.
  - It didn't fire for a `view` inside the folder. Plan mode refuses a write itself, before any hook.

  The body is the same in every case, so a hook can't tell which requests Copilot would ask about. This led to 6n1 decision 12.
- **Q5. Answers to `permissionRequest`.**
  - `{"behavior":"allow"}` runs the call.
  - `{"behavior":"deny","message":"..."}` refuses it with "Permission denied by a PermissionRequest hook: <message>", and the model sees the message.
  - Empty stdout goes on to Copilot's own flow: its dialog, or "Denied, no one to approve" in `-p`.
  - Exit 1 refuses with "Permission request hook failed".
  - A timeout is no answer: Copilot's own flow.
- **Q6. Neutral answers and failures.**
  - Empty stdout with exit 0 changes nothing, for every event.
  - A `preToolUse` that exits 1 refuses the call: 'Denied by preToolUse hook from "<file>" (hook errored)'.
  - A missing exe refuses the same way, for `preToolUse` and `permissionRequest`, even under `--allow-all-tools`. A missing exe on `postToolUse`, `agentStop` or `userPromptSubmitted` does no harm. This led to 6n1 decision 13.
- **Q7. Context.** `{"additionalContext":"<note>"}` from `userPromptSubmitted` reaches the model, appended to the user message as `<system_reminder>` `<note>` `</system_reminder>`. Autopilot's continuations also fire `userPromptSubmitted`, with Copilot's own text ("You have not yet marked the task as complete..."), so the note goes with each of them too.
- **Q8. Status line.**
  - The command runs through `cmd`: `%OS%` is expanded, `$HOME` is not, `"a b"` arrives as one argument, and `^&` arrives as `&`.
  - It runs at start, around each tool, after the turn and after a mode change. `settings.json` is read at launch only.
  - It gets `MARSHELL_TAB_ID` and `COPILOT_HOME`, but no `COPILOT_HOOK_DIR`.
  - Its stdin (`statusline/1.0.106/idle.json`) has these keys:
    - `cwd`, `session_id`, `session_name`, and `transcript_path`, which is the session folder, not `events.jsonl`;
    - `model {id, display_name, auto_tier, pending_auto_tier}`, `workspace.current_dir`, `username`, `remote.connected` and `version`;
    - `cost {total_api_duration_ms, total_lines_added, total_lines_removed, total_duration_ms, total_premium_requests}`;
    - `context_window {..., context_window_size, used_percentage, remaining_percentage, current_context_used_percentage, displayed_context_limit}`;
    - `ai_used`, and `allow_all_enabled`, which `/allow-all on` and the settings default turn true.
  - After `/clear` it reports the new session id at once.
- **Q9. Sessions.**
  - `--session-id <uuid>` names the new session, headless and interactive: the events, the status line and `session-state\<uuid>\` all use it.
  - Exit prints `Resume copilot --resume=<id>`.
  - `--resume=<id>` reopens the session in a new process. Its `sessionStart` says `resume`, at the first prompt. An allow-all turned on with `/allow-all on` comes back with it.
  - `/clear` sends `sessionEnd` (`user_exit`) for the old id at once, and a status line with a new id follows 0.2 s later. The new id's `sessionStart`, with `source: "new"`, waits for the next prompt.
  - No status line runs after `/exit`'s `sessionEnd`. A resumed Copilot runs one at launch, with the resumed id, before its `sessionStart`.
  - `--continue` from a folder with no session resumes the most recent session from any folder, in the new cwd.
  - `--trust-folder` can't be combined with `--session-id`.
- **Q10. Version.** "GitHub Copilot CLI 1.0.106." then "Run 'copilot update' to check for updates."
- **Q11. Processes.**
  - Launched by its full path, Copilot runs as one `copilot.exe` under the tab's shell.
  - A bare `copilot` here runs VS Code's shim (`...\Microsoft VS Code\bin\copilot-shim\copilot.exe`), which starts the real `copilot.exe` as its child.
  - Marshell's foreground reader reports `program: "copilot", agent: "copilot"` for both. On this PC the session PATH holds only the shim.
  - The taskbar relaunch wasn't exercised.
- **Q12. The fake model.**
  - The minimal set works with no sign-in prompt: `COPILOT_PROVIDER_BASE_URL`, `COPILOT_PROVIDER_TYPE=openai`, `COPILOT_PROVIDER_WIRE_API=completions`, `COPILOT_MODEL` and `COPILOT_OFFLINE=true`.
  - Copilot calls only `POST /v1/chat/completions`, streamed.
  - A first interactive run asks for folder trust ("1. Yes / 2. Yes, and remember this folder for future sessions / 3. No"). It also warns that the model isn't in the built-in catalog.
  - In a tab that inherited `WT_SESSION`, it offered "Set up terminal for multi-line input support" for Windows Terminal (default No). This led to 6n1 decision 14.
- **Q13. Skip-all settings.** `"defaultPermissionMode": "allow-all"` makes a launch with no mode flag "Allow All", and the status line's `allow_all_enabled` true. It has no effect in `-p`. `COPILOT_ALLOW_ALL=true` approves in `-p`. Neither stops `permissionRequest` from firing.
- **Q14. Claude Code's hooks.**
  - Copilot never ran a hook from the scratch `~/.claude\settings.json` or `CLAUDE_CONFIG_DIR`.
  - It runs a trusted project's `.claude\settings.json` hooks interactively, but not in `-p`, even trusted.
  - It rejects an empty `matcher` on `PreToolUse` and `PermissionRequest` ("matcher cannot be empty") and then ignores the whole file.
  - Those hook processes get `COPILOT_CLI=1`, `COPILOT_PROJECT_DIR` and `MARSHELL_TAB_ID`, but no `COPILOT_HOOK_DIR`. Their cwd is the project's parent folder. `PreToolUse` arrives in Claude's shape (`tool_name: "Bash"`), `PermissionRequest` in Copilot's camelCase shape.

Also seen:
- Esc in Copilot's own dialog prints "Operation cancelled by user" and fires no hook (no `postToolUseFailure`, no `agentStop`); only the status line runs.
- Trust chosen with "remember" goes to `config.json`'s `trustedFolders`.

---

Method: read-only. `copilot --version` printed "GitHub Copilot CLI 1.0.106."; `--help`, `help <topic>` were run on the real exe
(`C:\Users\Home Office\AppData\Local\GitHubCopilotCLI\copilot.exe`). Source evidence comes from strings in the shipped JS bundle
(`cli-main.js`), the native `runtime.node` / `cli-native.node`, the SDK `types.d.ts`, `schemas\session-events.schema.json`, the
changelog, and github/docs (`content/copilot/reference/hooks-reference.md`, settings and use-hooks docs). Docs are on main and may be
newer than 1.0.106. No session was started, nothing was written to `~/.copilot`.

Evidence tags: [help] = CLI help output; [src] = grepped string/code in 1.0.106; [docs] = GitHub docs; [chg] = changelog entry (version given).

---

### Q1. Hooks

#### Locations and merge order
Loaded in order policy, user, project, plugins. All entries from all sources run (they are merged, not overridden) [docs hooks-reference].
- Policy: `C:\ProgramData\GitHub\Copilot\policy.d\*.json` (alphabetical), plus Registry `HKLM\Software\Policies\GitHub\Copilot` (a subkey with a `Policy` REG_SZ) [docs]. Policy hooks ignore `disableAllHooks` and are available regardless of folder trust.
- Repo: `.github/hooks/*.json` [docs].
- User: `%USERPROFILE%\.copilot\hooks\*.json`, or `$COPILOT_HOME/hooks/` [docs].
- Inline `hooks` key in repo `.github/copilot/settings.json` and `settings.local.json`; also `.claude/settings.json` and `.claude/settings.local.json` in the repo [docs]. Inline `hooks` key in `~/.copilot/settings.json`. The CLI no longer reads `config.json` for hooks [docs].
- Plugin `hooks.json` or `hooks/hooks.json` [docs]. Agent-scoped hooks in a custom agent's frontmatter [chg 1.0.97].

Implication for Marshell: a user file in `~/.copilot/hooks/` or an inline `hooks` key in `~/.copilot/settings.json` are both user-level targets. The CLI also reads `.claude/settings*.json` hooks in a repo, so a Claude hook written there would run in Copilot too (double-fire risk if Marshell writes the same bridge in both).

#### Trust and workspace admission
- [chg 1.0.8] "Repo-level hooks are loaded only after folder trust is confirmed". [src] strings: "agent hooks deferred until their workspace is trusted"; "Skipping repo/workspace hooks: blocked by enterprise customization lockdown (strictPluginOnlyCustomization locks "hooks")".
- [docs] "Repository and inherited-agent hooks require workspace admission."
- User hooks: docs imply no trust needed; [chg 1.0.106] "Repository disableAllHooks continues suppressing user hooks after trusted-workspace rebuilds" suggests user hooks load independent of trust. Not tested: UNVERIFIED.
- `-p` mode: [chg 1.0.40] env opt-ins `GITHUB_COPILOT_PROMPT_MODE_REPO_HOOKS` and `GITHUB_COPILOT_PROMPT_MODE_WORKSPACE_MCP`; [chg 1.0.49] repo hooks load in `-p` when the folder is already trusted. `--trust-folder` trusts for one non-interactive session. `COPILOT_ALLOW_ALL` exactly "true" also trusts the cwd and loads its hooks [help environment].

#### Off switches
- `disableAllHooks` (top level of settings or inside a hooks file); a repo-level one skips every non-policy hook for sessions in that repo [docs, chg 1.0.106].
- `disabledHooks`: content-hash keys; policy hooks cannot be suppressed [docs].
- Managed `allowManagedHooksOnly`, `strictPluginOnlyCustomization` [docs, src].

#### File format
```
{ "version": 1, "hooks": { "<event>": [ entries ] } }
```
`version` may be omitted [chg 1.0.5]. Entry fields [docs]: `type` ("command"|"http"|"prompt", default command), `bash`, `powershell`, `command` (cross-platform fallback, copied to both when absent [chg 1.0.2]), `exec` + `args` (direct exec, no shell; not combinable with the others), `cwd`, `env` (variable expansion), `timeoutSec` (default 30, alias `timeout`), `matcher` (regex, anchored `^(?:PATTERN)$`).
- Claude-style nested `{matcher, hooks:[...]}` groups accepted [chg 1.0.6, 1.0.66].
- No explicit `cwd`: hook runs in the project root [chg 1.0.88]; lifecycle hooks run in the session dir after /cd [chg 1.0.72].
- HTTP hooks: JSON POST, https only (localhost with `COPILOT_HOOK_ALLOW_LOCALHOST=1`); `preToolUse` and `permissionRequest` require https [docs].
- Prompt hooks (`type:"prompt"`): only `sessionStart`, only new interactive sessions (not resume, not `-p`) [docs].
- A malformed entry drops only that item for directory-loaded files; inline settings hooks are strict [docs]. UTF-8 BOM accepted [chg 1.0.106].
- Stdout handling: a line `{"type":"progress","message":..,"temporary":bool}` is consumed; the rest is trimmed and parsed with one `JSON.parse`; empty or invalid means no output and default behaviour [src]. Stdout cap 10 MiB.

#### Windows shell
- Docs: `powershell` field for Windows, `bash` for Unix; Windows examples need PowerShell 7+ in PATH [docs use-hooks]. [src] error string "Neither 'bash' nor 'powershell' specified in hook command configuration".
- Settings doc: CLI prefers `pwsh`, falls back to `powershell.exe` [chg 1.0.45]; `powershellFlags` default `["-NoProfile","-NoLogo"]`.
- [src] runtime modules `hook_invocation_runtime\shell_selection.rs` and `power_shell_resolver.rs`; string "hook interpreter aborted during its own startup before running the script; retrying once". [chg 1.0.65] prevents console flashing.
- UNVERIFIED: the exact argv of the PowerShell invocation, and what a `bash`-only entry does on Windows. Safest for Marshell: write `powershell` (and `command`), or use `exec` + `args` to bypass any shell.

#### Env vars given to hook processes
[src, runtime `hook_invocation_runtime` strings]: `CLAUDE_PROJECT_DIR`, `COPILOT_PROJECT_DIR`, `COPILOT_HOOK_DIR`, `COPILOT_HOOK_DEPTH` ([chg 1.0.106] "Command hooks receive COPILOT_HOOK_DEPTH so hook-launched sessions can stop recursive agentStop chains"), `COPILOT_TRACEPARENT`, `COPILOT_TRACESTATE`. Plugin hooks also `PLUGIN_ROOT`, `COPILOT_PLUGIN_ROOT`, `CLAUDE_PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA`, `COPILOT_PLUGIN_DATA`. `COPILOT_CLI=1` is set for subprocesses [chg 0.0.421].
The same string cluster lists `GITHUB_COPILOT_GITHUB_TOKEN`, `GITHUB_TOKEN`, `COPILOT_AGENT_CALLBACK_URL`, `GITHUB_COPILOT_API_TOKEN`, `POWERSHELL_UPDATECHECK`, `GIT_CONFIG_*`. Whether these are stripped or set: UNVERIFIED. Note `CLAUDE_PROJECT_DIR` is also set, so a bridge cannot tell the CLI apart by that variable; use `COPILOT_PROJECT_DIR`/`COPILOT_HOOK_DIR` or a bridge argument (for example `--cli copilot`).

#### Event list
[src runtime string] "sessionStart sessionEnd userPromptSubmitted userPromptTransformed preToolUse preMcpToolCall postToolUse postToolUseFailure errorOccurred agentStop subagentStart subagentStop preCompact permissionRequest notification" = 15 events.
- `preMcpToolCall` [chg 1.0.51] is not in the docs table: input `serverName, toolName, arguments, toolCallId, _meta`; output `metaToUse`. Earlier notes said 14 events.
- PascalCase names in the runtime: `PreToolUse PreMcpToolCall PostToolUse PostToolUseFailure UserPromptSubmitted UserPromptTransformed SessionStart SessionEnd ErrorOccurred AgentStop SubagentStart SubagentStop`. Docs say `Stop` aliases `agentStop` and use `UserPromptSubmit` in examples; the runtime string is `UserPromptSubmitted`. Which spellings are accepted: UNVERIFIED. `PermissionRequest`, `Notification`, `PreCompact` have no PascalCase entry in that list but the docs payload for notification carries `hook_event_name:"Notification"`.
- Naming mode changes payload shape: PascalCase gives snake_case payloads with `hook_event_name`, `session_id`, ISO 8601 `timestamp` [chg 1.0.21], Claude tool names (`Bash`, `Read`, `Write`, `Edit`) and Claude matcher semantics. camelCase gives camelCase fields and a numeric epoch-ms `timestamp` [docs].
- Marshell consequence: PascalCase gives a Claude-compatible payload, so the Claude bridge parser can be reused. Registering the camelCase names gives different keys and Copilot's own tool names.

---

### Q2. Payload per event [docs hooks-reference; camelCase shown, PascalCase is snake_case]
Common: `sessionId`, `timestamp`, `cwd`; subagent hooks add `agentId`, `agentType`, `parentAgentId`; OTel adds `traceparent`/`tracestate`.
- sessionStart: `source` ("startup"|"resume"|"new"), `initialPrompt?`.
- sessionEnd: `reason` ("complete"|"error"|"abort"|"timeout"|"user_exit").
- userPromptSubmitted: `prompt`. userPromptTransformed: `prompt`, `transformedPrompt`.
- preToolUse: `toolName`, `toolArgs`, `toolCallId?` [SDK types.d.ts]. PascalCase: `tool_name`, `tool_input` (parsed JSON).
- postToolUse: `toolResult{resultType:"success", textResultForLlm}`. postToolUseFailure: `error` string.
- agentStop: `transcriptPath`, `stopReason:"end_turn"`, `stop_hook_active`.
- subagentStart: `transcriptPath`, `agentName`, `agentDisplayName?`, `agentDescription?`. subagentStop: `agentId`, `agentType`, `agentName`, `response`, `stopReason`.
- errorOccurred: `error{message,name,stack?}`, `errorContext` ("model_call"|"tool_execution"|"system"|"user_input"), `recoverable`.
- preCompact: `transcriptPath`, `trigger` ("manual"|"auto"), `customInstructions`.
- notification: `hook_event_name:"Notification"`, `message`, `title?`, `notification_type`.
- permissionRequest: input NOT documented. [src runtime strings] `hookName`, `toolName`, `toolInput`, `permissionSuggestions`, and `requestSandboxBypass` inside `toolInput`. Exact shape UNVERIFIED.
- Session correlation: `sessionId` / `session_id` is in every payload, a UUID, the same as the `session-state` folder name (Q6).
- `transcriptPath` target: probably the session's `events.jsonl`; UNVERIFIED.

---

### Q3. Decisions and failure semantics

#### preToolUse output [docs, chg]
`permissionDecision` allow|deny|ask, `permissionDecisionReason` (required for deny), `modifiedArgs` (alias `updatedInput`; null = unchanged), `additionalContext`, `suppressOutput`.
- allow suppresses the approval prompt [chg 1.0.18]. ask shows the hook-permission prompt (user may add feedback); in autopilot, ask is denied "Denied by preToolUse hook (unable to ask user for confirmation)" [chg 1.0.97].
- Exit 2 denies even if stdout says allow. Crash or any other non-zero exit denies ("Denied by preToolUse hook (hook errored)"). **Timeout is fail-open**: proceeds through the normal permission flow with a warning [chg 1.0.67; src "timed out; allowing processing to proceed"]. HTTP hooks fail open. Empty/invalid stdout = default. Policy hooks that fail are "execution failed (fail-closed)".
- Any deny among multiple hooks blocks the call.
- This differs from Claude Code, where only exit 2 blocks and other non-zero exits are non-blocking. A Marshell bridge that crashes or cannot reach the app must exit 0 with empty output, or Copilot denies every tool call.

#### permissionRequest [docs; chg 1.0.16]
- Fires BEFORE the permission service (rules, session approvals, auto allow/deny, user prompt) for every request except `read` and `hook` kinds.
- Output: `behavior` allow|deny, `message`, `interrupt` (bool; with deny stops the agent). Exit 2 = deny. Empty or `{}` = fall through. Multiple hooks' outputs merged, later overriding earlier.
- Sandbox exception: allow does not pre-approve a `requestSandboxBypass:true` request; only deny propagates.
- Session event `permission.requested` has `resolvedByHook`.
- No output field to persist an always-allow rule (grepped runtime for `updatedPermissions`/`alwaysAllow`: no match), unlike Claude's `updatedPermissions`. The input has `permissionSuggestions`, which is not usable on the way back.
- Whether it fires under `--allow-all` / autopilot: UNVERIFIED.
- Timeout behaviour for permissionRequest specifically: UNVERIFIED (preToolUse is fail-open on timeout).

#### Context injection
- sessionStart `additionalContext` is injected into the conversation [chg 1.0.11]; multiple outputs joined by "\n\n". `modifiedConfig` is SDK only.
- subagentStart `additionalContext` is prepended to the subagent prompt. postToolUse `additionalContext` is appended to the result [chg 1.0.49/1.0.51]. preToolUse `additionalContext` per docs.
- notification `additionalContext` is injected as a user message and can wake an idle session.
- userPromptSubmitted: CONFLICT. Docs: command and HTTP hook output is dropped, including `modifiedPrompt`; only SDK hooks honour it. [chg 1.0.65] "Include userPromptSubmitted hook additionalContext in the model-facing prompt"; [chg 1.0.44] hooks can "handle requests directly" (`handled`, `responseContent`). SDK: `decision:"block"` with `reason` refuses the prompt. Whether a command hook's `additionalContext` reaches the model here: UNVERIFIED.
- userPromptTransformed `modifiedTransformedPrompt` rewrites the model-facing content [docs].

#### Notification types [docs]
`shell_completed`, `shell_detached_completed`, `agent_completed`, `agent_idle`, `permission_prompt`, `elicitation_dialog`. Fire-and-forget, async, never block; matcher on `notification_type`.
- `permission_prompt` only fires when a prompt is actually shown [chg 1.0.26; added 1.0.18]. Note: a hook-resolved permission request shows no prompt, so no notification.
- `agent_idle` is a BACKGROUND agent entering idle, not the main turn ending.

#### Stop vs sessionEnd
- `agentStop`: main agent finished a turn (also via `task_complete` [chg 1.0.45]). May return `{decision:"block", reason}` to force another turn; after 8 consecutive blocks the CLI ends the turn; `stop_hook_active` true on a forced continuation. This is the per-turn "done" signal (equivalent of Claude's Stop).
- `sessionEnd`: session terminates (`user_exit`, `complete`, `error`, `abort`, `timeout`); once per session interactively [chg 1.0.22]; also on `/clear` with `user_exit`, detached [chg 1.0.85]. In `-p`/piped runs fires once after completed turns with `complete` [chg 1.0.78/1.0.92].
- Other outputs [SDK]: errorOccurred `errorHandling` retry|skip|abort, `retryCount`, `userNotification`; sessionEnd `cleanupActions`, `sessionSummary`.
- There is no "turn started" hook except `userPromptSubmitted`; a working/idle status can be derived from `userPromptSubmitted` (working), `preToolUse`/`postToolUse`, `agentStop` (idle), `permissionRequest`/`notification permission_prompt` (needs approval), `sessionEnd`.
- `events.jsonl` also records `hook.start`/`hook.end` with `hookInvocationId`, `hookType`, `input`/`output`, `success`, `error` [src schema].

---

### Q4. Copilot's own permission flow, modes and flags
- Default mode "manual": write and command requests prompted; read-only auto-approved [docs `defaultPermissionMode`]. File access limited to cwd, subdirs and system temp unless `--allow-all-paths`.
- Prompt answers: `y` once, `n` deny once, `!` allow all similar for the session, `#` deny all similar for the session, `?` details [docs]. Dialog scopes: Once, "This location" (saved to disk per location), "Always" (persisted to "Config file"; key/file UNVERIFIED).
- Session approvals in memory; `/reset-allowed-tools` clears them.
- "This location" approvals: `~/.copilot/permissions-config.json` = `{ "locations": { "<git root or cwd>": { "tool_approvals": [ {kind: commands|read|write|mcp|mcp-sampling|memory|custom-tool|extension-management|extension-permission-access, ...} ], "allowed_directories": [...] } } }`. Resolution: `--config-dir`, then `COPILOT_HOME`, then `~/.copilot`. A legacy extensionless `permissions-config` exists. No deny/ask rules in this file. URL rules: `settings.json` `allowedUrls`/`deniedUrls`. Trusted folders: `config.json` `trustedFolders`. [src]
- Flags [help 1.0.106]: `--mode <interactive|plan|autopilot>`, `--plan`, `--autopilot`, `--max-autopilot-continues <count>` (default 5), `--allow-all-tools`, `--allow-all-paths`, `--allow-all-urls`, `--allow-all` (= `--yolo`), `--allow-tool [<tools>...]`, `--deny-tool [<tools>...]`, `--available-tools`, `--excluded-tools`, `--allow-url`, `--deny-url`, `--disallow-temp-dir`, `--assisted-approval` (env `COPILOT_ASSISTED_APPROVAL`).
- Pattern kinds: `shell(cmd:*)`, `write(path)`, `read(path)`, `memory`, `<mcp-server>(tool)`, `url(...)`. Denials always win, even over `--allow-all-tools` [help permissions].
- Env `COPILOT_ALLOW_ALL`: truthy auto-approves; exactly "true" also trusts the cwd. Settings: `defaultMode`, `defaultPermissionMode` (manual|assisted|allow-all), `stayInAutopilot`. Managed: `permissions.disableBypassPermissionsMode`. Event `session.permissions_changed` carries `mode` manual|assisted|allow-all.
- Non-interactive mode denies tool requests no allow rule approves. Shift+Tab cycles standard, plan, autopilot.
- Consequence: Marshell's approve card can answer via the `permissionRequest` hook (allow/deny once). There is no way to write back "always" through the hook; "always" would have to be Marshell's own memory (it then answers allow itself next time) or a `--allow-tool` flag at launch.

---

### Q5. Status line
- Setting `statusLine` in `~/.copilot/settings.json` (also repo settings): `{ type?: "command", command: string, padding?: n, refreshInterval?: 1..2147483 }`. `command` has `~` and env vars expanded; plain shell commands allowed [chg 1.0.52]. `padding` = left spaces per line; `refreshInterval` in seconds, omit for event-driven only. [src `cli-main.js` functions `Evo`/`wvo`/`pcr`]
- NOT experimental-gated in 1.0.106: enabled when `footer.showCustom !== false` (default true) [src `isEnabled:Kn.footer?.showCustom!==!1`]. Also toggled via `/statusline`, `/footer`. An older project note said "experimental"; the source does not bear that out.
- Spawn [src]: `child_process.spawn(command, [], {stdio:["pipe","pipe","pipe"], shell:true on win32, cwd: session cwd})`; on Windows `shell:true` = `cmd.exe /c`. Timeout 10,000 ms (SIGKILL).
- Non-zero exit, spawn failure or timeout: blank status line plus one warning per failure episode (`--log-level all` for detail).
- stdout trimmed, split on newlines into multiple lines; padding applied per line. ANSI rendering: UNVERIFIED.
- Debounce about 200/500 ms; event-driven refresh on session-state changes; waits for folder trust before running Git checks [chg 1.0.106].
- Stdin: `JSON.stringify(statusObject)`. Key names found in `cli-native.node` `status_line_snapshot.rs` (nesting UNVERIFIED, both snake_case and camelCase appear): `snapshotTimeMs`, `modelContext`, `remoteSessionDisplay`, `remoteContext`, `task_id`, `task_name`, `task_url`, `task_type`, `pull_request_number`, `total_input_tokens`, `total_output_tokens`, `total_cache_read_tokens`, `total_cache_write_tokens`, `total_tokens`, `context_window_size`, `used_percentage`, `remaining_percentage`, `remaining_tokens`, `last_call_input_tokens`, `last_call_output_tokens`, `total_reasoning_tokens`, `current_context_tokens`, `displayed_context_limit`, `current_context_used_percentage`, `current_usage{cache_creation_input_tokens, cache_read_input_tokens,...}`, `currentWorkingDirectory`, `sessionTitle`, `selectedModelDisplay`, `auto_tier`, `pending_auto_tier`, `username`, `total_api_duration_ms`, `total_lines_added`, `total_lines_removed`, `total_duration_ms`, `total_premium_requests`, `formattedAiUsed`, `allow_all_enabled`. Capture input also has `sessionId`, `sessionStatePath`, `version`, `selectedModel`, `availableModels`, `contextTier`.
- Whether stdin has `session_id`, `model.id`, `transcript_path` as in Claude: UNVERIFIED. `sessionId` and `sessionStatePath` are in the capture input, which would give Marshell a session id for chaining.
- Chaining: Marshell can wrap the user's `command` (as for Claude), keeping in mind `cmd.exe /c` quoting on Windows and the 10 s timeout.

---

### Q6. Sessions, resume, history
- Flags [help]: `-r, --resume [<value>]` (value = session id, task id, id prefix of 7+ hex chars, or exact case-insensitive name; bare `--resume` opens a picker needing a TTY); `--continue` (most recent; docs: cwd first, then global); `-n/--name`; `--session-id <id>` (resume existing session/task by ID; creates new only if a valid UUID; not combinable with `--resume`/`--continue`); `--connect`; `-C <dir>`. `--resume` and `--continue` conflict. Session id is a UUID (help example `0cb916db-26aa-40f2-86b5-1ba81b225fd2`).
- Marshell can launch a new session with a chosen id via `--session-id <uuid>`, which would remove the need to discover the id.
- Exit resume hint: JS renders `copilot --resume=<sessionId>` after a label (`projection.resumeLabel`; label text is produced natively and not found: UNVERIFIED). With several sessions a numbered list `N) copilot --resume=<id> (name)`. Docs: `-p` exit summary includes `copilot --resume=SESSION-ID`.
- Layout [docs, src]: `~/.copilot/session-state/<session-id>/` (or `$COPILOT_HOME`) with `events.jsonl`, `workspace.yaml`, `plan.md`, `checkpoints/`, `files/`. Also `session-store.db` (SQLite cross-session search, `/chronicle reindex`), `command-history-state/`, `logs/process-{timestamp}-{pid}.log` (auto-pruned at startup [chg 1.0.52]). `history-session-state` not found anywhere in 1.0.106 (cli-main.js and runtime.node): treat as absent.
- `workspace.yaml` fields [src, fork/snapshot code]: `id`, `cwd`, `git_root`, `repository`, `host_type`, `branch`, `name`, `user_named`, `summary`, `summary_count`, `created_at`, `updated_at`, `fork_count`, `remote_steerable`, `mc_task_id`, `mc_session_id`, `mc_last_event_id`. A persisted `cwd` must be absolute and an existing directory on load.
- `events.jsonl`: one JSON event per line. Envelope `{id (uuid), timestamp (ISO 8601), parentId (uuid|null), ephemeral?, agentId?, checkpoint?, type, data}`. Schema `schemas\session-events.schema.json` (166 variants, 707 definitions). Ephemeral events (`session.idle`, `assistant.idle`, `session.title_changed`, `hook.progress`) are NOT persisted. Persisted types of use:
  - `session.start`: `sessionId`, `version`, `producer`, `copilotVersion`, `startTime`, `selectedModel`, `reasoningEffort`, `context{cwd, gitRoot, repository, branch}`. `session.resume`: `resumeTime`, `eventCount`, `selectedModel`, `context`.
  - `user.message`: `content`, `transformedContent`, `attachments`, `source`, `agentMode` (interactive|plan|autopilot|shell), `isAutopilotContinuation`, `messageId`, `turnId`.
  - `assistant.turn_start`/`turn_end`: `turnId`, `model`. `assistant.message`: `messageId`, `content`, `model`, `toolRequests[]`, `reasoningText`, `outputTokens`, `turnId`.
  - `tool.execution_start`: `toolCallId`, `toolName`, `arguments`, `mcpServerName`, `model`. `tool.execution_complete`: `toolCallId`, `success`, `result{content, detailedContent}`, `error`, `shellExecution{exitCode}`.
  - `session.model_change`, `session.mode_changed`, `session.permissions_changed`, `session.context_changed`, `session.error`, `abort`, `session.task_complete`, `permission.requested/completed`, `hook.start/end`, `session.compaction_*`, `subagent.*`, `system.notification`.
  - `session.shutdown`: `shutdownType` routine|error, `totalPremiumRequests`, `codeChanges`, `modelMetrics`, `currentModel` (persisted).
- Retention: no automatic age-based deletion of `session-state` found. Manual: `/session cleanup|prune|delete|delete-all`; SDK/RPC `pruneOld({olderThanDays, dryRun, includeNamed, excludeSessionIds})`. A "stale session reaper" exists [chg 1.0.12] (sessions with active work no longer cleaned): its policy UNVERIFIED. "Empty sessions no longer flash into the picker" [chg 1.0.106]: a session with no messages may not be resumable/listed.
- Remote sync: `remoteExport` (docs: boolean, default `true`, "Sync session history to your account when session sync is available"; refused when org policy is off/unknown); `--sync-session[=on|off]` per run; `remoteSessions` (default false) enables remote control and implies export. Whether it actually syncs on this account: UNVERIFIED. For privacy, Marshell may want to show or pass `--sync-session=off`.
- `copilot sessions` has only `import` [help]. A crash-restore offer appears at next startup [docs].

---

### Q7. Terminal signals and Windows notifications [src `cli-main.js`]
- Title (`updateTerminalTitle`, default true; off if `COPILOT_DISABLE_TERMINAL_TITLE` is "1"/"true"): pushes `ESC[22;0t` first, sets `OSC 0` (`ESC]0;<title>BEL`), restores with `ESC[23;0t` on exit. Text `<intent> - GitHub Copilot`, `<name> - <intent> - GitHub Copilot`, or `GitHub Copilot` with no intent. Written only when stdout is a TTY and TERM is not dumb.
- Progress (`terminalProgress`, default true): `OSC 9;4;3;0` BEL (indeterminate) while the agent works, re-sent every 5 s; `OSC 9;4;0;0` BEL clears. This is a usable working/idle signal from the pty stream.
- Bell (`beep`): 1.0.106 help and source say default **false** (`Kn.beep===!0`); this contradicts an older note that said default true. Source wins. When on, BEL on `session.idle` (100 ms debounce) and attention events. `beepOnSchedule` default true (only with beep on).
- `notifications` (default false): OS notifications when attention is needed and when the agent finishes; suppressed while the terminal has focus. Summary = session title (fallback "Copilot"); body "Agent finished: <last assistant message>" or "<activity label | Needs your attention>: <body>". `COPILOT_DISABLE_DESKTOP_NOTIFICATIONS=1` disables. Windows toast via the native backend (WinRT `ToastNotificationManager`, AUMID `GitHub.Copilot.CLI`, key under `HKCU\SOFTWARE\Classes\AppUserModelId\...`, DisplayName "GitHub Copilot CLI").
- `terminalNotifications` (default false, needs `notifications`): `OSC 777;notify;<summary>;<body>ST`, used for recognised Ghostty and WezTerm; tmux/screen/Zellij/unsupported terminals fall back to the OS backend. xterm.js would not be recognised, so with `notifications` on, Copilot raises its own Windows toasts, which can duplicate Marshell's.
- Taskbar presence (`taskbarPresence`, default true, restart needed; also gated by server-side rollout flag `TASKBAR_PRESENCE`): a Windows taskbar "app task" feature, not a toast. Sparse MSIX `GitHub.Copilot.CLI` (min OS build 10.0.26100, external location = exe folder) gives package identity; the AppxManifest declares `com.microsoft.apptaskprovider` and an app execution alias. The CLI relaunches itself through the WindowsApps alias to attach identity, so the alias-relaunched child is a second process (matters for pty child PIDs and process trees). It writes `%LOCALAPPDATA%\Packages\GitHub.Copilot.CLI_*\SystemAppData\AppTasks\tasks.json`; `copilot-taskbar-activator.exe` handles `local+copilot://...?t=session` click URIs and forwards a focus request over named pipe `\\.\pipe\copilot-taskbar-*`. On this PC no `GitHub.Copilot*` package, `Packages` folder or alias exists, so it has not provisioned here. Whether it activates for a user: UNVERIFIED (flag-gated). The setting can be turned off if the relaunch is a problem.

---

### Q8. Exe resolution through the VS Code shim
- PATH on this PC (user PATH) has `C:\Users\Home Office\AppData\Local\Programs\Microsoft VS Code\bin\copilot-shim` before anything else Copilot-related. `Get-Command copilot -All` lists only that folder's `copilot.exe` and the sh script `copilot`. The real exe `C:\Users\Home Office\AppData\Local\GitHubCopilotCLI\copilot.exe` sits beside `copilot-taskbar-activator.exe` and `GitHubCopilotCLI.sparse.msix`. Corrected by the main thread: that folder is on the user PATH in the registry, after the shim's folder; this pass's session environment predated the install.
- Shim `copilot.exe`: file description "GitHub Copilot CLI launcher", version 0.1.2, a Rust binary (`VSCODE_COPILOT_RUST_SHIM_V1`, `src\candidate.rs`, `src\setup\windows.rs`, user agent `VSCodeCopilotShim/0.1.2`).
- Shim strings [src]: it checks `LOCALAPPDATA` + `GitHubCopilotCLI`; registry `SOFTWARE\Policies\Microsoft\...` (VSCode, VSCodeInsiders, ... `CopilotCliCommandEnvironment`); `msi.dll`/`MsiEnumRelatedProductsW`; WinHTTP download with Authenticode verification; spawn flags `-NoLogo -NoProfile -ExecutionPolicyBypass -File`; options `--vscode-shim probe|install|policy|--scope|--timeout-ms`.
- Messages: "GitHub Copilot CLI was not found. Run copilot in a terminal to install it, or see https://docs.github.com/en/copilot/how-tos/copilot-cli/set-up-copilot-cli/install-copilot-cli"; "The install-only command requires an interactive terminal to install it"; "Installing it from VS Code is turned off by the policy; contact your administrator."; "Could not start GitHub Copilot CLI at ..."; "The installation completed, but no usable GitHub Copilot CLI was found on PATH or in the expected installation location."; "candidate is unusable because its interpreter is unavailable". Diagnostics: `copilot --vscode-shim verbose` or `VSCODE_COPILOT_SHIM_VERBOSE=1`.
- Sh script `copilot` (`vscode-copilot-launcher`): outside WSL runs `exec "$LAUNCHER_DIR/copilot.exe" "$@"`; in WSL looks for the VS Code Server's shim and strips launcher folders from PATH; otherwise prints "The copilot command runs GitHub Copilot CLI through the VS Code Server, which isn't installed in this WSL distribution yet..." and exits 127.
- Conclusion for Marshell: a bare `copilot` can hit the shim, which may print an install prompt and, when run non-interactively, fail. The robust order is: `%LOCALAPPDATA%\GitHubCopilotCLI\copilot.exe` first, then PATH entries that are not a `copilot-shim` folder (check the path segment), then the shim only as a last resort and with a "may prompt to install" warning. The real exe resolves from the `--version` output "GitHub Copilot CLI x.y.z." (the shim should be asked with `--vscode-shim probe`, UNVERIFIED semantics).
- UNVERIFIED: the shim's exact candidate search order (the strings suggest PATH without the launcher folder plus `%LOCALAPPDATA%\GitHubCopilotCLI`), and whether it forwards to the real CLI when it finds it. I did not run the shim.

---

### Other verified facts
- Help topics: `acp, billing, commands, config, environment, limits, logging, monitoring, permissions, providers, sandbox`. `--experimental` / `--no-experimental` exist. `COPILOT_HOME` overrides `~/.copilot`.
- Flags: `--output-format json` (JSONL for `-p`), `--bash-env`, `--screen-reader`, `--no-mouse`, `--banner`, `--no-ask-user`, `--add-dir`, `--plugin-dir`, `--additional-mcp-config`, `--log-dir`, `--log-level`, `--secret-env-vars`, `--share`, `--usage-output-file`, `--no-custom-instructions`.
- Custom instructions come from `AGENTS.md` and related files; the CLI reads a cross-tool subset of `.claude/settings.json` (including `hooks`, `disableAllHooks`).

### Conflicts with earlier notes
1. `beep` default: older note true, 1.0.106 source/help false.
2. `statusLine`: older note "experimental", source shows no gate.
3. `userPromptSubmitted` output: docs say dropped; changelog 1.0.44/1.0.65 say it works.
4. Event count: 15 in the runtime (undocumented `preMcpToolCall`), notes said 14.
5. `UserPromptSubmit` (docs) vs `UserPromptSubmitted` (runtime).
6. `history-session-state` is absent from 1.0.106.

### UNVERIFIED list
- Hook PowerShell argv; behaviour of a `bash`-only entry on Windows.
- ~~Whether user hooks run in an untrusted folder.~~ They do (spike Q1).
- ~~`permissionRequest` input shape; whether it fires under `--allow-all`/autopilot; its timeout behaviour.~~ Spike Q3, Q4 and Q5.
- ~~`userPromptSubmitted` `additionalContext` for command hooks.~~ It reaches the model (spike Q7).
- Accepted PascalCase spellings.
- Env vars stripped vs set for hook processes. The set ones are in spike Q3; which are stripped wasn't checked.
- Status line ANSI support. ~~JSON nesting~~ is in spike Q8.
- ~~Exit resume-hint label text.~~ `Resume copilot --resume=<id>` (spike Q9).
- "Always" persistence key and file.
- Stale-session reaper policy.
- Whether `remoteExport` actually syncs here.
- Taskbar presence activation.
- Shim candidate search order. ~~Forwarding~~: the shim starts the real exe as its child (spike Q11).
- ~~`transcriptPath` target.~~ `<COPILOT_HOME>\session-state\<id>\events.jsonl` (spike Q3).

## Part 2: surfaces and the Doctor (6n2, 6n3)

Scope: facts only, no feature design. Target: winget `GitHub.Copilot`, exe `C:\Users\Home Office\AppData\Local\GitHubCopilotCLI\copilot.exe` (Node SEA, 184 MB). It extracts its bundle to `%LOCALAPPDATA%\copilot\pkg\win32-x64\1.0.106\`. Most logic is in the Rust native module `prebuilds\win32-x64\runtime.node`; the JS (`cli-main.js`) holds help text, CLI wiring and some logic.

Method and rules followed: nothing was started that writes. Only `--version`, `--help`, `help <topic>` and `<sub> --help` were run. Everything else is a static read of the extracted bundle (`cli-main.js`, `sea-loader.js`, `schemas\*.json`, strings in `runtime.node`), the GitHub docs pages saved in `docs\`, and the real files on disk. Where source and docs disagree, source wins and the section says so. Helper scripts are in `tools\`; help dumps in `help*.txt` and `help2\`.

Evidence tags: [help] = text printed by `copilot ... --help`/`help <topic>`; [docs] = GitHub docs page (docs\ folder or URL); [cli-main] = grep of cli-main.js; [rt] = string grepped from runtime.node; [schema] = `schemas\api.schema.json` or `session-events.schema.json` description; [disk] = real file on this machine.

---

### Q1. Settings files

#### Location
- Config home is `COPILOT_HOME`, else `~/.copilot`. [help environment] "`COPILOT_HOME`: override the directory ... defaults to `$HOME/.copilot`". [cli-main] `resolveCopilotHome(configDir, process.env.COPILOT_HOME, homedir)`.
- Cache/state is a separate dir: `copilotCacheHome(platform, home, COPILOT_CACHE_HOME, LOCALAPPDATA, XDG_CACHE_HOME)`. Windows `%LOCALAPPDATA%\copilot`, Linux `$XDG_CACHE_HOME/copilot` or `~/.cache/copilot`, macOS `~/Library/Caches/copilot`. [docs cli-config-dir-reference]
- A legacy-migration routine (`mpr("XDG_STATE_HOME", ...)`, `mpr("XDG_CONFIG_HOME", ...)` in cli-main) moves old `$XDG_*/.copilot` items into `~/.copilot` and logs "XDG migration". Items include `session-state`, `session-store.db`, `command-history-state(.json)`, `installed-plugins`, `mcp-config`.

#### User settings file: `~/.copilot/settings.json`
- Format is JSONC. [docs] "The file supports JSON with comments (JSONC)." [schema] `repo/settings/readRaw`: "The existing JSONC parser accepts comments and trailing commas and applies its prototype-safety rules. Missing or blank files return an empty valid config. Invalid files return an empty config with invalid=true". [rt] vendors `jsonc-parser-0.34.0/src\serde.rs` (parse to serde value).
- Load status is `ok|missing|invalid` (`UserSettingsLoadForEditResult`).
- Tolerant load: invalid values are ignored with a startup warning and a Problems tab in `/settings`; unknown top-level keys are listed in Problems; `$schema` is tolerated.
- A file `settings.json` is the only one read. [schema] `loadForEdit`: "Retired config.json and extensionless files are not read." `UserSettingsPathResult`: an extensionless legacy `settings` is reported only when `settings.json` does not exist.

#### `config.json` vs `settings.json`
- `config.json` is CLI-owned internal state and its settings keys are ignored. [docs] says it holds "authentication data, installed plugin metadata", trusted folders, etc. [disk] the real file here is a `//` header ("User settings belong in settings.json." / "This file is managed automatically.") then `{"firstLaunchAt": "..."}` (so JSONC with a comment header).
- Global-state keys found in runtime.node: `installedPlugins`, `marketplaces`, `firstLaunchAt`, `askedSetupTerminals`, `suppressInitFolders`, `sandboxOnboardingShown`, `sandboxCredentialProxyCaDeclined`, `appTipShown`, `reasoningSummariesCleanupDone`, `copilotTokens`, `authTokens` (string "holds stored credentials and cannot be written"), `lastLoggedInUser`, `loggedInUsers`, `staffGithub`, `staffMicrosoft`, `staffModelResetAt`, `staffLogLevelMigrationAt`, `staffUpdateChannelMigrationAt`, `trustedFolders`, `recentModelIds`, `retiredMarketplacesMigrated`, `autoFeedbackLastPromptedAt`.
- `trustedFolders` is in config.json, not settings.json.

#### Project / local / policy files and precedence
- Repo: `.github/copilot/settings.json` and `.github/copilot/settings.local.json`. [help config] (`copilot config --repo/--local`). Only repo-overridable keys are allowed there. Merge policy per key: `model`/`effortLevel` replaced; `deniedUrls`/`disabledMcpServers`/`disabledSkills` union; `respectGitignore` tighten-only; `hooks`/`enabledPlugins` merged.
- Also read for a shared subset: `.claude/settings.json` and `.claude/settings.local.json` (`companyAnnouncements`, `disableAllHooks`, `enabledPlugins`, `extraKnownMarketplaces`, `hooks`). [docs]
- Documented precedence: defaults, managed/MDM, user, repo, local, env vars, CLI flags. [docs]
- Managed/policy sources: registry `HKLM\SOFTWARE\Policies\GitHubCopilot`; file `%ProgramFiles%\GitHubCopilot\managed-settings.json` [rt: `ProgramFiles` + `GitHubCopilot\managed-settings.json`]; macOS `/Library/Application Support/GitHubCopilot/managed-settings.json`; Linux `/etc/github-copilot/managed-settings.json` [docs]; hooks policy dir `C:\ProgramData\GitHub\Copilot\policy.d` [rt]. Embedded schema URL `https://github.com/schemas/copilot/managed-settings/v0`.
- Server-managed settings cache [disk]: `%LOCALAPPDATA%\copilot\managed-settings\<sha256>.json`, header "Disposable cache for enterprise managed settings, safe to delete", body `{"schemaVersion":1,"retrievedAtMs":...,"account":"<hash>","response":null,"ignoredPaths":[]}`.

#### Runtime writes to settings.json
- [schema]/[rt]: `user.settings.set` replaces one top-level key. `edit` is atomic and under a lock. `applyTopKey` rewrites only the touched key. `writeAll` replaces the whole file. `setHookDisabled` rewrites `disabledHooks`. `migrateObsoleteOnDisk` runs at startup, removes obsolete CLI-owned keys, preserves unknown keys, and leaves an unparsable file untouched ([rt] "refusing to overwrite settings.json (the existing file could not be parsed)").
- Things that persist to it: `/model`, `/settings`, `copilot config`, `copilot mcp enable/disable` (`disabledMcpServers`), `copilot skill enable/disable`/`add <dir>`, `autoUpdatesChannel`, `--bash-env`/`--mouse` flags. [help/docs] Symlinks are followed on write. [docs]
- UNVERIFIED: whether comments and key order survive a CLI write. No CST or comment-preserving writer was found in runtime.node; it probably goes through serde_json. Treat a write as comment-lossy until tested on a copy.

#### Key list (types and defaults)
Source: `help-config.txt` ([help config]) and the table in `docs\cli-config-dir-reference.md`. Main keys:
- Models: `model`, `modelProviderId`, `effortLevel`, `planModel`, `contextTier`, `autoTier`.
- UI: `theme` "github" (`colorMode` legacy alias), `defaultMode` "interactive", `defaultPermissionMode` "manual" (assisted/allow-all), `notifications` false, `terminalNotifications` false, `terminalProgress` true, `updateTerminalTitle` true, `statusLine` {type:"command", command, padding, refreshInterval}, `footer.*`, `beep`.
- Update: `autoUpdate` true (false in CI), `autoUpdatesChannel` "stable"|"prerelease".
- Hooks: `hooks`, `disableAllHooks`, `disabledHooks`.
- MCP: `disabledMcpServers`, `enabledMcpServers`, `githubMcpToolset(s)`, `githubMcpTools`, `enableAllGithubMcpTools`.
- Skills: `disabledSkills`, `skillDirectories`, `ignoredSkillsLocations`, `dynamicRetrieval.skills`, `dynamicRetrieval.mcp`, `toolSearch`.
- Other: `experimental` false, `remoteExport`, `remoteSessions` false, `storeTokenPlaintext` false, `allowedUrls`, `deniedUrls`, `sandbox.*`, `telemetry.*`, `powershellFlags` ["-NoProfile","-NoLogo"], `proxyUrl`, `enabledPlugins`, `extraKnownMarketplaces`, `strictKnownMarketplaces`, `memory` true, `includeCoAuthoredBy` true, `logLevel`, `copilotUrl`, `commandHistoryMaxSize` (50, 1 to 1000).
- DISCREPANCY (source wins): `beep` default is `true` in the docs table; `copilot help config` says it "defaults to `false`". `notifications` (OS toasts) appears in help but not in the docs table. The project's own research note records the same beep conflict.

#### Secrets
- `storeTokenPlaintext` (when true tokens go to config.json in plain text), `sandbox.userPolicy.network.proxy.password` (literal, `${secret:...}` or `${VAR}`), `telemetry.headersProvider`, `sandbox.credentials.envVars`, `hooks` (commands/headers may embed secrets), MCP `env`/`headers` in mcp-config.json, `providers.json` apiKey/bearerToken/headers (plain text per docs), `copilotTokens`/`authTokens` in config.json. `copilot config` redacts secret values ("Secret values are redacted from output"); `copilot config --list --json` exists.

---

### Q2. Accounts

- Only one subcommand: `copilot login` (`--host`, `--device-code`, `--web-flow`, `--with-token`). [help2\login] There is no `logout` or `whoami` subcommand. `/login`, `/logout`, `/user [show|list|switch]` are interactive slash commands. [help commands]
- No non-interactive account status command exists. The JSON-RPC API has `account.getCurrentAuth`, `getAllUsers`, `getQuota` [schema], reachable only through an ACP/SDK server. UNVERIFIED that any of them is usable without a session or model.
- Storage: [help login] token "stored securely in the system credential store. If a credential store is not found ... plain text config file under ~/.copilot/." [schema] `AccountLoginResult.storedInVault`: "system keychain, or the config file when plaintext storage is enabled"; a `needs-plaintext-consent` step exists. Logout: "Removes user authentication from keychain and persisted state."
- Non-secret identity in config.json: `lastLoggedInUser`, `loggedInUsers` (host + login). Secret fallback: `copilotTokens`/`authTokens` in config.json.
- The Windows Credential Manager target name is UNVERIFIED. `cmdkey /list` showed no copilot entry here (only gh/git), consistent with the real config.json holding only `firstLaunchAt` (not signed in on this machine, or tokens elsewhere).
- Env tokens, in order: `COPILOT_GITHUB_TOKEN`, `GH_TOKEN`, `GITHUB_TOKEN`; they win over stored credentials. Classic `ghp_` PATs are refused; fine-grained PATs need the "Copilot Requests" permission. [help environment]
- `gh` reuse: auth variant `GhCliAuthInfo` ("token returned by `gh auth token`"); [rt] `credential_sources\gh.rs` runs `gh auth token --hostname`; the no-auth error says "Run 'gh auth login'".
- Also: `GH_HOST`/`COPILOT_GH_HOST`, a Microsoft Entra account type, `forceLoginOrgs` policy, `GITHUB_ASKPASS`.

---

### Q3. Version and update

- `copilot --version` prints `GitHub Copilot CLI 1.0.106.` then `Run 'copilot update' to check for updates.` No network. [ran]
- `copilot version` and `copilot update` do a release lookup (network); `version` prints "Update available: X" plus guidance or "You are running the latest version." [help2, cli-main]
- Install type signals: the winget exe is the SEA (ProductVersion 1.0.106) at `%LOCALAPPDATA%\GitHubCopilotCLI\copilot.exe` beside `copilot-taskbar-activator.exe` and `GitHubCopilotCLI.sparse.msix`; `winget list --id GitHub.Copilot` shows 1.0.106. An npm `@github/copilot` install has `npm-loader.js` as bin (`package.json` `bin.copilot`, no `engines` field). [disk]
- SEA loader (`sea-loader.js`): auto-update off with `--no-auto-update`, `--prefer-version`, or `COPILOT_AUTO_UPDATE=false`. Pkg cache search order: `COPILOT_PKG_CACHE_HOME/pkg`, `COPILOT_CACHE_HOME/pkg`, `%LOCALAPPDATA%\copilot\pkg`, `$XDG_CACHE_HOME` or `~/.cache/copilot/pkg`, `COPILOT_HOME/pkg`, `~/.copilot/pkg`. Subdirs `universal`, `win32-x64`, each `<version>\index.js`. With auto-update on it picks the newest valid cached version that differs from and is >= the bundled one; otherwise the bundled one extracted to `<cache>\pkg\win32-x64\1.0.106` (marker `.extraction-complete`). `COPILOT_CLI_DIST_DIR`/`COPILOT_CLI_VERSION` are dev overrides.
- Updater installs to `<state>/pkg` if it exists else `<cache>/pkg`, at `<pkg>/<subdir>/<version>`.
- Standalone prints "Run 'copilot update'" (or `/update`); npm installs print "To update, run: npm i -g @github/copilot@<version>"; auto-update on a JS install only notifies ("Update not supported when running js directly").
- `autoUpdate` default true (false when `CI`/`BUILD_NUMBER`/`RUN_ID`/`SYSTEM_COLLECTIONURI` is set); a downloaded version applies at the next launch; turning `autoUpdate` off does not remove it. winget upgrade replaces the exe separately.
- No on-disk "update available" record was found other than a newer `pkg\<platform>\<ver>` dir. Anything else is UNVERIFIED. Here only `1.0.106` is cached.

---

### Q4. Skills

- Locations. [docs, help2\skill, schema SkillSource enum `project, inherited, personal-copilot, personal-agents, plugin, custom, builtin, sdk, mcp`]:
  - Project: `.github/skills`, `.agents/skills`, `.claude/skills` (plus parent `.github/skills` in monorepos).
  - Personal: `~/.copilot/skills`, `~/.agents/skills`.
  - Plugins; custom dirs (`COPILOT_SKILLS_DIRS` comma list, `skillDirectories` setting, `skill add <dir>`); `--add-dir` roots; builtin (`builtin-skills/`: customize-cloud-agent, discover-resources, github-pr-media, pr-stack; lowest priority); remote org skills; MCP-served skills.
- First found wins by `name`; plugin skills with duplicate names coexist as `/plugin/name`. `.claude/commands/*.md` rank below skills.
- `ignoredSkillsLocations` excludes dirs; `disabledSkills` (settings; `skill enable/disable`) means "discovered but not loaded". `copilot skill list --json` rows: `{name, description, source, path, enabled}`.
- Frontmatter: `name` (required, max 64), `description` (required, max 1024), `argument-hint`, `allowed-tools`, `user-invocable`, `disable-model-invocation`.
- Writes: `skill add <file|url>` materializes into `~/.copilot/skills/<name>/SKILL.md` (or `.github/skills` with `--project`); a directory is registered as a custom source; `remove` deletes personal/project skills.
- Token cost: the runtime prompt has an `<available_skills>` listing of names and descriptions; bodies load on demand through the `skill` tool or `/name`. `SKILL_CHAR_BUDGET` exists in the runtime (default UNVERIFIED). `dynamicRetrieval.skills` switches to embedding retrieval (`COPILOT_FORCE_EMBEDDING_RETRIEVAL`).
- Usage events [schema session-events]: `session.skills_loaded` is ephemeral (not in events.jsonl). `skill.invoked` is persisted with `name, path, source, trigger (user-invoked|agent-invoked|context-load), content, arguments, allowedTools, pluginName, pluginVersion, description, invokedAtTurn`. `skill.invoked_ref` is a deduplicated receipt (`contentId`, `contentLength`). `skill.context_delivered(_ref)` carries `source: skill-<name>`. API `session.skills.getInvoked` exists.

---

### Q5. MCP

- Config files: user `~/.copilot/mcp-config.json` (key `mcpServers`; lock `.copilot-mcp.lock`; legacy extensionless `mcp-config` in the migration list); workspace `.mcp.json` and `.github/mcp.json` from cwd up to the git root ([cli-main] `Nsi=[".mcp.json", ".github/mcp.json"]`; folder must be trusted); plugin servers (`.mcp.json`, `.github/mcp.json`, manifest `mcpServers`, `mcp.json`); `--additional-mcp-config <json|@file>`; managed.
- `.vscode/mcp.json` is NOT read. [cli-main] "Copilot CLI's incomplete support for .vscode/mcp.json has been removed. See https://gh.io/copilotcli-mcpmigrate". Migration is `servers` to `mcpServers`.
- Priority [docs]: `--additional-mcp-config` > plugins > workspace > user.
- UNVERIFIED: whether mcp-config.json is parsed as JSONC. Invalid workspace entries are skipped one by one with "Warning: workspace MCP config ...".
- Server fields: stdio `command, args, cwd, env, tools, type (local|stdio), timeout (default 30000; docs also mention a 60000 floor), deferTools (auto|never), disableToolCache, slowConnectionThresholdMs (default 10000, warning only), displayName, filterMapping, excludeTools`. Remote `type (http|sse|streamable-http), url, headers, oauthClientId/oauthScopes/oauthPublicClient/oauthGrantType, oidc, auth, headersRefreshTtlMs`. Env expansion `$VAR`, `${VAR}`, `${VAR:-default}`.
- Subcommands [help2\mcp]: `list [--json]`, `get <name> [--json] [--show-secrets]`, `add` (writes mcp-config.json; `--transport stdio|http|sse`, `--env`, `--header`, `--tools`, `--timeout`), `remove` (user scope only), `enable`, `disable`. [cli-main] `mcp list` is config-only: it merges config and prints `{...config, mcpServers:{name:{...,source,enabled}}}` without connecting; groups are User/Workspace/Plugin/Builtin.
- Disabling: persisted `disabledMcpServers` in settings.json (`McpConfigDisableRequest`: "added to the persisted disabled list so new sessions skip it"); per run `--disable-mcp-server`, `--enable-mcp-server`, `--disable-builtin-mcps`. Opt-in built-ins use `enabledMcpServers` (computer-use off by default).
- Trust: workspace servers need a trusted folder (config.json `trustedFolders`); `--trust-folder` covers one `-p` session; `COPILOT_ALLOW_ALL=true` also trusts; `GITHUB_COPILOT_PROMPT_MODE_WORKSPACE_MCP=true` loads them in `-p`.
- Built-ins: `github-mcp-server` (CLI tool subset; `--add-github-mcp-toolset/tool`, `--enable-all-github-mcp-tools`), `playwright`, `fetch`, `time`, `computer-use` [docs]. DISCREPANCY: `--disable-builtin-mcps` help says "currently: github-mcp-server, githubiq"; the docs list differs, so the docs list may be stale (source/help wins).
- Deferred loading: `toolSearch`, per-server `deferTools`, `dynamicRetrieval.mcp`, a tool-snapshot cache (`COPILOT_MCP_TOOL_CACHE=false` disables it).
- Tool naming [docs]: `<server>-<tool>`, sanitized to `a-zA-Z0-9_-`, capped at 64, numeric suffix on collision.
- Usage evidence [schema session-events]: persisted `tool.execution_start` has `toolName`, `mcpServerName` (configured key), `mcpConfigServerName`, `mcpToolName` (original), `mcpTransport`, `mcpConfigSource`, `mcpServerDisplayName`, `toolCallId`; `tool.execution_complete` has `success`, `error`. `session.shutdown` has `toolDefinitionsTokens`, `systemTokens`, `conversationTokens`, `currentTokens`.
- Ephemeral (not in events.jsonl): `session.mcp_servers_loaded` (status `connected|failed|needs-auth|pending|disabled|stopped|not_configured`, source `user|workspace|plugin|builtin|managed|account`), `session.mcp_server_status_changed`, `session.mcp_server_tools_loaded {serverName, toolCount}`.
- Managed policy: `allowedMcpServers`/`deniedMcpServers` (serverUrl/serverCommand/serverName matchers). Plugin servers expand `${PLUGIN_ROOT}`/`${PLUGIN_DATA}`.

---

### Q6. Doctor

- No `copilot doctor` subcommand, no `/doctor` slash command (grep of cli-main hit only emoji keywords).
- `/diagnose [PROMPT]` (interactive) sends the session log to the model, so it needs a model and network. `/env` shows loaded instructions, MCP servers, skills, agents, hooks, plugins, LSPs, extensions. `/collect-debug-logs` builds a .tgz.
- Offline config inspectors: `mcp list --json`, `skill list --json`, `plugin list --json`, `instruction list --json`, `config --list --json`, `lsp`, `sandbox ca status`. `copilot version` is network.
- `schemas\lifecycle-diagnostics.schema.json` is "experimental lifecycle diagnostic events emitted by the Copilot CLI runtime" (`visibility: internal`), not a user doctor.
- Logs: `~/.copilot/logs/process-<timestamp>-<pid>.log` (`--log-dir`, `--log-level`).
- The project note (`docs/research/2026-10-25-agent-and-tool-cli-manifests.md`, 1.0.103) also says "No non-interactive status command found"; still true for 1.0.106.

---

### Q7. Plugins and agents

- `copilot plugin install|uninstall|update|list [--json]|enable|disable|marketplace (add|remove|list|browse|update)`. Sources: `plugin@marketplace`, `owner/repo`, `owner/repo:path`, git URL, `--from-github`. Default marketplace `awesome-copilot` (`github/awesome-copilot`). [help2\plugin, docs\cli-plugin-reference.md]
- Installed to `~/.copilot/installed-plugins/<marketplace>/<plugin>` or `_direct/<source-id>`; data in `plugin-data/`; marketplace cache `%LOCALAPPDATA%\copilot\marketplaces`. State in config.json (`installedPlugins`, `marketplaces`). Settings: `enabledPlugins`, `extraKnownMarketplaces`, `strictKnownMarketplaces`.
- Plugins can contribute skills, agents, hooks, MCP servers, LSP servers, commands, rules. Manifest `plugin.json` (Agent Plugins 1.0/1.1; legacy `.plugin/`, `.github/plugin/`, `.claude-plugin/`). The bundle also ships `plugins\browser-use` and `plugins\computer-use`.
- Custom agents: `~/.copilot/agents/*.agent.md`, `.github/agents`, `.claude/agents`, plugin agents, remote org agents; dedupe by file ID, first found wins. Bundled `definitions\*.agent.yaml`.

---

### Q8. Instruction files (brief)

[docs, help2\instruction] Read and merged: `CLAUDE.md`, `GEMINI.md`, `AGENTS.md`, `.github/copilot-instructions.md`, `.github/instructions/**/*.instructions.md`, `.claude/rules/**/*.md` (git root and cwd), `~/.copilot/copilot-instructions.md`, `~/.copilot/instructions/**/*.instructions.md`, plus `COPILOT_CUSTOM_INSTRUCTIONS_DIRS`. `@path` imports work. `--no-custom-instructions` disables all. `copilot instruction list [--json]` inspects sources. `copilot init` and `/init` write `.github/copilot-instructions.md`.

---

### Q9. WSL / Linux

- Config dir `~/.copilot` or `COPILOT_HOME`; legacy XDG items migrate into it. Cache `$XDG_CACHE_HOME/copilot` or `~/.cache/copilot`. [cli-main, docs]
- Install methods [README of github/copilot-cli, fetched]: `curl -fsSL https://gh.io/copilot-install | bash` (macOS/Linux; `VERSION=` and `PREFIX=` env), Homebrew cask `copilot-cli` / `copilot-cli@prerelease`, WinGet `GitHub.Copilot` / `.Prerelease`, `npm install -g @github/copilot` (also `@prerelease`). Container image `ghcr.io/github/copilot-cli` (linux/amd64 and arm64, headless server for SDK clients).
- npm package: bin is `npm-loader.js`; `package.json` has `copilotRuntime.linuxOpenSSL: "system"`; no `engines` field. The Node version requirement is UNVERIFIED (README has a Prerequisites heading not captured here).
- A Windows Copilot install does not carry over into WSL: the WSL side has its own `~/.copilot`. UNVERIFIED whether a WSL session reads the Windows home.
- The Linux SEA file name and install prefix of the install script are UNVERIFIED (script not downloaded or run).

---

### Q10. BYOK / running without a subscription

- [help providers, help environment]: "Set the COPILOT_PROVIDER_BASE_URL environment variable to activate BYOK mode. GitHub authentication is not required when using a custom provider."
- Env vars: `COPILOT_PROVIDER_BASE_URL`, `COPILOT_PROVIDER_TYPE` (openai|azure|anthropic), `COPILOT_PROVIDER_API_KEY` (optional for local), `COPILOT_PROVIDER_API_KEY_COMMAND`, `COPILOT_PROVIDER_BEARER_TOKEN`, `COPILOT_PROVIDER_WIRE_API` (completions|responses), `COPILOT_PROVIDER_TRANSPORT` (http|websockets), `COPILOT_PROVIDER_MODEL_ID`, `COPILOT_PROVIDER_WIRE_MODEL`, `COPILOT_PROVIDER_MAX_PROMPT_TOKENS`, `COPILOT_PROVIDER_MAX_OUTPUT_TOKENS`, `COPILOT_PROVIDER_VISION`, `COPILOT_PROVIDER_HEADERS`, `COPILOT_MODEL`. A model is required (`COPILOT_MODEL` or `--model`).
- Documented example: `COPILOT_PROVIDER_BASE_URL=http://localhost:11434/v1 COPILOT_MODEL=... copilot`. For OpenAI-compatible types the base URL must include the prefix such as `/v1`.
- Registry file: `$COPILOT_HOME/providers.json` (`COPILOT_PROVIDERS_CONFIG` overrides the path), JSON without comments, `providers[]` (name, type openai|azure|anthropic|local_model, baseUrl, wireApi, transport, apiKey, bearerToken, headers...) and `models[]` (id, provider, wireModel, maxPromptTokens...). Select with `--model PROVIDER/MODEL`. When it declares anything it takes precedence over `COPILOT_PROVIDER_*` ([docs]); [rt] string: "Ignored discovered providers config ... an active COPILOT_PROVIDER_* provider takes precedence" suggests the opposite when the env var is set. Source vs docs disagree on precedence; treat as UNVERIFIED and avoid having both present.
- `COPILOT_OFFLINE=true`: skips GitHub auth, telemetry, web tools, the GitHub MCP server and auto-update; requires a local provider ([rt] "Offline mode requires a local model provider. Set COPILOT_PROVIDER_BASE_URL to configure one.").
- A GHES provider path also exists (`COPILOT_PROVIDER_GHES_HOST/TOKEN`).
- UNVERIFIED (no session could be started under the rules): that a fake loopback OpenAI-compatible server works end to end; which endpoints it must serve (`/v1/chat/completions`, `/v1/models`?); whether any GitHub call, trust prompt, first-run prompt or telemetry request still occurs in BYOK without `COPILOT_OFFLINE`; whether a model list probe happens.

---

### Q11. Transcript retention

- Store: `~/.copilot/session-state/<id>/` with `events.jsonl`, `workspace.yaml`, `plan.md`, `checkpoints/`, `files/`; plus `~/.copilot/session-store.db` (rebuildable with `/chronicle reindex` per [docs]).
- Event envelope: `{id, timestamp, parentId, ephemeral?, agentId?, type, data}`; `session.start` data has `sessionId, version, producer, copilotVersion, startTime, selectedModel, context`.
- Pruning is manual. [cli-main] slash command `/session` has sub-commands `cleanup` ("Remove empty or abandoned sessions"), `prune` ("Delete old sessions"), `delete`, `delete-all`; [rt] usage "Usage: /session prune --older-than <days> [--dry-run] [--include-named]". API `sessions.pruneOld {olderThanDays, dryRun, includeNamed, excludeSessionIds}` returns deleted IDs, dry-run candidates, skipped IDs and bytes freed [schema]. `sessions.bulkDelete` deletes workspace dirs and reports bytes freed.
- No call to `pruneOldSessions` was found outside that API/slash path in cli-main, and no retention setting exists in the key list (only `commandHistoryMaxSize`, which is command history, not transcripts). So no automatic age/count prune of session-state was found. This is a negative result from grep, so UNVERIFIED for the native runtime (a startup prune inside runtime.node was not ruled out; only a "Failed to delete pruned session" string was seen, tied to prune-old).
- A runtime prompt contains "NEVER recursively delete ... session-state root" (model instruction, not retention).
- Docs: deleting `session-state/` removes only local copies.

---

### UNVERIFIED list
1. Comment/format preservation when the CLI rewrites settings.json.
2. Whether mcp-config.json accepts comments (JSONC).
3. Windows Credential Manager target name for the token.
4. Whether account API calls work without a session.
5. BYOK end to end: fake loopback server, required endpoints, other GitHub calls, first-run/trust prompts.
6. providers.json vs `COPILOT_PROVIDER_*` precedence (docs and runtime string differ).
7. Node requirement for the npm install; Linux install script layout; WSL home sharing.
8. Automatic session retention inside runtime.node.
9. Default of `SKILL_CHAR_BUDGET`.
10. Any on-disk "update available" record besides a newer pkg dir.
