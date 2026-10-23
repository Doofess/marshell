# Phase 4 exit check (slice 4e)

- Date: 2026-10-23, 14:54 to 18:20 local time, with a pause while the PC was in use.
- Build: an NSIS build (`pnpm tauri build --bundles nsis`, after `pnpm fetch:conpty`) installed per user with `/S` on Windows 11 Pro (26300). The first run used `phase-4e` at 5e40299. After the final review's one Important fix (f99150c), the build was rebuilt, installed over the first and every later row ran on f99150c. A silent install does not start the app; it was started by hand with the scratch environment.
- Home: a scratch `MARSHELL_HOME` in the session's temp folder, CDP on port 9333. Onboarding's consent card was skipped ("Skip: use limited status"), so nothing was written to `~/.claude`.
- Sessions: plain `cmd.exe` sessions created through `POST /v1/sessions`. Agent states came from Claude Code hook bodies posted to `/v1/hook` with the scratch token: `UserPromptSubmit` from the 2.1.302 fixtures and Task 1's `StopFailure` fixture (2.1.309, `authentication_failed`). No agent CLI ran.
- Window state and events came over CDP: `document.hasFocus()`, the selected row, `document.activeElement`, and a listener counting `focus-session` events. Banners were read in screenshots of the screen corner and the notification centre. Every synthesized click was refused unless the window under the point belonged to the expected process and nobody had touched the keyboard or mouse in the last second. The guard refused four clicks: one on a banner that had already gone (the point belonged to a game), one when a game covered the target, and two while the person was using the PC.
- Before the check, neither `HKCU\Software\Classes\marshell` nor `HKCU\Software\marshell` existed, so there was nothing to export or restore.
- Windows do-not-disturb was on (a game was running). It was turned off from the notification centre for the check and turned back on at the end.

## Results: slice 4e

| Check | Result |
| --- | --- |
| Error from `StopFailure` | Pass. The row's live state became `error` with the phrase "Sign-in failed". With Marshell in the background, the banner came under Marshell's name and icon: "Beta stopped / Stopped: Sign-in failed. / Session 2 · Home Office", with an Open button. The sound and the taskbar flash were not heard or watched (the person was away). |
| No banner while focused | Pass. While the core counted the window as focused, a `StopFailure` raised no banner. Note: Marshell started behind a game had never had OS focus, and the core still counted it as focused until it gained and lost focus once. Minimizing does not count as losing focus when the minimized window stays the foreground window. |
| Body click on screen | Pass (link off, first build). One `focus-session` for Beta's tab; Beta selected (Alpha had been); the window in the foreground with `hasFocus` true. |
| Open click on screen | Pass (f99150c). One `focus-session` for Beta 5; Beta 5 selected (Alpha 4 had been); `hasFocus` true; focus on Beta 5's row (`div.session-row`, row 5), not its terminal. |
| Centre click, link off | Pass. A click on "Gamma 6 stopped" in the notification centre did nothing: no event, the selection stayed, the window stayed in the background. |
| Switch on | Pass. The key appeared with the command `"C:\Users\Home Office\AppData\Local\Marshell\marshell-app.exe" "%1"`. |
| Centre click, link on, Marshell running | Pass. One `focus-session` for Gamma 6, 481 ms after the click; Gamma 6 selected; `hasFocus` true. Focus went to Gamma 6's row although Alpha 4's terminal had focus before the click, so typing would not have reached a terminal. The shell logged the toast's in-process activation; a protocol launch, if Windows also made one, was absorbed by the 2 s de-dupe. |
| Centre click for a closed session | Pass. Beta 5's entry, clicked after Beta 5 was closed: one event, 498 ms after the click; the window came forward; Alpha 4 stayed selected; focus on Alpha 4's row. |
| Switch off | Pass. The key was removed, and Windows held no Marshell toasts afterwards (`History.GetHistory` count 0). |
| Switch on, then silent uninstall | Pass. With the key pointing at the install, `uninstall.exe /S` removed the key, the install folder, the Start-menu shortcut and the Uninstall entry. `HKCU\Software\marshell\Marshell` (the install path) was left, as in 4a finding 4, and removed by hand. |
| Generated installer | Pass. `installer.nsi` includes `hooks.nsh` and inserts `NSIS_HOOK_POSTUNINSTALL` after the uninstaller's own cleanup; `$UpdateMode` is read from `/UPDATE` in `un.onInit`. |
| Centre click with Marshell quit | Not run. Windows would start Marshell outside the check's environment, against the real `~/.marshell`. |

## Brief §5.5, rule by rule

The 4e check re-ran the rules 4e changed. The others were verified on Windows 11 by the slice live checks named here and were not re-run.

| Rule | Where verified |
| --- | --- |
| Four categories, Done and subagent done distinct | 4a (`2026-10-21-notifications-live-check.md`): each played its own sound and the user told all five apart. Error now has a real source (above). |
| Silent banners, Marshell's own sounds | 4a for the sounds. 4e: every toast carries `<audio silent="true"/>` (`toast.rs`, test `the_toast_carries_the_link_everywhere`); nobody listened during this check. |
| Per-category switches | 4a (Settings → Notifications). The dialog in this check showed Banner and Sound for Needs you, Done and Error, and the subagent and terminal-alert switches. |
| No notification when focused on the tab | 4a ("Window focused"), and 4e above. |
| Batching | 4a ("3 sessions need you"). |
| Subagent sound and its switch | 4a (listening run, switch on). |
| One-minute reminder | 4a. |
| Quiet hours; do-not-disturb (known limit) | 4a: quiet hours silence sounds and flashes; under Windows do-not-disturb banners are held and Marshell's sounds can still play. |
| Per-session mute | 4a. |
| A banner click focuses the session | 4a (Open), and 4e above: body, Open, and the notification centre with the link on. |
| Badge and taskbar flash | 4a (badge count, flash); 4b (`2026-10-22-tray-live-check.md`) for the count in the title and the tray. |
| BEL, OSC 9 and OSC 777 | 4d (`2026-10-23-terminal-attention-live-check.md`). |
| Doctor: offer to turn off each CLI's own notifications | Moved to phase 7 (slice 4e decision 4). |
| Every sound has a visual equivalent | 4a and 4c (`2026-10-23-departures-live-check.md`): each category changes the row (status, flash), the badge or title count, and the banner; 4e's Error turns the row Error. |

## Findings

1. **The final review's Important finding, confirmed fixed.** Before f99150c a link or banner click let the selected session's terminal take keyboard focus, so a page that fired a link while the person typed could land their keys in an agent's prompt. On f99150c every link and banner click left focus on a session's row (three checks above).
2. **A window that never had focus counts as focused.** Started behind a full-screen game, Marshell reported itself focused until it was focused and left once, so its first banners were held. It is the webview's initial report, not 4e's code. People normally start Marshell in front, so this is noted, not fixed.

## Real config

The check used only the scratch home and posted hook bodies, and started no agent CLI. `~/.claude/settings.json` kept 2026-10-23T14:41:00 and `~/.marshell` kept 2026-10-21T19:42:54 (newest file `bridge.log`, 2026-10-21T20:05:54), as recorded before the check. Marshell was stopped by PID with its tree (only the webview, the shell panel's `pwsh` and the check's `cmd.exe` sessions), then uninstalled. Neither registry key remains.
