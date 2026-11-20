# Resize-polish live check

- Date: 2026-11-20.
- Build: `resize-polish` under `pnpm tauri dev` on Windows 11 Pro (26300), WebView2 157. The "before" numbers were taken on `resize-polish-spike`, which is `resize-polish` at its plan commit plus the two spike blocks of S13; the blocks touch nothing that is measured here.
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `USERPROFILE` and `MARSHELL_CLAUDE_SETTINGS`, with CDP on port 9333. Onboarding was skipped with "Skip: use limited status" for every agent, so nothing was installed.
- Data: two PowerShell 7 sessions with no agent, started for the check in the scratch project folder, and the shell panel's own PowerShell.
- How it was driven: CDP evaluations and CDP keys in the webview; the window resized in steps with `SetWindowPos`, which sends no mouse or keyboard input and checks the window's process and path first. The user dragged the real window's edges for the hand checks.

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
