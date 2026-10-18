# S9: Permission hold vs the terminal prompt

Run 2026-10-18 on the dev box (Windows 11 Pro 26300) against Claude Code 2.1.302 in `default` (manual) mode, in the
same scratch setup as S10. `PermissionRequest` had a third entry: the bridge in the `permission` role, `timeout: 600`.
Requests were answered through `POST /v1/permissions/{rid}/decision` and `/release`. The screen was read by replaying
the session's recording through xterm.js, and keys went in as trusted CDP key events.

| Check | Result |
|---|---|
| Claude's own dialog while held | **Visible.** "Do you want to proceed? 1. Yes / 2. … / 3. … / 4. No" shows in the pane for the whole hold. The hold hides nothing. |
| Allow, once | **Pass.** `touch allow-1.txt` ran and the file was created. |
| Deny with a message | **Pass.** The pane showed "Denied by PermissionRequest hook", the model quoted the message back, and the file was not created. |
| Release | **Pass.** The hold ended. The native dialog was already on screen, so it took over with no delay, and Esc then cancelled it. |
| Expiry (`MARSHELL_PERMISSION_HOLD_SECS=10`) | **Pass.** The hold ended on its own after 9951 ms and the native dialog stayed. |
| Typing | **Pass.** A key pressed while held released the hold at once, and Claude took it as the answer ("4", No). |
| Allow for this session | **Pass.** `python -c "print(4242)"` was allowed with scope `session`, and the same command ran again without asking. `.claude/settings.local.json` gained no rule. |
| Allow for this project | Not run. It maps to the `localSettings` destination, and 3c tests it with the consent flow. |

## Findings

1. **Terminal reports released holds** (fixed in 3a).
   - What happened: blurring and refocusing the pane released a held request. Claude enables focus reports (`?1004h`) and any-motion mouse tracking (`?1003h` with SGR `?1006h`), so moving the mouse over the pane, the wheel, or clicking an approve card elsewhere would also release it.
   - The fix: the core now releases only for input that isn't made up entirely of terminal reports (`crates/core/src/server/reports.rs`). Terminal reports are focus changes, mouse motion, the wheel, button releases, and replies to queries (cursor position, device attributes, mode and window reports, OSC and DCS replies). These bytes still reach the program.
   - Re-checked with a 10 s hold: focus out and in, eight mouse moves and a wheel turn left the hold in place, and a key still released it.
   - Known gap: a modified F3 (`CSI 1;5R`) has the same bytes as a cursor position report, so it doesn't release.
2. **Scoped allows need an `addRules` suggestion.** A `touch` request offered only `addDirectories` and `setMode`
   suggestions, so the core refuses a session or project scope for it ("This request has no rule to add").
   **Follow-up for 3c/3d:** decide whether "allow for session" may use an `addDirectories` suggestion, or whether the card hides the scopes when there is no rule.
3. **The default hold stays 120 s.**
   - Why it's safe: Claude's own dialog is visible and answerable during the whole hold, and typing in the pane releases it, so a long hold costs the person nothing.
   - What it buys: it gives a card in another tab or window (3d) time to be seen.
   - PLAN §5 and `DEFAULT_PERMISSION_HOLD` are unchanged.

Decision: [x] hold with the native dialog visible, default 120 s  [ ] shorter hold
