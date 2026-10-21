# Notifications live check (slice 4a)

- Date: 2026-10-21 to 2026-10-22 (21:55 to 00:46 local time)
- Build: `phase-4a` at 67ef727, an NSIS build (`pnpm tauri build`) installed per user on Windows 11 Pro (26300). The code is the same as 9a1add2; 67ef727 changes only docs. The installed build was used because Windows shows a banner under the app's own name and icon only for an installed app; under `pnpm tauri dev` it came from PowerShell (spike S3).
- Home: a scratch `MARSHELL_HOME` in the session's temp folder, CDP on port 9333.
- Sessions: three plain `cmd.exe` sessions ("Shell", "Shell 2", "Shell 3"), started for the check. Agent states came from Claude Code hook bodies posted to `/v1/hook` with the scratch home's token (`UserPromptSubmit`, `PermissionRequest`, `Stop`, `SubagentStart`, `SubagentStop`), as Claude's bridge posts them. No real agent CLI ran.
- Window state came from CDP (`document.hasFocus()`, the selected row). Banners were checked in screenshots of the screen corner, and sounds by the user's ear.

## Results

| Check | Result |
| --- | --- |
| Needs-you, window in the background | Pass. The banner came under the Marshell name and icon: "Shell needs you", the request's phrase, "Session 1 · Home Office" on its own line, and an Open button. The taskbar button flashed and the badge counted the held requests. |
| Injected markup | Pass. A command containing `</text><action activationType="protocol" arguments="https://example.com"/>` showed as plain text on the banner, with Open the only button. |
| Batching | Pass. Requests from three sessions within the 3 s window: the first banner went out at once, then one banner "3 sessions need you" naming "Sessions 1 Shell, 2 Shell 2 and 3 Shell 3" (seen in the notification centre). |
| Reminder after a minute | Pass. About 60 s after the requests: "Shell 3 needs you / Shell 3 has waited 1 min to run a command". |
| Banner click | Pass. Open on "Shell 2 needs you" brought the window forward (`hasFocus` true) and selected session 2, where session 1 had been selected. |
| Window focused | Pass. A request on session 3 while the window had focus (session 2 selected): no banner and no flash. The badge went from 1 to 2. |
| Done needs 30 s out of sight | Pass. A turn finished 3 s after the window lost focus: no banner. 40 s after the focus loss, nothing had been replayed, and the next finished turn showed "Shell 2 is done / Finished the refactor. / Session 2 · Home Office". |
| Resumed turn | Pass. On sessions 2 and 3, `Stop` followed 0.4 s later by `UserPromptSubmit`, then a real `Stop`. Windows queued the banners while the user was away (see finding 2). Stepping through the queue showed three banners, "Shell 2 is done" twice and "Shell 3 is done" once, one for each real `Stop`. The two resumed turns showed none. |
| Quiet hours | Pass. Turned on at 22:00–08:00 while the clock read 22:20: the needs-you banner came, the taskbar did not flash, and in the listening run (step 6) a Done during quiet hours made no sound. |
| Muted session | Pass. With session 3 muted through `PUT /v1/sessions/{tab}/muted`, a request on it brought no banner and, in the listening run (step 5), no sound. The badge still counted it. |
| Sounds | Pass. The listening run played needs-you, Done, subagent done (switch on), error (`SystemHand` played directly, since nothing produces an error yet) and the one-minute reminder. The user heard each one, heard nothing for the muted and quiet-hours steps, and could tell all five apart. The user kept all five (PLAN §9, slice 4a decision 4). |
| Windows do-not-disturb | Pass, with the known limit. With do-not-disturb on, no banner appeared, and `SHQueryUserNotificationState` returned 5 (`QUNS_ACCEPTS_NOTIFICATIONS`), as spike S3 found. A single needs-you sound, played on its own, was heard, and so was its one-minute reminder, a different sound, 60 s later. Two earlier tries were unclear: one was silent, and in one of three sounds played 7 s apart, the user heard only one and couldn't say which. Marshell's sounds can play during do-not-disturb, so quiet hours remain the way to silence them. |
| Notification centre click | Not changed from S3: an entry clicked in the notification centre opens nothing (known limit). |

## Findings

1. **The badge counts held requests, not needs-you rows.** After a request's 120 s hold ran out, the CLI asks in its own terminal and the row still reads needs-you, but the taskbar count dropped by one. That is the count the approval queue has kept since slice 3d. 4b's needs-you count in the title should decide whether a row whose hold ran out counts.
2. **Windows keeps banners on screen while the user is away.** With no keyboard or mouse input, each banner stayed until it was dismissed, and later ones queued behind it, one at a time. Nothing was lost, but a batch summary can arrive long after its first banner. This is Windows behavior; the banner has no tag that would let a summary replace its first banner (T6 ruling).
3. **A banner times out before a slow click.** One Open click landed after the banner had gone (`MessageDuration` 5 s) and reached the desktop. That is Windows timing, not a fault.

4. **The uninstaller leaves the install path in the registry.** `uninstall.exe /S` removed the install folder, the Start-menu shortcut and the `Uninstall` entry, but kept `HKCU\Software\marshell\Marshell`, whose default value is the install folder. It comes from Tauri's NSIS installer, not from 4a, and was removed by hand. It belongs with the installer work in phase 9.

## Real config

The check used only the scratch home and posted hook bodies, and started no agent CLI. `~/.claude/settings.json` (2026-10-21T20:20:18) and `~/.marshell` (2026-10-21T19:42:54) kept their dates, both from before the check. The installed build was removed with its uninstaller (finding 4).
