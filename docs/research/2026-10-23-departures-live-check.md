# Departures live check (slice 4c)

- Date: 2026-10-23, finishing at 02:40 local time.
- Build: `phase-4c` under `pnpm tauri dev` on Windows 11 Pro (26300).
- Home: a scratch `MARSHELL_HOME` in the session's temp folder, CDP on port 9333, notifications off and onboarding skipped in the scratch `settings.json`, `close_to_tray` on.
- Sessions: three Claude Code sessions started for the check (real `claude.exe` with a scratch `CLAUDE_CONFIG_DIR`, left at its first-run screen, never typed into) and one plain shell. Agent states came from Claude Code hook bodies posted to `/v1/hook` with the scratch home's token. Held permission requests ran as hidden `node` processes, stopped by PID at the end.
- How it was driven: the webview over CDP only: evaluations returning small JSON, CDP mouse and key events inside the webview, emulated media for reduced motion, forced colours and a small viewport, and the tray action emitted as the tray's own `tray-action` event. No desktop mouse or keyboard input was sent.

## Results

| Check | Result |
| --- | --- |
| Row flash | Pass after two fixes (findings 1 and 2). A row going needs-permission and a turn ending (Working → Done) each set `data-flash` for about 600 ms, and the row now plays `row-flash` for 0.6 s. Done → Working does not flash; the first snapshot after a reload does not flash. |
| Open from the palette | Pass. "Departures" in the command palette opens the board inset 40 px from the top, the rows match the sidebar, the gate group comes first, and focus lands on the first gate row. |
| Live flip | Pass. A status cell going WORKING → DONE flipped over about 156 ms of frames; its screen-reader text was the final word at once. With `data-motion=reduce` no letter frames ran and the cell crossfaded (`data-fade`) for about 200 ms. |
| Hold under the pointer | Pass. With the pointer over the board, a session going needs-you stayed in Others with "Moves up when you move away", and a closed shell's row stayed, dimmed (`data-stale`). When the pointer left, the session moved to the gate and the closed row dropped. |
| Keys | Pass after a fix (finding 3). Y opens the approve card with focus on it; Escape returns focus to the board row; N focuses Deny; ArrowDown moves; Tab stays inside the board; Enter jumps to the session's terminal (its xterm textarea) and closes the board. Escape on the board returns focus to the element that opened it. |
| Title | Pass. "(3) api-server — Marshell" with three requests waiting, matching the gate. |
| Tray | Pass. The tray model sent to `set_tray` lists "Open Departures" second; the tooltip read "Marshell · 3 need you". The `tray-action` event with `{ id: "departures" }` opens the board with focus on the oldest waiting row. |
| All sessions | Pass. At a 190 px viewport the board showed "+3 more sessions"; "All sessions" closed the board and focused the palette input. |
| Forced colours | Pass, emulated (not a real high-contrast theme). System colours throughout, a Highlight outline on the active row, framed status cells. |

## Findings

1. **`motion.css` was never loaded by the app.** `src/main.tsx` imported every stylesheet in `src/styles/` except `motion.css`, which only Storybook imported. The `data-flash` attribute toggled, but no stylesheet held `row-flash`, `g-bounce` or any vendor loop, so no row flash, glyph bounce or working-mark animation has run in the app since phase 0. Fixed in ec9fc00, with `src/styles/entryImports.test.ts` checking that the entry imports every sheet in the folder. Live afterwards: needs-permission glyphs run `g-bounce` and a finished turn's row plays `row-flash` for 0.6 s.
2. **A finished turn did not flash.** `toRow` maps the core's Done to done-seen (the live app has no "seen" yet), and the flash only counted done-unseen. Fixed in f7b9e0d: done-seen counts, except when it comes from done-unseen.
3. **Escape on a board card left focus on `<body>`.** The row unmounted when its card opened, so there was nothing to return to. Fixed in 4c3e683 (the row stays mounted beside its card).
4. **Not seen live: error.** The core never sets `LiveState::Error`, so the error flash and the error glyph cannot appear yet. Covered by unit tests only.
5. **Not seen live: the drawn check on Done.** The check stroke (`g-draw`) runs only for done-unseen, which the live app never produces (finding 2). This predates 4c.
6. **An expired hold reads "Has a question" on a terminal-only entry.** When a permission hold ran out, the board's one-line entry (the lane's `LaneRow`) said "Has a question". This comes from the lane's existing labelling in `laneEntries`, not from 4c.
7. **Very short windows show no rows.** At the smallest heights the board fits 0 rows and shows only "+N more sessions" and "All sessions".
8. **A motion change applies on the next render.** The board reads the reduced-motion setting when the app renders, so changing it while the board is open takes effect on the next update.
9. **Check method: the tray event needs an object payload.** Emitting `tray-action` with a bare string did nothing; `{ id: "departures" }` is what the tray sends.

## Real config

The check used only the scratch home and a scratch Claude config folder, sent no input to any agent CLI, and installed no hooks. `~/.marshell` kept its date (2026-10-21T19:42:54). `~/.claude/settings.json` was written at 01:12:58 by the user's own Claude Code sessions running beside the check; it holds no Marshell entry.
