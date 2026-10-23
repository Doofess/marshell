# Terminal attention and stuck live check (slice 4d)

- Date: 2026-10-23, finishing at 12:26 local time.
- Build: `phase-4d` under `pnpm tauri dev` on Windows 11 Pro (26300).
- Home: a scratch `MARSHELL_HOME` in the session's temp folder, CDP on port 9333, notifications on in the scratch `settings.json`, and `MARSHELL_STUCK_QUIET_SECS=60` so the quiet timer could be seen in a minute.
- Sessions: three PowerShell sessions (alpha, beta, gamma) and one Claude Code session (delta: real `claude.exe` with a scratch `CLAUDE_CONFIG_DIR`, never typed into), all started for the check. Gamma and delta took their agent states from Claude Code hook bodies posted to `/v1/hook` with the scratch home's token. The OSC and bell output came from commands typed into alpha's shell over CDP.
- How it was driven: the webview over CDP only (evaluations returning small JSON, `Input.insertText` into the focused terminal, emulated media for forced colours, the tray's own `tray-action` event to open the notifications dialog), plus the local API for hooks, input and `PUT /v1/ui/view`. Banners were read from a copy of the Windows notification database. No desktop mouse or keyboard input was sent.
- The real `~/.marshell` was not touched: its folder date stayed at 2026-10-21 19:42:54.

## Method notes

- **No banner in a focused window.** The notify engine suppresses every banner while `window_focused` is true, so the first OSC 9 check showed the row and title change with no banner. That is the existing rule, not a 4d defect. Banner checks switched to another session and reported the window as unfocused with `PUT /v1/ui/view {"window_focused": false, ...}`.
- **Reading banners.** `wpndatabase.db` was copied to the scratch folder and only toasts under PowerShell's app id (the dev build's) in the last few minutes were printed.

## Results

| Check | Result |
| --- | --- |
| OSC 9 | Pass. `ESC ] 9 ; Build done BEL` in alpha's shell turned the row to needs you with "Build done" and the title to "(1)". |
| OSC 777 | Pass. `ESC ] 777 ; notify ; Tests ; All green BEL` read "Tests: All green", the row flashed (`data-flash` for 603 ms), and one banner read "alpha needs you · Tests: All green · Session 1 · Home Office". |
| Bell from a running command | Pass. `Start-Sleep 4; [console]::Write([char]7)` gave "Wants your attention". A failed Tab completion at the prompt gave no attention (the prompt rule and the typing-echo guard are unit-tested; whether PSReadLine rang was not observed). |
| Cleared by the user | Pass. Typing into the session (input "x", then a backspace) cleared it, and so did opening alpha from beta (the view report). |
| Switch off | Pass. "Alerts from terminal programs (bell, OSC 9, OSC 777)" turned off in the notifications dialog saved `terminal_alerts: false`. A later OSC 9 still changed the row and the title, with no banner. |
| Hooked session ignores it | Pass. Gamma (live idle from a SessionStart hook) printing OSC 9 and a bell stayed Idle with no attention. |
| Stuck: repeat | Pass. Nine identical PreToolUse bodies on delta marked it stuck on the ninth, not the eighth; the row read "Same command 9× in 4 min" with the "Might be stuck" glyph and `data-status="stuck"`. The title count did not change and no banner was posted. A different call cleared it. |
| Stuck: quiet | Pass. With delta Working and no hook for 60 s, the 30 s tick marked it quiet-stuck by 12:24:19 (started 12:22:45); the row read "No progress for 10 min". A Stop hook cleared it (Done). |
| Forced colours | Pass, emulated (not a real high-contrast theme). The stuck glyph's dashed ring and the attention row's glyph both show, and the reason lines are readable in system colours. |

## Findings

1. **The quiet copy names the default, not the setting.** With `MARSHELL_STUCK_QUIET_SECS=60` the row still read "No progress for 10 min". The override is a developer setting for checks like this one, so the copy stays as the plan has it.
2. **OSC 777 spacing.** A title or body with spaces around it (`; Tests ; All green `) keeps them, giving "Tests :  All green". Deferred.
