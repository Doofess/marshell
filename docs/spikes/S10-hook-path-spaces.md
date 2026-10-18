# S10: Windows hook path with spaces

Run 2026-10-18 on the dev box (Windows 11 Pro 26300) against Claude Code 2.1.302 (Opus 5.5), from a session
started in Marshell (`pnpm tauri dev`) in the scratch project `C:\dev\spike-3a`. `MARSHELL_HOME` was a scratch
folder under `C:\Users\Home Office\AppData\Local\Temp`, so every bridge path held the space.

Hooks came from `C:\dev\spike-3a\.claude\settings.local.json`: for every event, one shell-form entry
(`"<home with forward slashes>/bin/marshell-hook.exe" record --agent claude --tag mshl1`) and one exec-form entry
(the bare path, with `args: ["event", "--agent", "claude", "--tag", "mshl1"]`).

| Check | Pass criterion | Result |
|---|---|---|
| Shell form, quoted path with a space | The bridge runs and the core gets the event | **Pass.** Every fired event arrived with role `record`. |
| Exec form, bare path with a space | The bridge runs and the core gets the event | **Pass.** Every fired event arrived with role `event`. |
| Status line, quoted path with a space | The bridge runs as the status line | **Pass** (see S4). |
| Failures | `logs/bridge.log` stays empty | **Pass.** No `bridge.log` was written. |

Events seen from both forms: SessionStart, InstructionsLoaded, UserPromptSubmit, PreToolUse (Bash), PermissionRequest,
PostToolUse, PostToolBatch, MessageDisplay, Notification, Stop and SessionEnd.

## Findings

1. **Both forms work; the two copies of an event arrive 2–35 ms apart.**
   - Timing: a hand run of the bridge in the `event` role took 12–70 ms, median about 19 ms, end to end, including process start.
   - Which form 3c installs: the exec form. It needs no quoting and no shell, so the shell Claude picks can't change it.
2. **No trust prompt.** A project-local `settings.local.json` hook ran at SessionStart without a folder-trust question
   in this setup. 3c still shows its own consent card before it writes any hook.
3. **Hook to core latency against the transcript was not measured.** The session ran with transcript saving off.
   - Why: the dev app was started from a Claude Code session, so the child inherited `CLAUDE_CODE_CHILD_SESSION`, and Claude printed "Transcript saving is off — inherited CLAUDE_CODE_CHILD_SESSION marker".
   - **Follow-up for 3c:** Marshell should drop `CLAUDE_CODE_*` variables from a session's environment, so an app started from Claude Code doesn't pass the marker on.
4. **The status line floods the event ring.** With `refreshInterval: 1` a status line post arrives every second.
   After about three minutes, 150 of 166 entries in the 200-entry ring were status lines, so hook events fall out.
   **Follow-up for 3c:** keep only the latest status line per tab, or give it its own ring.

Decision: [x] exec form for 3c's install  [ ] shell form
