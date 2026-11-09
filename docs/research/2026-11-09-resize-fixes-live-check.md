# Resize fixes live check (after slice 6q-0)

- Date: 2026-11-09 23:31 to 2026-11-10 00:15 local time.
- Build: `resize-fixes` under `pnpm tauri dev` on Windows 11 Pro (10.0.26300), xterm 6.0.0 with the WebGL renderer, one display at 100% scale and 144 Hz (frames about 7 ms apart).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `MARSHELL_CLAUDE_SETTINGS`, `CODEX_HOME` (with a copy of the real `auth.json`, deleted at the end), `COPILOT_HOME` and `USERPROFILE`, with CDP on port 9333. Onboarding was left with every CLI on "Skip: use limited status".
- Data: none seeded. Three sessions started by the check: PowerShell 7, Claude Code 2.1.321 (left at its first-run theme picker, behind its login) and Codex 0.164.0 (left at "Trust this folder?", unanswered).
- How it was driven: CDP evaluations and CDP keys and clicks in the webview; a page init script that counts blank frames (below); the window resized in steps with `SetWindowPos`, and once shown and minimized again with `ShowWindow`, which send no mouse or keyboard input and check the window's process first; emulated colour scheme, forced colours and device scale; 4 screenshots. The user dragged the real window's edges and dividers for the hand checks.

## How the frames were counted

The probe wraps `requestAnimationFrame`, `ResizeObserver`, the canvas's `width` and `height` setters and the WebGL draw calls. A **blank frame** is one where a WebGL canvas had its size set (which clears it) inside the frame's rendering work and nothing drew on it before the next frame began. It also counts synchronized-output blocks (DEC mode 2026) on the pty sockets. A second hook on xterm's renderer notes, at each draw, the row a known line of the program's screen is on; a **jumped frame** is one drawn with that line on another row than the one it rests on.

One run is 80 steps: the window shrinks by 280x200 in 40 steps of 16 ms and grows back. Other runs move only the bottom edge, or the left and top edges.

To prove the probe sees the flicker, a temporary edit, never committed, forced the fallback path (`main`'s next-frame fit). It counted 86 blank frames in 80 steps with a shell, and the app logged "Marshell: xterm no longer has RenderService._renderRows; the terminal falls back to the next-frame fit and may flicker while resizing."

## Results

| Check | Result |
| --- | --- |
| No blank frame with a shell (PowerShell 7) | Pass. 0 blank frames in 80 steps, against 86 on the fallback path. |
| No blank frame with Claude Code's TUI | Pass, for its first-run screen only. 0 blank in 160 steps. It sent no synchronized-output blocks; its main UI is behind the login and wasn't reached. |
| No blank frame with Codex's TUI | Pass. 0 blank in 240 steps while Codex sent 675 synchronized-output blocks, and again 0 in 240 steps with the final code. |
| No blank frame while a program holds synchronized output | Pass after finding 2. A loop in the shell that holds the block open half the time: 0 blank in 80 steps. |
| No blank frame at a fractional display scale | Pass after finding 3, emulated only. 0 blank in 160 steps at an emulated 125% and 0 in 80 at 150%. An emulated scale isn't a true one (the canvas's backing size equals its CSS size), so a real 125% or 150% display wasn't checked. |
| The window's background follows the theme | Pass. Emulated light sent `{"rgb":[255,255,255]}` and dark `[0,0,0]`; setting and removing `data-theme="light"` sent the same pair; in emulated forced colours the forced colour was sent. `[0,0,300]` was refused with "invalid args `rgb` for command `set_window_background`". The native colour can't be read back, so the strip itself rests on the hand check. |
| Hand check: a shell's text doesn't flicker (window edges, the sidebar's divider; the main terminal and the bottom-left shell) | Pass. The user: "Flicker gone". |
| Hand check: Codex's TUI | Pass after finding 4. First: "no flickers but draggin from the bottom and the top sometimes makes the text jump up and down". After the fix: "Jump is gone". |
| Hand check: no white strip at the window's moving edge | Pass. The user: "Strip is invisible". |

## Findings

1. **The same-frame draw removes the blank frames.** Fallback path 86 blank frames in 80 steps, first fix 0 (`58de8c7a`, tests in `syncRedraw.test.ts`).
2. **A resize inside a synchronized-output block left the canvas empty.** xterm's private `_renderRows` only notes the rows while a program holds DEC mode 2026, and ConPTY passes the mode through. Codex: 1 blank frame in 160 steps (254 blocks). A shell loop holding the block open half the time: 63 blank in 80 steps, up to 3 in a row. The user chose a draw in the middle of the update over a blank frame; the terminal now asks xterm's renderer itself then, and both runs gave 0 (`96d12ea6`, tests "asks the renderer itself while a program holds synchronized output" and two more).
3. **At a fractional scale the WebGL addon set the canvas's size a second time, after the draw.** At an emulated 125%: 86 blank frames in 80 steps with the first fix alone. A second device-pixel observer on the canvas, made after the addon's, brought it to 1; that one came from a fit made outside the observer, so every fit now goes through the same draw path: 0 in 160 steps (`96d12ea6`, tests "draws again when xterm sets the canvas's size once more after the draw" and three more).
4. **A full-screen program's text jumped when the terminal lost rows.** Codex draws on the alternate screen and parks its cursor on the last row. When rows go and the cursor is on the last row, xterm drops rows from the top, so the text moved up a row and came back when Codex repainted, 3 or 4 frames later. `main` has it too: 16 jumped and 22 blank frames in 80 bottom-edge steps on the fallback path. With the same-frame draw the blank frames showed the shifted text instead: 35 jumped, 0 blank. The user chose to fix it here. Before a fit that loses rows, the terminal moves the alternate screen's cursor onto the new last row, where xterm puts it anyway, so the rows go from the bottom: 0 jumped and 0 blank frames in 240 steps (bottom edge, top edge, corner), and the screen ended as Codex drew it (`05c0414b`; `topAnchor.test.ts` runs the installed xterm, starting with the control test "is needed: xterm alone drops a full-screen program's top rows when its cursor sits on the last one").
5. **In forced colours a canvas doesn't resolve the `Canvas` keyword to the forced colour.** It gave white in both schemes, while the body's computed background was the forced colour. The window's colour is read from the computed background only (`96d12ea6`).
6. **The app doesn't set xterm's `windowsPty` option.** `term.options.windowsPty` is `{}`, so xterm's ConPTY-aware resize paths are off. Not changed here; a lead for the resize-polish slice.
7. **A shell's or an inline TUI's top rows still move when rows go.** With Claude Code's first-run screen (the normal screen), its first line scrolled into the history as the window shrank. That is a terminal's usual behaviour, not a jump back and forth, and it is untouched. Not fixed.
8. **A CDP click on a session row can leave the pointer over the row, so the peek takes the keys.** One typed loop was lost that way during the check. Moving the pointer onto the terminal and clicking it first avoids it. A note for later live checks, not a product change.

## Not checked

- A real 125% or 150% display scale, and a real dark high-contrast theme (both emulated only).
- The DOM renderer (WebGL unavailable or its context lost).
- Claude Code's main UI, behind its login.
- The history player and the scripted terminal, which keep their own fits.
- Full-screen programs other than Codex with the top anchor (vim, less): until the program repaints, its bottom rows are cut instead of its top rows.
