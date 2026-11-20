# Resize-polish live check

- Date: 2026-11-20.
- Build: `resize-polish` under `pnpm tauri dev` on Windows 11 Pro (26300), WebView2 157. The "before" numbers were taken on `resize-polish-spike`, which is `resize-polish` at its plan commit plus the two spike blocks of S13; the blocks touch nothing that is measured here.
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `USERPROFILE` and `MARSHELL_CLAUDE_SETTINGS`, with CDP on port 9333. Onboarding was skipped with "Skip: use limited status" for every agent, so nothing was installed.
- Data: two PowerShell 7 sessions with no agent, started for the check in the scratch project folder, and the shell panel's own PowerShell.
- How it was driven: CDP evaluations and CDP keys in the webview; the window resized in steps with `SetWindowPos`, which sends no mouse or keyboard input and checks the window's process and path first. The user dragged the real window's edges for the hand checks. For a page's keys, which need real input, a guarded script clicked inside the check's own window and pressed one chord, after checking the window under the point and the foreground window; the user had allowed that kind of test. One emulated forced-colours pass (not a real high-contrast theme), one screenshot.
- Runs: three starts of the app. The baseline (01:30 to 02:05), the checks on the whole-rows build (02:23 until the PC was shut down at about 02:39), and the rest on 2026-11-20 from 10:17 to 10:44.

## The measures

**M1, layout lag.** The window is 1200x780 with the shell panel open. Its height goes from 780 to 620 and back in 80 steps of 4 px, one every 33 ms. A `ResizeObserver` on the document element, made after the app's own, reads the shell panel's height in the frame the window's new size is laid out, before it is painted. A step is stale when that height isn't the one the layout rule gives for the window's height. A `MutationObserver` on the sidebar's style says when the right height does arrive.

The plan's first sampler read the two heights in an animation-frame callback. That runs before the frame's layout, so it counts every step as stale whatever the app does, and it was replaced by the one above.

**M2, part row.** The same window at twelve heights, 780 down to 560 in steps of 20. At each, once the terminal has settled: the gap is the shell terminal's body height, less the terminal's own padding (16 px), less the height of its rows.

**M3, `windowsPty`.** A PowerShell session prints `1..200 | % { "line $_" }`. The window then shrinks by ten terminal rows and grows back, 20 times with 220 ms between sizes and 20 times with 70 ms. Afterwards the terminal's buffer is read: lines of the 200 that are missing, lines that are there twice, and blank lines between them. Once as the app is, and once on a second session with `term.options.windowsPty = { backend: "conpty", buildNumber: 22000 }` set before the resizes.

## Before

| Measure | Result |
| --- | --- |
| M1 | 80 of 80 steps stale. In every step the panel still had the last step's height when the frame was laid out (off by 1 px a step here; a drag moves further per frame). The right height arrived after that frame's paint and before the next frame, 80 times of 80. |
| M2 | The gap runs from 1 px to 16 px: 16, 10, 4, 15, 9, 3, 14, 8, 2, 13, 7, 1. A row is 17 px, so the panel shows up to 16 px of a row that isn't there. |
| M3, as is | 0 lost, 0 doubled, 0 blank, over 80 resizes between 42 and 32 rows. The prompt ended on the last row, where it started. |
| M3, `windowsPty` set | 0 lost, 0 doubled, 0 blank, over 80 resizes. But the prompt ended ten rows above the bottom with ten empty rows under it, and ten more lines in the scrollback: xterm kept the rows it got back as new empty ones. |

`windowsPty` is not better than the app as it is, and leaves the prompt in the wrong place, so the slice doesn't set it (decision 4's condition; the plan's Task 5 doesn't run).

## By hand, before

The user dragged the real window's edges on the spike build (the two blocks are S13's; `docs/spikes/S13-window-buttons-strip.md` has that part).

| Check | Result |
| --- | --- |
| Right edge and top-right corner, slow and fast | The app's own window buttons trail the edge. With a browser page open in the side pane too. |
| Left edge, slow and fast | The window buttons stay still. |
| Bottom edge: the shell panel and the bar under it | "It looks fine." The user saw no jump in the bar, no jumping text and no gap under the last row. |

The numbers above say the panel is a frame late and shows part of a row, and the eye didn't mind either. So the check after the change has to ask the opposite question as well: whether a panel that now changes a whole row at a time looks worse than one that changed a pixel at a time.

## After

The slice first made the shell panel as tall as whole terminal rows (decision 1). The hand check below reversed that, so there are two "after" builds: the whole-rows build, and the final one, where the panel is sized by the pixel again and only the same-frame measure is left.

| Measure | Whole-rows build | Final build |
| --- | --- | --- |
| M1 | 0 of 80 steps stale. The panel changed 4 times, each in the frame of the resize that caused it. | 0 of 92 steps stale (M1's 80, then M2's 11 and one back). The panel changed in every step, by the pixel, in that step's own frame. |
| M2 | Gap 0 at all twelve heights (11, 11, 11, 10, 10, 10, 9, 9, 9, 8, 8, 8 rows). | The gap is as before the slice: 16, 10, 4, 15, 9, 3, 14, 8, 2, 13, 7, 1. |
| M3 | Not repeated: `windowsPty` was not set (Task 5 did not run). | |

No "ResizeObserver loop" line and no console error or warning in either run.

## The checks

C1 to C3 were run on the whole-rows build before the PC was shut down. C4 to C8 were run the next morning, C4 before and after the fix it led to. C1 to C3 were not repeated on the final build.

| Check | Result |
| --- | --- |
| C1, M2 at an emulated 125% | On the whole-rows build the gaps read 0 or 1 px (under 1 px in truth) and the rows were as meant. It found finding 1. |
| C2, the panel folded and opened, and the app started folded | Pass on the whole-rows build: whole rows once the terminal had drawn, never 0. The rule it checked is gone with decision 1. |
| C3, the font raised from 13 to 15 with the panel open | Pass on the whole-rows build: one fit, whole rows of the new size, gap 0. The rule it checked is gone with decision 1. |
| C4, 1300 px wide, the pane beside the terminal | Pass. Esc in a text tab's body moves the keyboard to the terminal and nothing is said. Ctrl+Shift+Tab does the same and says the cursor's receipt, "Selected session 2, PowerShell 7 2", as on `main`. The pane stays. |
| C4, 1000 px wide, the pane covering | Esc: pass. The pane hides, `document.activeElement` is the terminal's textarea, and the announcer says "The side pane is hidden. Ctrl+Shift+Q shows it." once. Ctrl+Shift+Tab: failed first (finding 2), pass after the fix: "Selected session 2, PowerShell 7 2. The side pane is hidden. Ctrl+Shift+Q shows it." once. |
| C5, the same with the selected session ended | Pass. Esc and Ctrl+Shift+Tab hide the pane, each says its line once, and the console logs nothing. |
| C6, the palette open while the height steps as in M1 | Pass. 80 steps, 0 stale, no React warning, no "ResizeObserver loop" line, and the palette's field kept the keyboard and took typing afterwards. |
| C7, the divider focused, then the sidebar folded | Pass. The divider was `document.activeElement` at every sample through the motion and after it. Right moved the pane from 525 to 509 px and Left back to 525; at the widest (541 px) Left does nothing. The `sidebar` command has no default key, so the fold was fired by the title bar button's click from script, which leaves focus where it is. |
| C7, the divider focused, then the window narrowed to 1000 px | Pass. `document.activeElement` was the text tab's editor, inside `[data-nav-side]`. |
| C8, forced colours (emulated) | Pass. The branch changes no stylesheet. The focused divider shows a 2 px system-colour outline and the shell panel draws as before. Not compared side by side with a `main` build. |

## By hand, after

| Check | Result |
| --- | --- |
| Top- and bottom-edge drags with the shell panel open, whole-rows build | "Worse: steps distract." The user found the panel changing a row at a time worse than the smooth resize. Asked what the panel should do, they chose the smooth resize back, with the part row it can show. The final build does that. The user has not dragged the final build. |
| Esc in a page of a browser tab, the pane covering | The user answered "Pane stayed", for Ctrl+Shift+Tab too. Their window was 1966 px wide when it was read afterwards, where the pane is beside the terminal and both keys only move the keyboard. Run again by the guarded script with real input at 1000 px: Esc in the page hid the pane, said "The side pane is hidden. Ctrl+Shift+Q shows it." once, and left the terminal's textarea active. |
| Ctrl+Shift+Tab in the page, the pane covering | By the same script: the pane hid, the announcer said "Selected session 1, PowerShell 7. The side pane is hidden. Ctrl+Shift+Q shows it.", and the terminal's textarea was active. |

During the hand session the user resumed session 2 and started Claude Code in session 1 of the check's window; it stopped at its login screen under the scratch `CLAUDE_CONFIG_DIR`. The script's last step typed one "x" to see where the keyboard had gone, and that key went to this Claude Code's login menu, which showed no change. The user was told, and agreed to it being stopped with the check. The real `~/.claude/settings.json` has the modified time it had before the first start.

## Findings

1. **A display scale that changes with no window resize left the terminals at their old rows.** At an emulated change from 100% to 125% the shell panel kept 8 rows of 17.6 px in a body made for 17 px rows, with 5 px of the last row cut, and the main terminal did the same. Fixed in `e9672156`: a terminal fits again when the scale changes. With the fix the panel went 8 to 7 rows and back, and the main terminal 29 to 28 and back. No failing unit test first: the change is an effect, and Vitest has no DOM.
2. **Ctrl+Shift+Tab's hide of a covering pane was silent.** The cursor's receipt and the hidden line went to the one announcer in the same update, and the receipt won. Fixed in `45049b37`: the two are one notice (`receiptWithPaneHidden` in `paneCommands.test.ts`, failing first).
3. **Whole-row steps look worse on a drag than a part row does at rest.** The numbers said the old panel showed up to 16 px of a row that isn't there, and the eye never minded; the fix for it was what the eye minded. Decision 1 is reversed in `f0731344`.
4. **Narrowing the window until the pane covers, with the keyboard in the terminal, leaves the keyboard on nothing.** A covered terminal is suspended, so focus falls to the page's body. It is so on `main` as well. Not fixed: it is one of the other paths decision 4 leaves for later.
5. **An ended session's first prompt line lost its text after the width changed.** After 1300, 1000 and 1200 px the line that held `marshell open notes.txt` showed only its first letter. Seen once, not looked into. Not fixed.
6. **The launcher once opened with the keyboard still in the terminal behind it.** Seen on the second run while the window had been restored behind every other window without being activated. On the third run the launcher took the keyboard within 200 ms in a plain window, at 1000 px with the pane hidden, and at 1000 px with the pane covering, and Esc gave it back where it was. Not reproduced, not fixed.
7. **Editing files under the running app can leave the dev server with a broken module.** After a worker rewrote `Sidebar.tsx`, a reload showed a blank page ("does not provide an export named 'Sidebar'") until the files were touched again. A dev-server matter, not the app's.

## Not fixed

- The sidebar's bottom bar shifts for a frame on bottom-edge drags, by WebView2's late frame.
- The address bar's buttons trail on left- and right-edge drags, for the same reason.
- The window buttons trail the right edge. S13 ended go for a native strip, which is its own slice.
- The shell panel can show up to 16 px of a part row under its last line (decision 1, reversed).
- Findings 4, 5 and 6.
