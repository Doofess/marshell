# Antigravity CLI hooks, status line and payloads (slice 6d spike)

- Date: 2026-10-26, 01:43 to 02:30 local time.
- agy: `agy --version` printed `1.4.7` (it printed `1.4.0` when the slice was planned the same night; agy updated itself in between). Windows 11 Pro 10.0.26300.
- Home: a scratch `USERPROFILE` (`live-check.ps1 start -AgyAuth`), with `oauth_creds.json` and `google_accounts.json` copied from the real `~/.gemini` and nothing else. The dev app ran from `phase-6d` under `pnpm tauri dev` with a scratch `MARSHELL_HOME`, and agy ran in PowerShell 7 tabs opened in a scratch project, driven over CDP (the terminal's `input()` for keys, its buffer for reads). Some command-form probes ran `agy -p` from a scratch shell with the same `USERPROFILE`.
- First run: agy's onboarding asked for a colour scheme, the interactions-data opt-in (left off) and folder trust (given, for the scratch project only).
- Fixtures: `crates/core/tests/fixtures/antigravity/hooks/1.4.7/` and `statusline/1.4.7/`, scrubbed with `pnpm scrub:fixtures` (14 files); `cargo test -p marshell-core -j 4 --test fixtures_scrubbed` passed.

## What ran

`<home>\.gemini\config\hooks.json`, one named hook with every event of A1 plus a made-up one (`<rec>` is `pwsh -NoProfile -File <8.3 path>\rec.ps1`, see A2 for why it is a short path):

```json
{ "marshell-spike": { "enabled": true,
  "PreToolUse":  [{ "matcher": ".*", "hooks": [{ "type": "command", "command": "<rec> PreToolUse",  "timeout": 8 }] }],
  "PostToolUse": [{ "matcher": ".*", "hooks": [{ "type": "command", "command": "<rec> PostToolUse", "timeout": 8 }] }],
  "PreInvocation":  [{ "type": "command", "command": "<rec> PreInvocation",  "timeout": 8 }],
  "PostInvocation": [{ "type": "command", "command": "<rec> PostInvocation", "timeout": 8 }],
  "Stop":           [{ "type": "command", "command": "<rec> Stop",           "timeout": 8 }],
  "SessionStart":   [{ "type": "command", "command": "<rec> SessionStart",   "timeout": 8 }],
  "Bogus":          [{ "type": "command", "command": "<rec> Bogus",          "timeout": 8 }] } }
```

`<home>\.gemini\antigravity-cli\settings.json`: `{ "statusLine": { "type": "command", "command": "<rec> statusline" } }`, later with `"stack_with_default": true`.

The recorder saved stdin, `MARSHELL_TAB_ID`, `MARSHELL_HOME`, `ANTIGRAVITY_CONVERSATION_ID` and its parent and grandparent process names, then printed `answer\<Event>.txt` (or exited 1, or slept, for the special forms).

## Findings

### A1. `hooks.json` shape: confirmed

The shape is as assumed, read from `~/.gemini/config/hooks.json` with no trust step (log: `hooks_manager.go:53] loaded 1 named hooks from 1 hooks.json file(s)`). agy reloads the file while it runs: the log shows the load again mid-session after the file changed.

- `SessionStart` fires once when a new conversation starts, with the first prompt in `lastUserInput`. It does not fire when `--conversation <id>` resumes one (A8).
- An unknown event name (`Bogus`) is ignored without a warning or error in the log.
- A failing `SessionStart`, `PreInvocation`, `PostInvocation` or `Stop` hook is logged (`failed to call custom ... hook`) and the turn goes on.

### A2. The shell and quoting: corrected

agy runs a hook as `cmd /c "<command>"`: the probe's `%cmdcmdline%` read `cmd /c "<command>"`, the recorder's parent was `cmd.exe` and its grandparent `agy.exe`. The status line runs the same way. But agy passes the command as one argument that Go escapes, so every `"` inside the command reaches cmd as `\"`. A path quoted the usual way therefore breaks. `"C:\Users\Home Office\...\rec.ps1"` arrived at pwsh as `'"C:\Users\Home'` ("is not recognized as the name of a script file", exit 64).

What each form did (each run as a `SessionStart` hook in `agy -p`):

| Form | Result |
| --- | --- |
| `"C:\...\bridge dir\probe.cmd" V4` (quoted) | Failed. |
| `C:\Users\Home^ Office\...\bridge^ dir\probe.cmd V3` (caret-escaped spaces) | Ran for a `.cmd`. Failed (exit 1) for an `.exe`. |
| `<8.3 short path>\probeexe.exe` | Ran. 8.3 names can be turned off per volume, so Marshell can't count on them. |
| `C:"\Users\...\bridge dir"\probeexe.exe` | Ran. Each `"` becomes `\"`, cmd drops the quotes, and the path is left with doubled separators (`C:\\Users\...\bridge dir\\probeexe.exe`), which Windows accepts. |
| `C:"/Users/.../bridge dir"/probeexe.exe` (forward slashes) | Ran. |
| `C:"\...\o&b (1);x,y=z^q"\probeexe.exe` | Ran: `&`, `(`, `)`, `;`, `,`, `=` and `^` are safe inside the quotes. |
| `C:"\...\a&b "^%"USERNAME"^%" (1);x,y=z"\probeexe.exe` | Failed ("The system cannot find the path specified"): every quote leaves a `\` behind, so a quote is safe only next to a separator, and a `%` in a folder name can't be protected. |
| `C:"\...\bridge dir"\findprobe.exe /R . > file` | Ran with the hook's stdin and both arguments (`findstr` printed the JSON body), the end-to-end check of the form. |
| `echo %COMSPEC% ^& set MARSHELL > file` | Printed `C:\WINDOWS\system32\cmd.exe & set MARSHELL`: cmd expands `%VAR%`, and `^` escapes. |

So the bridge command that works is the bridge's folder in quotes from just after the drive to just before the file name, with plain arguments: `C:"/Users/<name>/.marshell/bin"/marshell-hook.exe <role> --agent antigravity --event <Name> --tag mshl1`. A `%` (cmd would expand it) or a `"` in the bridge path can't be written safely, so the install refuses those paths, as slice 6a did for Codex. The same applies to a path with no drive letter (UNC).

### A3. What agy does with each `PreToolUse` answer: corrected

| Answer | Default mode, `run_command` | Default mode, `read_file` in the workspace | `--dangerously-skip-permissions` |
| --- | --- | --- | --- |
| Empty stdout, exit 0 | agy's own prompt ("Run this command?") | Runs, no prompt | Runs, no prompt |
| `{}` | **Blocked**: "tool call denied by pre-tool hook" | **Blocked** | **Blocked** |
| Exit 1 | **Blocked**: `JSON hook "jsonhook__marshell-spike_PreToolUse_0_0" failed: command failed: exit status 1` | (as run_command) | (not run) |
| Invalid JSON | **Blocked**: `failed to unmarshal result from hook ... via protojson` | (as run_command) | (not run) |
| Timeout (12 s against an 8 s timeout) | **Blocked**: `command failed: exit status 1` | (as run_command) | (not run) |
| A command that doesn't start (the first, quoted run) | **Blocked**, every tool call, until the command was fixed | Blocked | (not run) |
| `{"decision":"allow"}` | agy's own prompt anyway | (not run) | (not run) |
| `allow` with `permissionOverrides: ["command(echo case-override)"]` | agy's own prompt anyway | | |
| `allow` for `write_file` | agy's own prompt anyway ("Allow creation of this file?") | | |
| `{"decision":"deny","reason":"Marshell spike says no"}` | Blocked; the agent sees "tool call denied by pre-tool hook: Marshell spike says no" | | Blocked, with the reason |
| `{"decision":"ask"}` | agy's own prompt; with an always-allow rule saved for the command, it runs with no prompt | Runs, no prompt | Runs, no prompt |
| `{"decision":"force_ask"}` | agy's own prompt, even with the always-allow rule saved | agy's own prompt ("Allow access to this file?") | **Runs, no prompt** |

What this means:

- **The neutral answer is zero bytes on stdout and exit 0.** It leaves agy exactly as with no hook. `{}` is a deny, unlike Claude Code and Codex. Any failure of the hook (exit code, timeout, bad JSON, a missing or misquoted command) blocks the tool. A bridge that is missing or misquoted would therefore stop every agy tool call until the hooks are removed.
- **A hook can't approve.** `allow` never skipped agy's prompt, not even with `permissionOverrides` naming the exact command in the grammar agy itself persists (A7). A hook can deny, ask (agy's normal flow) or force agy's prompt, but only in modes where agy prompts at all.
- **Under `--dangerously-skip-permissions`, `ask` and `force_ask` are both skipped.** Only a deny (or `{}`, or a failure) stops a call, so a released or timed-out request that answers anything else lets the call run.
- **`--mode accept-edits`:** `write_file` ran with no prompt under both the empty answer and `force_ask`.
- **`--mode auto`:** on this machine, `echo` prompted under both the empty answer and `force_ask`.
- **`dangerouslySkipPermissions: true` in `settings.json` did nothing in 1.4.7.** agy still prompted. A scan of the binary finds no lowercase `dangerouslySkipPermissions` key, only the flag `--dangerously-skip-permissions`. The changelog speaks of permission modes in `/config` (such as `always-proceed`). The key was in the real settings on 2026-10-15 under an older agy (`docs/research/2026-10-15-local-install-check.md`).
- **PreToolUse fires before every tool, in every mode tried** (default, `auto`, `accept-edits`, the skip flag), and before reads.
- **A hook may hold a call for a long time.** A 45 s hold (timeout 120) ran the command when it ended; the long-hold result is below. During a hold the status line says `agent_state: "working"` and `tool_confirmation_pending` stays unset.
- No `PostToolUse` fired after a call the hook denied.

### A4. Environment and two tabs: confirmed

- `MARSHELL_TAB_ID` and `MARSHELL_HOME` reach the hook process.
- `ANTIGRAVITY_CONVERSATION_ID` is in its environment too, equal to the body's `conversationId`.
- Two agy tabs at once (02:07:48 and 02:07:54) were two `agy.exe` processes. Each reported its own tab id (`t52481f6a645e`, `tb50ecdca78fe`) and its own conversation; agy shares no daemon between them.

### A5. Payloads: confirmed, with corrections

- Keys are camelCase, and **no body names its event**, so the bridge's `--event` is needed.
- The common fields are `conversationId`, `workspacePaths`, `transcriptPath`, `artifactDirectoryPath`, `modelName` (`gemini-3.8-flash-high`) and `lastUserInput` (the 1.4.7 field, present in every hook). Every event's body carries all of them.
- **`workspacePaths` was `[]` in every body**, even in a trusted workspace. The project folder comes from the status line's `workspace.project_dir` (or the tab's cwd), not from `workspacePaths[0]`.
- **`transcriptPath` names `.../brain/<id>/.system_generated/logs/transcript_full.jsonl`**; `transcript.jsonl` sits beside it.
- `PreToolUse` adds `stepIdx` and `toolCall { name, args }`.
- `PostToolUse` adds the same, plus `error` (`""` on success).
- `PreInvocation` and `PostInvocation` add `invocationNum` and `initialNumSteps`.
- `Stop` adds `executionNum`, `terminationReason` (`NO_TOOL_CALL` seen), `error` and `fullyIdle` (`true`).

The tools and their argument keys:

| What | `toolCall.name` | `args` |
| --- | --- | --- |
| Shell command | `run_command` | `CommandLine`, `WaitMsBeforeAsync`, `toolSummary` |
| New file | `write_file` | `Path`, `Content` |
| Edit | `edit_file` | `Path`, `Target`, `Replacement` |
| Read, inside or outside the workspace | `read_file` (not `view_file`) | `Path` |
| URL | No URL tool was offered in this setup. The agent ran `curl.exe -s https://example.com` through `run_command`. agy's changelog says URL reads now ask first. | |

agy prompts in default mode for `run_command`, `write_file`, `edit_file` and a `read_file` outside the workspace, but not for a `read_file` inside it.

### A6. The status line: corrected

The status line is `"statusLine": { "type": "command", "command": "<string>" }`, with optional `stack_with_default`. Its stdin keys are:

- `cwd`, `session_id` and `conversation_id` (equal), `transcript_path`
- `model { id, display_name, effort }`
- `workspace { current_dir, project_dir }`, `version` (`1.4.7`)
- `context_window { total_input_tokens, total_output_tokens, context_window_size, used_percentage, remaining_percentage, current_usage {...} }`, `exceeds_200k_tokens`
- `product` (`antigravity`), `quota`, `agent_state`, `sandbox { enabled }`, `plan_tier`, `email`
- `tool_confirmation_pending` (present only while agy shows a prompt), `cycle_mode` (present only in `--mode accept-edits`: `"accept-edits"`), `terminal_width`

Notes on those keys:

- **`quota` has two weekly buckets**, `"3p-weekly"` and `"gemini-weekly"`, each with `remaining_fraction`, `reset_time` and `reset_in_seconds` (about 604800). There is no five-hour window.
- **No key names the mode** in default, `auto` or the skip flag. `pending_input_count` and `execution_mode` were never present.
- **`transcript_path` points to `.gemini\antigravity\brain\...\transcript.jsonl`, a folder that doesn't exist.** The real files are under `.gemini\antigravity-cli\brain\`. Use the hook's `transcriptPath`.
- `email` and the hooks' `lastUserInput` are personal data. The fixtures are scrubbed, and the core should keep neither.
- `agent_state` values seen: `authenticating`, `initializing`, `working`, `tool_use`, `idle`.

Cadence: agy runs the command only when something changes. That was every 0.2 to 0.5 s while working, then nothing at all: 50 s passed with no run while agy's own prompt waited, and none while idle. **`tool_confirmation_pending` turned true about 0.8 s after the `PreToolUse` that raised agy's prompt** (01:52:03.838 to 01:52:04.608). It cleared once the prompt was answered.

Output:

- Printed text shows on its own row under the prompt box (`MSHL-STATUS-TEXT`).
- Empty output without `stack_with_default` leaves that row blank, and agy's own line ("? for shortcuts … Gemini 3.8 Flash · high") is gone.
- Empty output with `"stack_with_default": true` keeps agy's own line.
- A failing command shows a "⚠ Statusline Error" block in the conversation, every time it runs.

### A7. `permissionOverrides`: corrected

On an `allow`, `permissionOverrides` granted nothing: agy prompted anyway (A3). Nothing was written to `settings.json` or `config/`. Choosing agy's own "Yes, and always allow ... (Persist to settings.json)" wrote `"permissions": { "allow": ["command(echo case-override)"] }` into `antigravity-cli/settings.json`. That is the same grammar as the override that did nothing. agy also writes `trustedWorkspaces` into that file itself.

### A8. Resume and the transcript: confirmed, with a correction

- On `/quit`, agy prints "Resume with -c (or command below):" and `agy --conversation=<id>`.
- `agy --conversation <id>` (space form) in a new process resumed the conversation: it answered with the first command run in it. Its `PreInvocation` carried the old `conversationId` with `initialNumSteps: 59`, and no `SessionStart` fired.
- The transcript files (`transcript.jsonl`, `transcript_full.jsonl` and `chunks/`) exist from the first prompt (created 01:51:58, with `SessionStart`).

### A9. `--version`: confirmed

`agy --version` prints the bare version, `1.4.7`.

## Long hold

With `timeout: 600` and the recorder holding a `PreToolUse` for 200 s under `--dangerously-skip-permissions`, agy waited the whole time and then ran the command: `PreToolUse` at 02:20:51.070, `PostToolUse` at 02:24:12.112. While the hook held, agy's screen read "Running (echo long-hold)" and "Running command...". So a card can be held for several minutes, and the 600 s timeout of the Claude and Codex permission entries carries over.

## The real `~/.gemini`

The newest modified time under the real `~/.gemini` was 2026-10-26 01:24:30 (`antigravity-cli\cache\CHANGELOG.md`) both before the check and after it (02:27). `~/.claude/settings.json` kept its modified time (2026-10-25 14:04:04). `live-check.ps1 stop` stopped all 23 recorded processes by PID and deleted the two copied sign-in files. One Claude session started by the check's first app instance was left running when `tauri dev` rebuilt the app; it was identified by its start time, which matched the scratch `CLAUDE_CONFIG_DIR`'s creation, and stopped by PID.
