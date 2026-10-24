# Codex CLI hooks: what Codex 0.163 runs, trusts and sends (slice 6a, Task 1)

- Date: 2026-10-24, 18:38 to 19:47 local time (UTC+3), across one dev-app run restarted by `tauri dev` rebuilds.
- Codex: `codex-cli 0.163.0` (`C:\Users\<user>\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe`). PowerShell 7.6.8 runs the hook commands.
- Setup: `live-check.ps1 start -CodexAuth` with scratch `lc-6a-3`. Codex ran in Marshell tabs with `CODEX_HOME` = `<scratch>\cx`, opened as custom argv `codex --no-daemon -s read-only -a on-request` in `<scratch>\proj`. Every prompt and every Codex question was typed or answered by this check over CDP.
- Recording: each entry ran the bridge's `record` role, which saves its stdin under `<MARSHELL_HOME>\fixtures\codex\<Event>[-n].json` and posts it to the core. The fixtures in `crates/core/tests/fixtures/codex/hooks/0.163.0/` are copies of those files, scrubbed with `pnpm scrub:fixtures`.
- Real Codex home: `~/.codex/config.toml` last modified 18:35:18, before the first scratch folder of this slice (18:38:05), and unchanged at the end; it is 16 s after the user's ChatGPT Codex app started (18:35:02). `~/.codex/hooks.json` absent before and after. The only file read from `~/.codex` was `auth.json`, copied into the scratch home and deleted by `stop`.

## The hooks.json that ran

One group per event, no `matcher`, one entry each. `<bridge>` is `<scratch>/mh/bin/marshell-hook.exe` with forward slashes, and the path holds a space.

```json
{
  "hooks": {
    "SessionStart":     [{ "hooks": [{ "type": "command", "command": "& \"<bridge>\" record --agent codex --tag mshl1", "timeout": 10 }] }],
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "& \"<bridge>\" record --agent codex --tag mshl1", "timeout": 10 }] }],
    "PreToolUse":       [{ "hooks": [{ "type": "command", "command": "& \"<bridge>\" record --agent codex --tag mshl1", "timeout": 10 }] }],
    "PermissionRequest": [
      { "hooks": [{ "type": "command", "command": "& \"<bridge>\" record --agent codex --tag mshl1", "timeout": 10 }] },
      { "hooks": [{ "type": "command", "command": "& \"<scratch>/pr.cmd\"", "timeout": 10 }] }
    ],
    "PostToolUse":      [{ "hooks": [{ "type": "command", "command": "& \"<bridge>\" record --agent codex --tag mshl1", "timeout": 10 }] }],
    "SubagentStart":    [{ "hooks": [{ "type": "command", "command": "& \"<bridge>\" record --agent codex --tag mshl1", "timeout": 10 }] }],
    "SubagentStop":     [{ "hooks": [{ "type": "command", "command": "& \"<bridge>\" record --agent codex --tag mshl1", "timeout": 10 }] }],
    "Stop":             [{ "hooks": [{ "type": "command", "command": "& \"<bridge>\" record --agent codex --tag mshl1", "timeout": 10 }] }],
    "Interrupt":        [{ "hooks": [{ "type": "command", "command": "& \"<bridge>\" record --agent codex --tag mshl1", "timeout": 3 }] }],
    "PreCompact":       [{ "hooks": [{ "type": "command", "command": "& \"<bridge>\" record --agent codex --tag mshl1", "timeout": 10 }] }],
    "PostCompact":      [{ "hooks": [{ "type": "command", "command": "& \"<bridge>\" record --agent codex --tag mshl1", "timeout": 10 }] }],
    "SessionEnd":       [{ "hooks": [{ "type": "command", "command": "& \"<bridge>\" record --agent codex --tag mshl1", "timeout": 3 }] }]
  }
}
```

`<scratch>/pr.cmd` is a two-line stub, `@echo off` and `type "<scratch>\pr-out.txt"`, so the A6 answers changed by rewriting `pr-out.txt` without touching `hooks.json` or its trust.

## Answers

| | Assumption | Finding |
| --- | --- | --- |
| A1 | The `hooks.json` shape, `matcher` optional | Confirmed. The file above loaded with no matcher on any group, and every event fired except `SubagentStart`, `SubagentStop`, `PreCompact` and `PostCompact`, which this check didn't provoke. An empty group `{"hooks": []}` loads without a warning (see A3). |
| A2 | `"<path>" event ...` runs on Windows | **Corrected.** Codex runs the command through PowerShell, where a quoted path followed by arguments is a parse error. `& "<path>" <role> --agent codex --tag mshl1` runs, with a space in the path. |
| A3 | Hooks run only once trusted | Confirmed, and trust is per entry and bound to a hash. See "Trust" below. |
| A4 | The session's environment reaches the hook | Confirmed. `MARSHELL_HOME` and `MARSHELL_TAB_ID` reach the bridge: the `record` role wrote under the scratch `MARSHELL_HOME`, and the core received its posts with the tab. |
| A5 | Payload fields | Confirmed with corrections: the shell tool is named `Bash`, `apply_patch` carries the patch in `tool_input.command`, and some fields are missing on some events. See "Payloads" below. |
| A6 | Allow, deny and empty stdout | Confirmed. See "Answering a PermissionRequest" below. |

## Trust

- Codex keeps trust in the Codex home's `config.toml`, one table per entry: `[hooks.state.'<hooks.json path>:<event_snake>:<group>:<handler>']` with `trusted_hash = "sha256:<hex>"`. The path is Codex's own spelling (backslashes), and `<group>` and `<handler>` are raw array positions.
- At start, Codex counts the entries whose key is missing or whose hash differs and shows "Hooks need review / N hooks are new or changed / Hooks can run outside the sandbox after you trust them", with **Review hooks**, **Trust all and continue** and **Continue without trusting (hooks won't run)**. It asks before any prompt and needs no model turn.
- Adding one entry flags that entry only ("1 hook is new or changed"). Removing entries asks nothing, and the removed entries' keys stay in `config.toml`.
- **Positions count.** With PermissionRequest group 0 (the `record` entry) deleted, the stub moved from `:1:0` to `:0:0`, and Codex flagged it as new, though its own entry was unchanged. So removing a group shifts every later group of the same event and un-trusts it. With an empty group `{"hooks": []}` inserted before `Stop`'s only group, the `record` entry moved to `:1:0` and was flagged; after trusting, `stop:1:0` held the same hash as `stop:0:0`. Codex counts empty groups, so an empty placeholder would keep later groups at their trusted positions.
- The hash recipe in plan Task 3b reproduced all 12 trusted entries of the first run. Two vectors with a neutral path, `& "C:/Users/someone/.marshell/bin/marshell-hook.exe" event --agent codex --tag mshl1`, were added as group 1 and trusted with **Trust all and continue**:

| Event | Group | Timeout | `trusted_hash` |
| --- | --- | --- | --- |
| `Stop` | 1 | 5 | `sha256:eae810b59ee8e55e5a7a3169f12f3216e3be79d801f5259f1259fec5c558ab56` |
| `SessionEnd` | 1 | 2 | `sha256:6da5391ccb8c4e99a5fb910f7bc2b0dec6a6b0c9ac476f61d69c1cdb0ae97131` |

  Both equal the vectors in `crates/core/src/agents/codex/trust.rs` (`hash_matches_codex_for_the_recorded_vectors`), which were computed from the recipe before this run.

## When the events fire

- **SessionStart fires with the first prompt, not at launch.** In three sessions it arrived in the same second as the first `UserPromptSubmit` (18:46:06 with "say ok" at 18:46:05; 18:47:08; 19:36:08). A Codex started and quit with no prompt fired `SessionEnd` only, yet left a 22 KB rollout in `sessions/`. The rollout's name is in `transcript_path` on every event, `SessionEnd` included.
- **Sandbox retry.** In the read-only sandbox, the Windows sandbox couldn't start any process in the scratch home (`CreateProcessAsUserW failed: 5`). Codex then retried the same command with escalation. That gave two `PreToolUse` with different `tool_use_id`s, a `PermissionRequest` only before the second, and a `PostToolUse` only for the second. When the model asked for escalation up front, there was one `PreToolUse`.
- **Allowed or answered Yes:** `PreToolUse`, `PermissionRequest`, `PostToolUse`, `Stop`.
- **Denied by the hook:** `PreToolUse`, `PermissionRequest`, then `Stop`, with no `PostToolUse`. Codex prints "Blocked by hook" and the deny message.
- **Interrupt (Esc during a command):** `Interrupt`, and no `PostToolUse` or `Stop` after it. Codex prints "Conversation interrupted".
- **Quit (`/quit`):** `SessionEnd` with `reason: "other"`.
- Earlier in the run, a turn that ended in a usage-limit error fired no `Stop`, and Codex has no StopFailure-like event.

## Payloads

Every event carries `session_id`, `transcript_path` (the rollout), `cwd` and `hook_event_name`. Others vary:

| Event | Further fields |
| --- | --- |
| `SessionStart` | `model`, `permission_mode`, `source: "startup"`. No `turn_id`. |
| `UserPromptSubmit` | `turn_id`, `model`, `permission_mode`, `prompt`. |
| `PreToolUse` | `turn_id`, `model`, `permission_mode`, `tool_name`, `tool_input`, `tool_use_id` (`exec-<uuid>`). |
| `PermissionRequest` | `turn_id`, `model`, `permission_mode`, `tool_name`, `tool_input`. **No `tool_use_id`.** For `Bash`, `tool_input.description` holds Codex's reason ("Allow running the requested command to write out.txt?"); `apply_patch` has none. |
| `PostToolUse` | as `PreToolUse`, plus `tool_response`, a string (`""` for the shell; "Exit code: 0\nWall time: …\nOutput:\nSuccess. Updated the following files:\nA notes.md\n" for `apply_patch`). |
| `Stop` | `turn_id`, `model`, `permission_mode`, `stop_hook_active: false`, `last_assistant_message`: the final reply, or **`null`** when the turn ended without one. |
| `Interrupt` | `turn_id`, `model`, `permission_mode`. |
| `SessionEnd` | `reason`. **No `model` or `permission_mode`.** |

Tools seen:

- Shell: `tool_name: "Bash"` (on Windows too, where it ran PowerShell), `tool_input: {"command": "Set-Content out.txt hi"}`.
- Edit: `tool_name: "apply_patch"`, `tool_input: {"command": "*** Begin Patch\n*** Add File: notes.md\n+hello\n*** End Patch"}`. The file is not a field. It is on the `*** Add File:`, `*** Update File:` or `*** Delete File:` line.
- No MCP tool ran, so its `tool_name` format is not recorded.
- `permission_mode` was `"default"` in every event, under `-s read-only -a on-request`. The other documented values were not seen.
- `model` follows the session's model: it changed from `gpt-6.1-sol` to `gpt-6-luna` after Codex's rate-limit prompt switched it mid-session.

## Answering a PermissionRequest

- Empty stdout: Codex shows its own "Would you like to run the following command?" prompt.
- `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}`: the command ran with no prompt (`allowed.txt` written).
- `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Denied by the Marshell spike stub."}}}`: the command didn't run. Codex printed "Blocked by hook / Denied by the Marshell spike stub.", and the model's reply was "Approval was denied, so `Set-Content denied.txt no` did not run."
- Timeout: a stub that slept 15 s against its 10 s timeout and then printed the allow JSON gave Codex's own prompt about 10 s after the request, and the late allow was ignored. Marshell's PermissionRequest entry has a 600 s timeout, so a held approval answers within it, or Codex falls back to its own prompt.
- When two groups answer the same event, an empty answer from one (the `record` entry) leaves the other's decision in force.

## Running Codex in a live check

- `-a untrusted` no longer exists: `--ask-for-approval` takes `on-request` or `never`. Marshell's Manual mode already passes `--sandbox read-only --ask-for-approval on-request`.
- With `CODEX_HOME` inside the session scratchpad, plain `codex` quits at once with "path must be shorter than SUN_LEN": its app-server socket is `<CODEX_HOME>\app-server-control\app-server-control.sock`, 43 characters longer than `CODEX_HOME`. `--no-daemon` avoids it, and has no environment or config equivalent. A tab opened from a Codex profile can't add the flag, so a live check that needs `agent: "codex"` tabs needs a `CODEX_HOME` of about 60 characters or fewer.
- Codex warns that it won't create PATH helper binaries under a temporary folder. The warning is harmless here.
- In the scratch home every sandboxed command fails and escalates, so a shell write reliably gives a `PermissionRequest`.

## Fixtures

`crates/core/tests/fixtures/codex/hooks/0.163.0/`, all from one session (scrubbed id `…0001`): `SessionStart.json`, `UserPromptSubmit.json`, `PreToolUse-shell.json`, `PreToolUse-apply_patch.json`, `PermissionRequest-shell.json`, `PermissionRequest-apply_patch.json`, `PostToolUse.json` (shell), `PostToolUse-apply_patch.json`, `Stop.json` (`last_assistant_message: null`), `Stop-recap.json` (with a reply), `Interrupt.json`, `SessionEnd.json`. `cargo test -p marshell-core -j 4 --test fixtures_scrubbed` passes.

## Open

- `SubagentStart`, `SubagentStop`, `PreCompact` and `PostCompact` payloads were not recorded.
- The MCP tool name format.
- `permission_mode` values other than `"default"`.
