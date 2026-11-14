# Ctrl+Tab rollover live check (slice 6q-3 decision 15)

- Date: 2026-11-13, 19:16 to 19:45 local time (the CDP checks), and 2026-11-14, 14:10 to 14:21 (real keys, with the user). The re-check after the fix pass has its own section at the end.
- Build: `ctrl-tab-rollover` at f20432d under `pnpm tauri dev`, plus an uncommitted log in the shell's key handler (Tab and the modifiers only), on Windows 11 Pro (10.0.26300) with WebView2 runtime 157.0.4341.46, one display.
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `MARSHELL_CLAUDE_SETTINGS`, `CODEX_HOME`, `COPILOT_HOME` and `USERPROFILE`, with CDP on port 9333. Onboarding was passed with "Skip: use limited status" on every card. No agent CLI was started. The real `~/.claude/settings.json` kept its modified time.
- Data: one PowerShell 7 session ("Just the shell"), and three browser tabs opened with "Open URL…" on a key-logger site on loopback (`http://127.0.0.1:47630/`, `/b`, `/c`, a Node script in the scratch folder that is not committed). Each page logs every key, focus and blur it hears to the site.
- How it was driven: CDP keys in the app's webview (`page.keyboard.down/up` with `waitForTimeout`), and CDP evaluations that logged each key's `timeStamp` and `isTrusted`, the swallowed ones too, plus the announcer and the selected tab. Native focus was read with `GetGUIThreadInfo`, read-only. Then, with the user and the dev window in front, real keys through `SendInput` from a script that checked, before every send, that the foreground window was the dev app's and that no modifier was down: 30 rolled presses with Tab 10 ms before Shift, and 2 plain Ctrl+Tabs. Then the user pressed the chords by hand. No screenshots.

## Results

| Check | Result |
| --- | --- |
| Rolled, timer path (app) | Pass. Over CDP from the terminal: Ctrl, then Tab, with Shift 16.1 ms later and Tab held 80 ms. The Tab press was swallowed and Ctrl+Shift+Tab was sent on Shift's key-down; the announcer said "Selected the side pane", and the tab stayed on Page C. In the app both rolled paths end at Shift's key-down, since the app hears Shift. |
| Rolled, release path (app) | Pass, by real keys: 15 of 15 from the terminal, with Shift 10 ms after Tab and Tab up at 60 ms (10 presses) or 30 ms (5). Shift reached the app 16 to 18 ms after Tab. Each said "Selected the side pane", and the tab never stepped. The CDP run of this case hit finding 2. |
| Plain Ctrl+Tab | Pass. From the terminal over CDP: Tab held 80 ms stepped the tab (Page C to Page A) when the timer sent it, 60.6 ms after the press, and Tab up at 28 ms stepped it (A to B) on the release. Both times the keys stayed in the terminal. From a page by real keys: Tab held 80 ms timed out in the shell with Shift up and stepped C to A; Tab up at 20 ms ended at the release and stepped A to B, and that hold's late timeout sent nothing. |
| Auto-repeat | Pass. Over CDP, two repeats at 29 and 45 ms were swallowed, and the tab stepped once (B to C), 60.3 ms after the press. |
| Settings | Pass. Settings moves Tab and Ctrl+Tab alike to its next control. A held Ctrl+Tab and one released at 20 ms both moved from "Advanced" to the theme chip, as a plain Tab does. |
| Page side, rolled (real keys) | Pass, 15 of 15. The shell held each Ctrl+Tab. The 10 with Tab held 60 ms ended at 50 ms, which read Shift as down (`GetAsyncKeyState`). The 5 with Tab up at 30 ms ended at Tab's release, which read Shift as down too (`GetKeyState`). Each reached the app as Ctrl+Shift+Tab ("Selected session 1, PowerShell 7"). The tab never stepped, and the page heard no Tab key-down. |
| Did WebView2 report Shift? | No. The shell's handler saw Ctrl and Tab and never Shift, down or up, in all 30 rolled presses and the hand round, as the `AcceleratorKeyPressed` reference says. The page heard every Shift. |
| By hand | Pass for the ordinary chords. Each of the user's 20 Ctrl+Shift+Tab presses (10 from the terminal, 10 from a page) switched between the terminal and the page; the page's focus and blur alternate with them. Their 15 plain Ctrl+Tabs each stepped one tab: 7 from pages, held in the shell, and 8 quick ones from the terminal (A B C A B C A B). None of the hand presses rolled (finding 5). |

## Findings

1. **Finding 30 of the 6q-3a live check is fixed.** A Ctrl+Shift+Tab whose Tab lands first switches between the terminal and the side pane, from the terminal and from a page, with real keys, in 30 of 30 presses. The fix is in 0dc4e7f, cf745cb, a0d82db and f20432d, with failing-first tests in `tabHold.test.ts` and `browser::rules`.
2. **A window in the background doesn't get the keys back from a page.** Over CDP, with the user's browser in front, a Ctrl+Shift+Tab from the terminal entered Page C. The way back moved the DOM focus to the terminal, but the page's webview kept the native focus (`GetGUIThreadInfo`), so the app still counted the page as having the keys. The next Ctrl+Shift+Tab then picked the terminal again, which looked like nothing happening; a Shift-first one did the same. With the window in front, all 30 round trips worked. Not changed. Real keys only reach the window in front, so this limits only checks that drive a background window over CDP: a check that enters a page needs the window in front.
3. **Entering a page with Ctrl+Shift+Tab gives the page the chord's key-ups.** From the terminal, the page takes the keys while Tab, Shift and Ctrl are still down, so it hears their releases (10 of 10). The shell clears its taken keys when a page gets the keyboard, by design, so that a release that went elsewhere isn't swallowed later. A Shift-first Ctrl+Shift+Tab does the same, so this isn't new. Not changed.
4. **The app's 50 ms timer ran about 10 ms late in the dev build.** The held Ctrl+Tab went out 60.3 to 60.6 ms after the press (3 runs). That is within decision 14's "about 50 ms". Not changed.
5. **No hand press rolled.** In each of the user's 20 Ctrl+Shift+Tab presses, Shift was already down when Tab went down. From the terminal the app heard Shift 22 to 423 ms before Tab, or held over from the press before. From a page the shell read Shift as down at Tab's key-down, so it held none of them. So the hand round confirms that the ordinary chords still work, and the rolled case rests on the 30 scripted real-key presses.
6. **Outside this slice: a session with no folder ran in the real user folder.** "Start PowerShell 7 with no agent in an unknown folder" started the shell in `C:\Users\Home Office`, not the scratch `USERPROFILE`, and the palette then listed files from there. Nothing was opened or written. A live check should give its sessions a folder in the scratch home.

## Re-check after the fix pass (2026-11-14)

The final review's fix pass changed both holds (eaeb19e9, 2a7671d8, 33e3465a), so the real-key part ran again.

- Build: `ctrl-tab-rollover` at 77aa8ff8 under `pnpm tauri dev`, between 14:52 and 21:13 local time, with the user and the dev window in front.
- Home and data: a second set of scratch homes, the same key-logger site on loopback, one PowerShell 7 session and the three pages. No agent CLI was started.
- How it was driven: the same guarded `SendInput` script, 16 presses in all.

| Check | Result |
| --- | --- |
| Rolled (Shift 10 ms after Tab, Tab up at 60 ms) | Pass, 10 of 10: 5 from the terminal and 5 from a page. Each switched between the terminal and the side pane, and the tab stayed on Page C. From a page the shell's 2 ms look sent Ctrl+Shift+Tab 1 to 6 ms after Shift went down, about 45 ms before Tab's release. The page heard no Tab key-down. The app heard the Tab, Shift and Ctrl key-ups afterwards, and the navigation hold ended. |
| Shift tapped and let go before Tab (Shift down at 10.2 ms, up at 25.4 ms, Tab up at 60.6 ms) | Pass, 4 of 4: page, terminal, page, terminal. Each switched panes, and the tab stayed on Page C. From a page the app got Ctrl+Shift+Tab 8 to 12 ms after Shift's key-down, before Shift came up. This is the case eaeb19e9 fixed: before it the shell read Shift at Tab's release, when it was already up, and stepped the tab. |
| Plain Ctrl+Tab from a page | Pass, 2 of 2. Tab held 80 ms went out at the time-out, 61 ms after Tab, and stepped C to A. Tab up at 20 ms went out 8 ms after the release and stepped A to B. |

Two things happened around the run that are not results of it:

- **The first try at the tap round sent nothing.** Steam, then FACEIT (which runs elevated), took the front. The script's foreground guard stopped one run, and Windows refused `SendInput` in the other (UIPI). No key was sent and none was left held. The round above ran after the user cleared the desktop.
- **The real `~/.claude/settings.json` changed during the check, and the check didn't change it.** Its modified time moved from 2026-11-13 00:35:57 to 2026-11-14 15:57:59. The check's scratch `CLAUDE_CONFIG_DIR` stayed empty and no `claude.exe` ran in the dev app's process tree. The file differs from the setup repo's copy only in keys a Claude Code session writes for itself (the model, the editor mode, the full-screen setting, remote control at startup), so another Claude Code session wrote it. The file was left as it is.
