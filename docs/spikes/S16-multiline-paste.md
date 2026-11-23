# S16: A multi-line paste into each agent CLI's prompt under ConPTY (Windows)

Slice 6r-3, run 2026-11-23 before Task 7 was briefed, to pick each CLI's `comments_delivery` arm ("Slice 6r-3 decisions" 9 and 13). Dev box: Windows 11 Pro (build 26300). The app was `phase-6r-3` under `pnpm tauri dev`, started by the live-check skill against a scratch home (`%TEMP%\lc-6r3-s16`, with `CODEX_HOME` at `%TEMP%\lc-6r3-cx`) with `-CopilotFake -QwenFake -AgyAuth -CodexAuth`. No app code changed.

**Verdict: Paste for all five CLIs, at 16,000 characters.** None of them submitted a pasted line break, and each kept the whole of the largest text, so `PASTE_MAX` stays 16,000.

## How it was measured

- Each CLI ran in a Marshell session started for the check: Copilot CLI, Qwen Code, Codex and agy from their agent rows through the core's `POST /v1/sessions`, and Claude Code typed into a PowerShell session.
- Claude Code reached its prompt without the user's account. The shell set `ANTHROPIC_API_KEY` to a fake key and `ANTHROPIC_BASE_URL` to the loopback fake model's port, with `DISABLE_AUTOUPDATER=1` and `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`. `CLAUDE_CONFIG_DIR` and `USERPROFILE` were checked to be the scratch ones before `claude` started. Claude asked whether to use the key found in the environment, and the check said yes.
- The texts went in as the terminal's own paste. Over CDP, a `ClipboardEvent("paste")` with a `DataTransfer` holding the text was dispatched on the session's `.xterm-helper-textarea`, so xterm.js turned `\n` into `\r` and added the bracketed-paste markers when the CLI had asked for them. Task 7 sends the same bytes.
- Three texts were used:
  - three lines, `Spike line one\nSpike line two\n> spike line three`;
  - 20 lines of 150 characters (3,019 characters);
  - 100 lines of 160 characters (16,099 characters).
- Each result was read from the core's `GET /v1/sessions/{tab}/peek`. A submit would have shown as a turn on screen and as a request in the fake model's log (Copilot, Qwen, and Claude through its base URL), or as a rollout under Codex's `sessions/`. No log file and no rollout was ever created.
- The input was cleared between texts with Ctrl+C (Copilot, Codex, Claude Code) or Esc twice (Qwen, agy), never Enter.

## Findings

| CLI (version) | Three lines | 3,019 characters | 16,099 characters | Arm |
| --- | --- | --- | --- | --- |
| Claude Code (2.1.330) | kept as three lines | `[Pasted text #1 +19 lines]` | `[Pasted text #2 +99 lines]` | Paste |
| Codex (0.171.0) | kept as three lines | `[Pasted Content 3019 chars]` | `[Pasted Content 16099 chars]`, settled about 6 s after the paste | Paste |
| Copilot CLI (1.0.106) | kept as three lines | `[Paste #1 - 20 lines]` | `[Paste #2 - 100 lines]` | Paste |
| Qwen Code (0.26.5) | kept as three lines | `[Pasted Content 3019 chars]` | `[Pasted Content 16099 chars]` | Paste |
| Antigravity CLI (1.5.14) | kept as three lines | `[Pasted text #1 +20 lines]` | `[Pasted text #1 +100 lines]` | Paste |

1. **No CLI submits a pasted line break.** Every line of every text stayed in the input, and the line counts in the placeholders match the texts (Claude counts line breaks, the others count lines).
2. **Codex keeps the paste although it reads it as typed keys.** `docs/research/attachment-ref.md` found that on Windows a bracketed paste reaches Codex as typed characters. The composer still treats the burst as one paste: the `\r`s became line breaks, not a submit. The 16,099 characters took about 6 s to settle, and the composer looked empty in between. An Enter typed during those seconds would send what had arrived so far, so Send moves the keyboard to the terminal only after the paste has been written, and never types Enter itself (decision 9).
3. **Codex saves a cleared draft to its prompt history.** Ctrl+C on a draft added it to `history.jsonl`, as Up-arrow recall, without running a turn. A comments message cleared that way stays in Codex's history.
4. **Onboarding that a check meets.** agy's first run ticks "Yes, I agree to help improve Antigravity CLI by allowing Google to collect and use my Interactions data"; the check unticked it before Done, since agy runs on the user's own Google sign-in. Copilot and agy ask for folder trust, Codex for its sandbox, Claude Code for theme, key and trust. All of it was answered in the check's own sessions.

## What 6r-3 takes from this

- `comments_delivery` is `Paste` for `claude`, `codex`, `copilot`, `qwen` and `antigravity`. The file form (decision 13) stays for a session whose CLI has bracketed paste off when Send runs, and for a message over `PASTE_MAX`.
- `PASTE_MAX` stays 16,000 characters.
- Known limit: in Codex a long message takes seconds to appear in the composer. Marshell can't see when it has settled, so the announcement ("Pasted 3 comments into …") comes when the bytes are written, not when Codex shows them.
