# Tray live check (slice 4b)

- Date: 2026-10-22, 02:35 to 03:30 local time.
- Build: `phase-4b` under `pnpm tauri dev` on Windows 11 Pro (26300). Banners in a dev build come from PowerShell's app id, not Marshell's (spike S3); their text was read from the Windows notification database (`wpndatabase.db`, copied to the scratch folder).
- Home: a scratch `MARSHELL_HOME` in the session's temp folder, CDP on port 9333, `MARSHELL_PERMISSION_HOLD_SECS` 30 for the first run and 900 for the second.
- Sessions: plain `cmd.exe` sessions started for the check. Agent states came from Claude Code hook bodies posted to `/v1/hook` with the scratch home's token. No real agent CLI ran.
- How it was driven: the webview over CDP, the window list and the tray icon through UI Automation (the icon is in the taskbar's overflow flyout, where its accessible name is the tooltip), and the tray menu with real mouse and keyboard input. The menu's items are not exposed to the UI Automation client, so the menu was read from a screenshot. Two rounds of hand steps were tried first; they left no trace in the app (no saved close choice, no pause), so they are not counted below.

## Results

| Check | Result |
| --- | --- |
| Count and title | Pass. Two held requests: title "(2) Command Prompt — Marshell", both rows needs-permission in the core. A third wait: "(3) …". |
| Expired hold | Pass, with a note. When a 30 s hold ran out on a plain-shell row, the title dropped the count while the core still said needs-permission: `toRow` reads `live` only for agent rows, so a plain shell reads Idle in the sidebar and the count agrees with the sidebar. An agent row keeps counting (the `laneEntries` test). 4a's finding 1 came from the same check method: hook bodies posted into plain shells. |
| First close | Pass. The title-bar Close with no choice saved opened "When you close the window", with the exact copy and focus on "Keep running in the tray". |
| Escape | Pass on the second run: the dialog closed, the window stayed, `close_to_tray` stayed null. On the first run Escape closed the dialog but `close_to_tray` had become `true`, which fits "Keep running" being pressed by a stray key while the Ask path brought the window forward during remote driving. It did not happen again. |
| Keep running | Pass. `close_to_tray` saved `true`, the window hid (no taskbar button, no top-level window), and the sessions kept running. The hidden webview's `document.visibilityState` stayed "visible", which is why the core keeps its own hidden flag. |
| Close with a choice saved | Pass. The next Close hid the window with no dialog. |
| Done while hidden | Pass. Session 1 was on screen when the window hid; a finished turn more than 30 s later raised "Command Prompt is done / Finished the refactor. / Session 1 · Home Office". |
| Back from hidden | Pass. A second launch (single instance) brought the window back, focused. A finished turn on the selected session then raised no banner, so the hidden flag was cleared. |
| Tray tooltip | Pass. "Marshell · 2 need you", then "Marshell · 3 need you (notifications paused)" while paused. See finding 1 for how it is read. |
| Tray menu | Pass. Open Marshell; "5 · Command Prompt 5: Needs your permission" and "6 · …"; Go to the oldest waiting session, Ctrl+Shift+N; Pause notifications; Notifications…; Quit Marshell. Dark menu, separators between the groups. While paused the item reads "Resume notifications" and a third session is listed. |
| Icon | Pass on a dark taskbar: white wands with the accent dot while requests wait. The light-taskbar drawing was not switched live (it would change the user's Windows theme); `tray_rgba` and `taskbar_is_light` have unit tests. |
| Left click | Pass. A real left click on the hidden window's icon showed it, in the foreground and focused. |
| Go to the oldest | Pass. From hidden, it showed the window on session 5, the oldest waiting, in the foreground (see finding 3 for a keyboard-picked item). |
| Notifications… | Pass. From hidden, it showed the window with the Notifications dialog open and focus on its title. |
| Pause | Pass. `GET /v1/notify/paused` read `true`. A new wait while paused raised the count to 3, with no banner and no one-minute reminder. |
| Resume | Pass. `paused` read `false`. See finding 2 for the reminder that fell due during the pause. |
| Close button with the switch off | Pass. Turning "Keep Marshell running in the tray when I close the window" off saved `close_to_tray: false`; the next Close quit the app and its whole process tree. |
| Hidden for more than 5 minutes | Pass. With the window hidden for 13 minutes, the last 5½ with nothing touching the webview (no CDP), a new wait changed the tooltip from "2 need you" to "3 need you" 0.6 s after it was posted. An earlier run after 7 minutes gave the same 0.6 s. The hidden webview keeps driving the tray. |
| Quit from the tray while hidden | Pass. Picked with the keyboard from the menu (Up, Enter), Quit ended the app and its whole process tree; the next start showed the window visible. |
| Empty tray menu | Pass. With nothing waiting, the menu read "Nothing needs you" and "Go to the oldest waiting session" was disabled; the tooltip read "Marshell". |
| Tray icon from the keyboard | Pass. Win+B, then Enter, focused the Marshell icon in the overflow; Shift+F10 opened Marshell's own menu (owned by the app's process) and Escape closed it. |
| Forced colours | Pass, emulated. With forced colours turned on over CDP (not a real high-contrast theme), the close dialog and its buttons were outlined, and the default button had the thicker border with a visible focus ring. |

## Findings

1. **The icon's accessible name says "Marshell" twice.** Windows reads the overflow icon as "Marshell Marshell · 2 need you": it puts the app's name in front of the tooltip. A screen reader at G1 of the script will hear the doubled name. Whether the installed build's prefix is the same is for the release pass; if it is, the tooltip could drop its own "Marshell".
2. **A reminder that falls due during a pause is dropped.** Session 7's wait began while paused; its one-minute reminder fell inside the pause and went nowhere, and none came after Resume. The count, the title and the tray kept showing it. This fits decision 4 (pause silences until Resume) and leaves no burst of old reminders on Resume.
3. **A menu item picked with synthesized keys shows the window behind the foreground app.** Picking "Go to the oldest" with arrow keys sent from a script showed the window on the right session but left another app in front. Picked with a mouse click, the window came to the front. Windows grants the foreground to the process that received the input; a person's own keyboard was not tried.
4. **The sidebar's resize handle showed through four dialogs.** Reported by the user during the check: in Appearance a line ran down the dialog and could be dragged. Appearance, Keyboard shortcuts, Shell profiles and the Review sheet set no `z-index`, so the handle (`z-index: 2`) stayed on top. Not 4b code; fixed on this branch in c01257d with a CSS test that pins every full-window dialog above the handle.
5. **An approve card with no known agent takes Claude's colour.** The card follows the session's vendor (PLAN §314): the requests in this check were posted as Claude Code, so Allow was Claude's terracotta. When neither the row nor the request names a known agent, `agentOf` in `src/features/approval/fromCore.ts` falls back to `"claude"`. Not 4b code; noted for the adapter work.
6. **Check method: synthesized mouse input needs a guard.** A helper script damaged by an in-place edit sent a right click and a click to screen coordinates near (0, 0), which landed in another app open beside the check rather than Marshell's menu. Later checks must confirm that the target rectangle is not empty and belongs to Marshell's process before sending any input, and should prefer the keyboard route above.
7. **Check method: fetch gives up on a held request after 300 s.** Node's `fetch` (undici's `headersTimeout`) ended one held request after five minutes, and the row then read Idle. The helper was switched to `node:http`. It is a property of the test client, not of the app.

## Real config

The check used only the scratch home and posted hook bodies, and started no agent CLI; onboarding was skipped, so no hooks were installed. `~/.marshell` kept its date (2026-10-21T19:42:54). `~/.claude/settings.json` was written at 03:06:03 during the check by the user's own Claude Code sessions running beside it; it holds no Marshell entry.
