# Qwen Code hooks, status line and payloads (slice 6l1 spike)

- Date: 2026-11-03, two parts. A headless run (`qwen -p`) from a plain shell, then an interactive run in Marshell's dev app from about 11:39 to 11:49 local time.
- Qwen Code: 0.26.5, the standalone install in `%LOCALAPPDATA%\qwen-code` (`qwen.cmd`, its own `node.exe`).
- Build: `phase-6l1` at 0030bd9 under `pnpm tauri dev` on Windows 11 Pro (10.0.26300), started with `live-check.ps1 start -Scratch %TEMP%\lc-6l1-2 -QwenFake`. The first try, `lc-6l1-1`, was stopped during its Rust build because a script bug left `-QwenFake` off.
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `USERPROFILE`, so Qwen's home was `lc-6l1-2\home\.qwen`. Onboarding was closed with "Skip: use limited status" for Claude Code, Codex and Antigravity.
- Model: `fake-openai.mjs` (slice 6l1 decision 8) on `127.0.0.1:47612`, logging every request to `qwen-fake.jsonl`. No account and no real model were used.
- How it was driven: two shell tabs made through the scratch core's API (`POST /v1/sessions`), with `qwen` typed in through `/v1/sessions/{tab}/input`. Enter, Escape and Tab went in as CDP keys to the shown terminal. The screen was read through `/v1/sessions/{tab}/peek`. No screenshots, and no desktop input.
- Recorders:
  - Every event ran `marshell-hook.exe record --agent qwen --tag mshl1`, a copy of the dev bridge in `lc-6l1-2\rec dir & x\`, which saves each body to `mh\fixtures\qwen`.
  - `SessionStart` ran a second copy from `lc-6l1-2\pct%x\`.
  - `UserPromptSubmit`, `PermissionRequest` and `ui.statusLine` also ran `answer.mjs`. It saves stdin and env, then prints `answer\<Event>.txt` after `answer\<Event>.sleep` ms, with the exit code in `answer\<Event>.exit`.

The `settings.json` that ran (paths shortened to `<S>` = `C:/Users/Home Office/AppData/Local/Temp/lc-6l1-2`, and `<s>` = its 8.3 form `C:/Users/HOMEOF~1/AppData/Local/Temp/lc-6l1-2`):

```json
{
  "security": { "auth": { "selectedType": "openai" } },
  "model": { "name": "marshell-fake" },
  "ui": { "statusLine": { "type": "command", "command": "node <s>/answer.mjs StatusLine" } },
  "hooks": {
    "SessionStart": [
      { "hooks": [{ "type": "command", "command": "C:\"/<S minus C:>/rec dir & x\"/marshell-hook.exe record --agent qwen --tag mshl1", "timeout": 5 }] },
      { "hooks": [{ "type": "command", "command": "C:\"/<S minus C:>/pct%x\"/marshell-hook.exe record --agent qwen --tag mshl1", "timeout": 5 }] }
    ],
    "UserPromptSubmit": [ <bridge, 5>, { "hooks": [{ "type": "command", "command": "node <s>/answer.mjs UserPromptSubmit", "timeout": 5 }] } ],
    "PreToolUse": [ { "matcher": ".*", <bridge, 5> } ],
    "PostToolUse": [ { "matcher": ".*", <bridge, 5> } ],
    "PostToolUseFailure": [ { "matcher": ".*", <bridge, 5> } ],
    "PermissionRequest": [ { "matcher": ".*", <bridge, 5> }, { "matcher": ".*", "hooks": [{ "type": "command", "command": "node <s>/answer.mjs PermissionRequest", "timeout": 600 }] } ],
    "Notification": [ <bridge, 5> ], "Stop": [ <bridge, 5> ], "StopFailure": [ <bridge, 5> ], "SessionEnd": [ <bridge, 2> ]
  },
  "$version": 4
}
```

Midway, the second tab was started after `general.enableAutoUpdate: false`, `tools.approvalMode: "yolo"` and a `PermissionRequest` answerer timeout of 5 were added.

## Answers

| Question | Finding |
| --- | --- |
| Q1. Shape | Confirmed. Groups with no `matcher` work for `SessionStart`, `UserPromptSubmit`, `Notification`, `Stop` and `SessionEnd`. Two groups in one event both run, in parallel. Qwen adds `"$version": 4` to the user `settings.json` on its first run and keeps the rest of the file. |
| Q2. The shell | **The plain form fails.** `"C:/folder with space/marshell-hook.exe" record ...` gave `'\"C:/...exe\"' is not recognized`, because Qwen escapes `"` to `\"` before `%ComSpec% /d /s /c`. That failure is non-blocking: Qwen carried on as with no hook. Antigravity's form works, with a space and an `&` in the folder: `` C:"/Users/Home Office/.../rec dir & x"/marshell-hook.exe record --agent qwen --tag mshl1 ``. A quoted argument after the program breaks the same way, so the bridge's arguments must stay unquoted. An unpaired `%` in the folder (`pct%x`) also ran. A `%name%` pair would be expanded by cmd, so the `%` refusal stays. |
| Q3. Bodies | Every body carries `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `timestamp` and `permission_mode`, and every body but `SessionStart` and `SessionEnd` adds `prompt_id`. `UserPromptSubmit` adds `prompt` and also `submitted_prompt` (the same text), so both must be stripped. When Qwen sends a tool result back to the model, it fires a second `UserPromptSubmit` with `prompt: ""` and no `submitted_prompt`. `PreToolUse` adds `tool_name`, `tool_input`, `tool_use_id` and `tool_call_id`. `PermissionRequest` adds `tool_name` and `tool_input` only, with **no tool id**. `PostToolUse` adds `tool_response` and `duration_ms`. Tool ids: `run_shell_command` (`command`, `is_background`, `description`), `write_file` (`file_path`, `content`), `edit` (`file_path`, `old_string`, `new_string`), `read_file` (`file_path`). `Notification` adds `notification_type`, `title` and `message`. `Stop` adds `stop_hook_active`, `last_assistant_message`, `background_tasks`, `crons`, `context_usage` (0.005), `context_limit` (200000) and `input_tokens`. `SessionStart` adds `source` (`startup`, or `resume` after `--resume`) and `model`. `SessionEnd` adds `reason` (`prompt_input_exit` after `/quit`). The env reaching the hook holds `MARSHELL_TAB_ID`, `MARSHELL_HOME` and `QWEN_CODE_SESSION_ID`. Two tabs reported their own tab ids (`tf71c6a295517`, `ted852e7d61b0`) and session ids. |
| Q3. Modes | `permission_mode` read `default`, `auto_edit`, `auto`, `yolo` and `plan`, in that order through Tab's cycle (footer: "Ask permissions", "auto-accept edits", "Auto mode", "YOLO mode", "plan mode"). `PermissionRequest` was seen only in `default`, so its `auto-edit` spelling wasn't seen. `mode_of` takes both spellings. |
| Q3. Notification timing | `idle_prompt` fires **during a turn**: 10 to 20 ms after each `PreToolUse`, before the tool runs. It fires again after `Stop`. `permission_prompt` fires only when Qwen shows its own dialog. When the person picks "No" (Escape) in that dialog, there is no `PostToolUse` and no `Stop`; the next event is an `idle_prompt` about 8 s later. |
| Q4. `PermissionRequest` | It fired before each dialog. `echo marshell` ran in `default` with no request, so read-only shell commands never ask. Qwen waits for the hook: a 4 s delayed allow ran the write with no dialog. Answer matrix for `write_file`: `allow` ran it with no dialog. `deny` with `message` refused it, and the screen showed "Denied in Marshell: MSHL-DENY-7". Empty stdout with exit 0, exit 1 and exit 2 each showed Qwen's own dialog. A hook past its `timeout` (5 s, the answerer sleeping 9 s) showed the dialog 5.2 s after the request. Not checked live: the other modes and `updatedPermissions`. The bundle merges `updatedPermissions` from hook output, but decision 5 refuses always-allow, so Marshell never sends it. |
| Q5. Neutral answers | Confirmed. The recorder prints nothing and exits 0 on every event, and Qwen ran as with no hook. |
| Q6. Context | Confirmed. `additionalContext` reached the model as a second text part, `<qwen:user-prompt-submit-context>\nMARSHELL-NOTE-77\n</qwen:user-prompt-submit-context>`, once per prompt. The tool-result continuation didn't add it again. |
| Q7. Status line | `ui.statusLine` `{"type":"command","command":...}` runs through cmd with the same escaping as hooks. It runs on Qwen's own events, not on a timer: at start, at each prompt, and after each model reply (24 runs in two tabs over 6 minutes). Its stdin holds `session_id`, `version`, `model.display_name`, `context_window` (`context_window_size`, `used_percentage`, `remaining_percentage`, `current_usage`, `total_input_tokens`, `total_output_tokens`), `workspace.current_dir` and `metrics` (`models`, `files.total_lines_added`, `files.total_lines_removed`). There is no mode in it. With the fake model, `context_window_size` and `used_percentage` stayed 0 while `Stop`'s `context_usage` read 0.005, so the fold takes context % from `Stop` and reads a size of 0 as unknown. `type: "preset"` exists in 0.26.5: the settings schema says "Use `type: "preset"` with built-in item ids, or `type: "command"`". The env reaching it holds `MARSHELL_TAB_ID` and `QWEN_CODE_SESSION_ID`. |
| Q8. Sessions | Confirmed. `/quit` printed "To continue this session, run `qwen --resume c9d73051-…`". That command in the same tab reopened the transcript with `SessionStart` `source: "resume"` and the same `session_id`. `transcript_path` is `<home>\.qwen\projects\c--users-home-office-appdata-local-temp-lc-6l1-2-proj6\chats\<session_id>.jsonl`. Not checked: `--continue` with no session, and when the file first exists. |
| Q9. Version | `qwen --version` prints `0.26.5`. |
| Q10. Process chain | Typed into a pwsh tab: `pwsh → cmd.exe /c ""%LOCALAPPDATA%\qwen-code\bin\qwen.cmd" <args>" → node.exe "...\qwen-code\bin\..\lib\cli-entry.js" <args> → node.exe --expose-gc "...\qwen-code\lib\cli.js" <args> → node.exe --expose-gc "...\lib\cli.js" <args>`. Every node is `%LOCALAPPDATA%\qwen-code\qwen-code\node\node.exe`. Not checked: a Qwen started from the launcher, and the foreground reader's report. |
| Q11. Mode flag over setting | Confirmed. With `tools.approvalMode: "yolo"` in the user settings, `qwen --approval-mode default` showed "Ask permissions" and asked for a write. `qwen --resume <id>` with no flag started in "YOLO mode" (`permission_mode: "yolo"`). A Marshell launch always passes the flag, so `skips_all` returns None. |
| Q12. Fake model | Confirmed. The minimal set is `security.auth.selectedType: "openai"` and `model.name` in settings, with `OPENAI_API_KEY`, `OPENAI_BASE_URL` and `OPENAI_MODEL` in the env. Qwen streams (SSE). Besides each prompt, it sends its own requests: a "suggestion mode" request after each reply, and a memory-extraction request. |

## Other findings

1. **Qwen updates itself on exit.** The second tab printed "Qwen Code update available! 0.26.5 → 0.26.6. The update will be installed after you exit this session." The `cli.js` child asks its parent `cli.js` (`UPDATE_ON_EXIT`) to install the update when the child exits. That would change the person's real install from inside a check. That tab's Qwen was ended top-down by PID (`cli-entry.js` first; both `cli.js` processes then exited on their own). The install stayed at 0.26.5, with no file under `%LOCALAPPDATA%\qwen-code` changed. `live-check.ps1 -QwenFake` now writes `general.enableAutoUpdate: false` and sets `QWEN_CODE_SKIP_UPDATE_CHECK_ONCE=true`. The first tab's update check had timed out ("Update check skipped (registry did not respond within 5s)"), so its `/quit` was safe.
2. **`live-check.ps1 stop` didn't know Qwen.** Qwen runs as `node.exe`, so a Qwen the person started in the dev window would have been stopped without a question. The tree now marks the topmost `node.exe` whose command line holds `qwen-code` as an agent. With a Qwen running, `stop` refused and named only `node.exe pid 39476` (its `cli-entry.js`). `-AlsoStop 39476` then stopped the tree.
3. **The fixture scrubber missed Qwen's project folder.** Qwen names it after the lowercased path with every other character a hyphen (`c--users-home-office-…`). `scripts/scrub-fixtures.mjs` now replaces that spelling, tested first in `scrub-fixtures.test.ts`.
4. **Relative paths fail in Qwen's file tools.** `write_file` with `b.txt` failed "File path must be absolute: b.txt", with a `PreToolUse` and no `PostToolUse` or `PostToolUseFailure`. The headless `edit` probably failed the same way. The fake model's prompts use absolute paths.
5. **A feedback prompt** ("How is Qwen doing this session? (optional) 1: Good 2: Bad 3: Fine 0: Dismiss") appeared above the input after a few turns. The text typed after it was taken as a prompt.

## Fixtures

`crates/core/tests/fixtures/qwen/hooks/0.26.5/` and `statusline/0.26.5/`, scrubbed with `pnpm scrub:fixtures crates/core/tests/fixtures/qwen` (21 files, 4 ids). `cargo test -p marshell-core -j 4 --test fixtures_scrubbed` passed.

- `PreToolUse-edit.json` and `PreToolUse-read_file.json` come from the headless run in `yolo`, and the rest from the interactive run.
- `UserPromptSubmit-<mode>.json` are the answerer's copies of the same prompt in each mode.
- No `PermissionRequest` for `run_shell_command` or `edit` was recorded. The `write_file` one is the only real body, and the others differ only in `tool_name` and `tool_input`, as in `PreToolUse`.

## Clean-up checks

- The real `~/.qwen` didn't exist before or after either run.
- The real `~/.claude/settings.json` kept its modified time, 2026-10-25T14:04:04.
- The Qwen install stayed at 0.26.5.
- The check was stopped by PID, along with the fake model (pid 38528) and the headless run's fake model. The scratch folders `%TEMP%\lc-6l1-1`, `%TEMP%\lc-6l1-2` and `%TEMP%\mshl-6l1-spike` are still in place.
