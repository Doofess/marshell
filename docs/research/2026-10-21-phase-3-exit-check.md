# Phase 3 exit check

- Date: 2026-10-21
- Build: `main` at 3696161 (after PR #43). Behaviour was checked on `pnpm tauri dev`. The restart, the index repair, cold start and typing were checked on the release exe from `pnpm tauri build --no-bundle`. Windows 11 Pro (26300), Claude Code 2.1.306
- Home: a scratch `MARSHELL_HOME` (`%LOCALAPPDATA%\Temp\marshell-p3x2\home`)
- Target: `MARSHELL_CLAUDE_SETTINGS` set to the scratch project's `.claude\settings.local.json`. Before consent it was seeded with content of the user's own: an unrelated key (`cleanupPeriodDays`), a `Stop` hook group and a `statusLine` running a small Node script
- Driven over CDP (`--remote-debugging-port=9333`) with trusted input (`Input.dispatchKeyEvent`, `Input.insertText`) and through the local API with the scratch home's token. Two sessions were started for the check: a Claude Code session ("p3 exit", Manual) and a PowerShell session. The Claude session was resumed twice

## Exit criteria

| Criterion | Result |
| --- | --- |
| Model | Pass. The row read "Opus 5.5", Claude's status line `model.display_name`. After `/model sonnet` it read "Sonnet 5.5" at the next status line, in the row and its accessible name. |
| Effort | Pass. The row read "high", as Claude's own footer did ("● high · /effort"). After `/effort medium` it read "medium". |
| Mode | Pass. The hooks reported `default` and the chip read Manual. After Shift+Tab, Claude's footer read "accept edits on", the next `UserPromptSubmit` reported `acceptEdits`, and the chip read "Mode: Auto-edit, as Claude Code last reported." Cycling back gave Manual. Both Resumes ran in the mode last reported (`--permission-mode acceptEdits`, then `manual`). |
| Subagents | Pass, and the first live run of PR #43. Two background Explore agents: the row went 0 → "1 subagent" → "2 subagents" → "Waiting for subagents · 2 subagents" (main `Stop` with 2 running `background_tasks`) → 1 → 0 → Done. Done came at Claude's final `Stop`, after Claude resumed the turn twice on its own, not from the 30 s fallback. |
| Context | **Not correct on this setup** (finding 1). The ring read 6% while `/context` read 59.3k/300k tokens (20%). Fixed by decision 1 below. |
| Approval from a card | Pass. `echo approved > approved.txt` got a card "Run a command" with Deny, Allow once and Answer in terminal. Allow once ran the command and the file held "approved". `echo denied > denied.txt` got a card, and after Deny no file existed and Claude said "Marshell denied it". `rm -rf approved.txt` got the risk check ("Deletes files recursively", Hold to allow), and after Deny the file stayed. No card offered Always, because each request's only suggestion was `addDirectories` (slice 3d decision 2). |
| Uninstall leaves zero byte diff | Pass. Consent wrote 10 hook groups and the status line chain, appending Marshell's `Stop` group after the user's own. `marshell uninstall` without `--yes`, answered N, listed only the scratch settings file and `<home>\bin`, and changed nothing. With `--yes`, the settings file was byte-identical to the seed (`cmp`, SHA-256 `afbdc4fe…` both). `marshell.exe` could not delete itself (finding 3). |
| Corrupt `index.sqlite` is repaired on start | Pass. With the app closed, the index was overwritten with text. On start, the release exe moved it to `index.sqlite.broken-1792596403846`, rebuilt the index from the one sidecar, and logged it in `logs/marshell.log` with the home path redacted ("history index was broken; moved to ~\AppData\…\index.sqlite.broken-… and rebuilt from 1 sidecar(s)"). Both rows came back Ended, and Resume ran `claude --resume <the same id>`. This closes finding 2 of the 3e1 history check. |

Also checked:

- **Version gate.** Claude Code 2.1.306, one patch past the 2.1.305 fixtures, was accepted: live status, never "Status limited".
- **Resume in a live shell (#39).** After Claude ended inside its PowerShell, the row read Ended and offered no Resume bar, palette command or menu item. Once the shell exited, the bar returned, and Resume ran `claude.exe --resume <id> --permission-mode acceptEdits`. Mode ▸ stayed in the menu (finding 2).
- **Status line chain.** Claude's footer showed the seeded status line's output ("seed status line") while Marshell's ran first.

## Latency budgets

| Interaction | Budget | Measured | Result |
| --- | --- | --- | --- |
| Typing, keypress to paint | ≤ 30 ms | Release: p50 3.7 ms, p95 8.3 ms (n 89, single keys). Dev: p50 4.5 ms, p95 31.0 ms (n 104, including whole prompts inserted at once) | Pass on the release build |
| Hook to visible card | ≤ 150 ms | 18 ms and 30 ms, from the core receiving `PermissionRequest` to the card's buttons in the DOM | Pass. The bridge's own start is not included |
| Decision to bridge return | ≤ 50 ms | Not measured. Nothing times it; click to Claude's `PostToolUse` was 116 ms, which includes running the command | Not measured |
| Tab switch | ≤ 50 ms | 6.4–9.3 ms (dev, 8 switches between a Claude and a PowerShell session) | Pass |
| Palette and launcher open | ≤ 50 ms | Palette 10–15 ms, launcher 4.6–7.8 ms (dev, from the chord's keydown to the frame after it shows) | Pass |
| Peek | ≤ 100 ms | 12–16 ms (dev, Space on the row to the frame after the screen text arrives) | Pass |
| Cold start to the layout | ≤ 800 ms | 598 ms from process start to first contentful paint, restoring two rows (release). The core was serving at 373 ms (`startup_ms`) | Pass |

## Checks on main

- `cargo test --workspace`: 42 suites, 1061 passed, 0 failed, 2 ignored.
- `npx vitest run --dir src`: 294 files, 4309 tests passed (see finding 7 for why not plain `pnpm test`).
- `pnpm tauri build --no-bundle`: built `target\release\marshell-app.exe`.
- PLAN's "before claiming any phase done" asks for CI green on all three OSes and a fixture and E2E suite. CI runs on Windows only, by decision (macOS and Ubuntu are paused until the hardware exists). The fixture tests pass in `cargo test`. There is no E2E suite. This check is the manual run with real Claude Code.

## Findings

1. **The context ring doesn't match `/context` when `autoCompactWindow` is set.** Claude's status line sends `context_window.used_percentage: 6` with `context_window_size: 1000000`. The user's `~/.claude/settings.json` sets `"autoCompactWindow": 300000`, which the status line never carries, and `/context` measures against it: 59.3k/300k, 20%. With the ring as built, it would read about 30% when Claude compacts, so its 70% and 90% warnings never fire. `total_input_tokens` (59,245) matches `/context`'s total. Decision 1 below.
2. **Mode ▸ is still offered while an ended agent's shell runs.** Resume is hidden then (#39), but Mode ▸ in the row menu restarts the agent, which ends the shell the person is typing in (after its confirm bar). Decision 2 below.
3. **`marshell uninstall` can't delete its own exe.** It removed `marshell-hook.exe` and printed "Couldn't remove …\bin\marshell.exe; delete it once this window is closed.", because Windows locks a running program. Decision 3 below.
4. **A row is briefly Done between a `Stop` and Claude's own resumed turn.** After the second subagent ended, a main `Stop` with no running `background_tasks` came 78 ms before the `UserPromptSubmit` Claude sends when it resumes the turn. The row was Done for those 78 ms. Nothing acts on Done yet; phase 4 needs to handle this before Done gets a sound or notice.
5. **Claude Code 2.1.306 kept the recorded shapes.** The hook bodies and the status line had the fields the 2.1.302 and 2.1.305 fixtures have, so no 2.1.306 fixtures were recorded. Hook bodies already carry `effort`, which nothing reads; it could give the effort before the first status line.
6. **The index rebuild is logged twice,** once by `history::index` ("moved to …") and once by `history::recorder` ("moved to … and rebuilt from 1 sidecar(s)").
7. **`pnpm test` at the repo root also runs the worktrees' tests.** With seven worktrees under `.claude/worktrees/`, Vitest found 2035 files instead of 294. CI is not affected (no worktrees there). The Vitest config should exclude `.claude/**`.

## Decisions (2026-10-21)

Recorded in docs/PLAN.md §9, "Phase 3 exit check decisions".

1. The context ring matches `/context`: used input tokens divided by the effective window, the smaller of the model's window and Claude's `autoCompactWindow`.
2. Mode ▸ is hidden while an agent has ended inside a shell that still runs, as Resume is.
3. `marshell uninstall` deletes its own exe after it exits, then the empty `bin` folder.

## Notes on the check

- **Claude Code reads its settings files as strict JSON.** The first seed had a `//` comment, and 2.1.306 refused the whole file at start ("Invalid or malformed JSON … Files with errors are skipped entirely"). Marshell's JSONC edit had kept the comment. The check was restarted in a fresh scratch folder with a strict JSON seed.
- **Claude's `/model` and `/effort` write the real `~/.claude/settings.json`,** whatever `MARSHELL_CLAUDE_SETTINGS` says: `/model sonnet` set `"model"` ("saved as your default for new sessions"), and `/effort medium` set `modelSettings.claude-opus-5-5.effortLevel`. Both were put back by hand. Later checks should start a session with `--model` and `--effort` instead. Marshell itself wrote nothing outside the scratch folder: `~/.marshell` kept its date (2026-10-17 22:16:46), and there was no `~/.marshell/bin`.
- **A card needs Manual mode.** Claude auto-allows read-only commands (`git status --short`) in every mode, and file commands such as `rm` inside the project in accept-edits mode.
- **`/exit` typed through `Input.insertText` reached the model as a prompt** (it answered "Goodbye!"). Ctrl+C twice ended Claude.
- At the end, the process list held nothing the check started. A `node.exe` from 18:12 belonged to the Codex desktop app and was left alone.
