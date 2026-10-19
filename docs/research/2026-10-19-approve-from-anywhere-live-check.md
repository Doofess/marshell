# Approve from anywhere live check (slice 3d)

- Date: 2026-10-19 to 2026-10-20
- Build: `phase-3d` from 1d52fd2 (the last plan task) through the fixes below, `pnpm tauri dev` on Windows 11 Pro (26300), Claude Code 2.1.303
- Home: a scratch `MARSHELL_HOME` with a space in its path (`%TEMP%\marshell 3d home`)
- Target: `MARSHELL_CLAUDE_SETTINGS` set to `%TEMP%\marshell-3d project\.claude\settings.local.json`; the status hooks and the `PermissionRequest` hook were installed there through the consent card, so the real `~/.claude` was never written
- One Claude session, started for this check in the scratch project, in manual permission mode
- Driven over CDP (`--remote-debugging-port=9333`) and the local API with the scratch home's token; `set_badge` and `alert_needs_you` were logged by wrapping the webview's IPC `fetch`

## Results

| Check | Result |
|---|---|
| 1. Plain request: `node -e "console.log(7*8)"`, Ctrl+Shift+N, Y at once, then Y after the guard | Pass: the card appeared 14 ms after the hold opened, showing the exact command. Ctrl+Shift+N focused it; the early Y did nothing; the later Y allowed it. The "Allowed" receipt showed and cleared, focus went back to the terminal, and the command ran |
| 2. Deny with N | Pass: N after 700 ms gave the "Denied" receipt, and Claude replied that the command did not run because permission was denied |
| 3. Always with A (`node -e "console.log(9*9)"`) | Pass: `permissions.allow` gained `Bash(node -e "console.log\(9*9\)")`, Claude's own escaped form, and the rerun of the same command raised no card |
| 4. Risky `rm -rf junk/deep` | Pass: the card read "Deletes files recursively" and offered no Always; a short Y (down then up at once) left it pending; N denied it, and `junk/deep/a.txt` survived. Finding 1 |
| 5. Typing in the pane while a card is pending | Pass: typing `1` released the card within 400 ms with no error, and the keys reached Claude's own prompt |
| 6. A request with no tab (the bridge run from a plain terminal, no `MARSHELL_TAB_ID`) | Pass: the bridge exited 0 after 46 ms with no output, so Claude Code falls back to its own prompt |
| 7. Peek: Space on a row opens it, Esc closes it | Pass: Peek showed the session's last rows and Esc went back to the row. Finding 2 |
| 8. Taskbar badge and alert with the window unfocused | Pass: `set_badge 1` and one `alert_needs_you {sound: true}` per request; the taskbar badge read "1" legibly; Windows' notification sound played; answering gave `set_badge 0`. Findings 3 and 4 |
| 9. Card and Peek in forced colours, keyboard only | Pass: the card's text, risk line, focus ring and buttons, and Peek's frame and rows, all drawn in system colours; every step was reachable from the keyboard |
| 10. The lane opening (reported by the user as laggy) | Pass after finding 5: the lane grows from 28 to 284 px over about 180 ms (sampled each frame) and the card fades in |

## Findings

1. **The caution glyph wrapped away from the risk reason in the lane.** The line clamp sat on the risk line, which is a flex row, so a long reason pushed the glyph onto its own line. The clamp now sits on the reason's span (c495735).
2. **Peek opened at the session's oldest row.** A peek is for what the session is doing now, so it now opens scrolled to the newest row (5a864e1).
3. **The synthesised needs-you chime was harsh.** The user heard it during the check and asked for it to go. By their decision (PLAN §9, 3d decision 9) the alert now plays Windows' own notification sound, on by default with a switch in Settings, in place of the plan's rodio chime (64bef4f).
4. **One request alerted twice.** The lane first shows the session's waiting row (`terminal-<tab>`), then the held request replaces it under a new id, so `attentionStep` saw two new entries. New entries are now keyed by tab, so it alerts once per tab (ef12f57).
5. **The lane jumped 256 px in one frame when a card arrived.** Frames were steady (about 7 ms each at 144 Hz); the lag the user saw was the missing enter motion. The lane's block size now transitions over `--dur-enter` (180 ms) with `--ease-out`, and the card fades in with a 4 px rise, as PLAN's Motion table says. Under reduced motion the size snaps and the card fades over 80 ms (c677005).

### Minor, not fixed

- The pty WebSocket attach (`/v1/pty/<tab>`) logs a 404 once on first attach and after a restart, then connects; the terminal works. This is the known spawn race.
- Nerd-font glyphs in Peek's rows render as "Ø", since Peek draws plain text in the UI font.

## Notes

- Claude Code shows its own permission prompt in the terminal while the card is pending. Typing into the pane releases the card (check 5) and the keys answer that prompt, so the native prompt is the fallback.
- Claude escapes parentheses in the rule an Always adds; the card names the rule as Claude's suggestion gives it.
- Claude Code 2.1.303 prints "※ recap" lines in the terminal; they come from Claude, not Marshell.
- An earlier report of lost input during the check came from real mouse clicks outside the driven element (the pointer log showed them), not from Marshell.
- Space on the card's Deny button first did nothing because the CDP harness sent the key without a `code`; once it sent `Space`, it worked. A harness artifact, not a Marshell bug.
- Keys typed while a card was pending reached Claude's native prompt and chose its option 2, adding `Bash(node *)` to the scratch project's `settings.local.json`. That is check 5 working as designed; later traces used `touch marker.txt`, which was denied.
- `Emulation.setEmulatedMedia` (forced colours, reduced motion) resets when each CDP connection closes, so each emulated check ran in one connection.
- Cleanup: the dev app was stopped by its task, its hook bridge had already exited, and both scratch folders were removed.
