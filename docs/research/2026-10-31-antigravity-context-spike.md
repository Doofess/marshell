# Antigravity context spike (slice 6i2, Task 4)

- Date: 2026-10-31, 11:50 to 12:03 local time (UTC+2).
- Build: `phase-6i2` at `b24b039` under `pnpm tauri dev` on Windows 11 Pro (26300), started by `live-check.ps1 start -AgyAuth` with a scratch `USERPROFILE`, `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR` and `CODEX_HOME`. agy 1.4.11, Gemini 3.8 Flash (High).
- Onboarding: every CLI's consent was skipped, so Marshell installed no hooks. The only hook was the spike's.
- How it was driven: CDP keys into a PowerShell 7 tab in the scratch project, where `agy` was typed. There were 7 screenshots and no desktop input. agy's first-run screens were answered: the "help improve" data-sharing box was unticked, and the empty scratch folder was trusted. Two read-only commands (`Get-ChildItem`, `Get-Date`) were approved once each in agy's own prompt.
- The question: can a `PreInvocation` hook add a note the model sees, and how often must it be sent per prompt? (`2026-10-31-prompt-context-injection.md`, "Unknown, for the 6i2 spike")

## Setup

`<home>\.gemini\config\hooks.json`:

```json
{ "marshell-spike": { "enabled": true,
  "PreInvocation": [{ "type": "command", "command": "pwsh -NoProfile -File C:/dev/.../spike/spike.ps1", "timeout": 10 }] } }
```

The script appends a timestamp and its stdin to `spike.log`, then prints `{"injectSteps":[{"ephemeralMessage":"Spike fact: the codeword is TANGERINE-42."}]}`. It was then changed for prompt 3:
- It sleeps 2 s first.
- On `invocationNum` 0 only, it injects a new codeword, `PLUM-77`.
- On later invocations, it prints nothing.

## Results

| Check | Result |
| --- | --- |
| The model sees an `ephemeralMessage` | Yes. Prompt 1, "What is the codeword? Answer in one line.", was answered "The codeword is TANGERINE-42." |
| `PreInvocation` calls per prompt | One per model call. Prompt 1 had no tool step, and made one call (`invocationNum` 0, `initialNumSteps` 1). Prompt 2 ran one tool step and made two calls (`invocationNum` 0 and 1, `initialNumSteps` 4 and 7). `invocationNum` starts again at 0 on each new prompt. |
| `lastUserInput` | The prompt's text, the same on every call of that prompt. |
| A note sent once lasts the whole prompt | Yes. Prompt 3, "Run Get-Date, then tell me the new codeword if you were given one.", got `PLUM-77` only on call 0. The hook printed nothing on call 1, which came after the tool step. The answer, written in call 1, was "The new codeword is PLUM-77." |
| Shown in agy's terminal | No. The injected text appears only where the model repeats it. |
| Saved in the transcript | Yes. Both `transcript.jsonl` and `transcript_full.jsonl` hold each note as a step: `{"source":"SYSTEM_SDK","type":"EPHEMERAL_MESSAGE","status":"DONE","content":"Spike fact: ..."}`. |
| A slow hook holds the call | Yes. The 2 s hook began at 10:00:44.442Z, and its step was created at 10:00:46Z. agy waited for it, within the entry's 10 s timeout. |

## Verdict: build (Task 5a)

- **Once per prompt:** answer `PreInvocation` only when `invocationNum == 0`, and answer 204 on every later call. The note stays in the model's context for the rest of the prompt. The core keeps no state for this: no marker, and no hash of `lastUserInput`.
- **The answer:** `{"injectSteps":[{"ephemeralMessage":"<note>"}]}`, the same note text as Claude Code and Codex.
- **The chip:** it is set (or cleared, when nothing matches) on call 0 and left alone on later calls.
- **Privacy:** like Claude Code's `additionalContext`, the note is saved in agy's own transcript under the agy home. The core still keeps no prompt text. `lastUserInput` is read for the match before `strip_personal` runs.
