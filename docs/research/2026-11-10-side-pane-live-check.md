# Side pane live check (slice 6q-2)

- Date: 2026-11-10, 02:45 to 03:06 local time.
- Build: `phase-6q-2` under `pnpm tauri dev` on Windows 11 Pro (10.0.26300), xterm 6.0.0 with the WebGL renderer, one display at 100% scale.
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `MARSHELL_CLAUDE_SETTINGS`, `CODEX_HOME`, `COPILOT_HOME` and `USERPROFILE`, with CDP on port 9333. No agent CLI was started; the real `~/.claude/settings.json` kept its modified time.
- Data: a scratch project folder with `README.md`, `docs/notes.md`, `docs/README.md`, `main.rs`, `pic.png` and `doc.pdf`, written by a script. Two PowerShell 7 sessions started by the check in that folder. A scratch web server on loopback for check 6.
- How it was driven: CDP evaluations, keys and clicks in the webview; a page probe that counts the resize messages on the pty sockets, samples the layout each frame and counts blank terminal frames (the method of `2026-11-09-resize-fixes-live-check.md`, proved here with a forced clear that counted 1); the window resized in steps with `SetWindowPos`, which sends no mouse or keyboard input and checks the window's process first; emulated forced colours, reduced motion and device scale; 4 screenshots. The app was restarted three times by touching a Rust file's modified time. One Ctrl+click opened the scratch server's page in the system browser, as check 6 asks.

## Results

| Check | Result |
| --- | --- |
| 1. Tabs per session and restart | Pass. Session 2 held `README.md`, `pic.png` (an image) and `doc.pdf` (a canvas), session 1 held `notes.md`. Switching sessions swapped the strip with 0 pty resizes. After a restart both sessions came back as "Ended last time" with their tabs and their selected tab (`doc.pdf`, `notes.md`), the pane open, and `layout.json` holding `open: true`, `width: 721`. The window reopens at 1200 px, so the pane showed at its widest there (321 px) with 721 still saved. |
| 2. Every way in | Pass for the parts a script can reach. Ctrl+click on a path in output: the hint reads "Ctrl+click to open in the side pane", a plain click does nothing, the announcement reads "Opened README.md in the side pane." and focus lands in "README.md, contents". Quick open ("Type a file name"). The + menu "New tab" with Open file…, Browse… and the recent rows ("name — folder"); Esc returns focus to +. The empty pane: "No tabs open in this session.", Open file…, Browse…, "Recent in this project", "Drop a file here to open it.", with Tab going + → Hide the side pane → Open file… → Browse… → the recent rows. A second open of an open file selects its tab. Not done: Browse… and drops from Explorer, which are the user's hand checks. |
| 3. Width | Pass after findings 1 and 3. The divider moves 16 px a key, Home gives 320, End gives exactly 80 terminal columns, a drag stops at 80 columns, and a double-click returns to half. At a 1100 px window the pane is 320 px (65 columns); at 1000 px it covers the terminal (320 → 712 px in about 190 ms, 0 pty resizes); the saved 576 px returned at 1600 px. |
| 4. Motion, measured | Pass. Show and hide take 180 to 190 ms; the sidebar to the rail 174 ms and back 173 ms; focus mode on 177 ms and off 152 ms; across 960 px the sidebar (288 → 52) and the pane (712 → 320) move together. Each switch sent 1 pty resize and drew 0 blank frames. Two toggles 80 ms apart: 286 ms in all, the motion turns round from where it was, 0 resizes (the size ended where it began). With reduced motion: one frame, 1 resize. |
| 5. Keys | Pass. Ctrl+Shift+Tab goes from the terminal to the pane and back. Ctrl+Tab steps the tabs with focus staying in the terminal. In the strip: Left, Right, Home, End, Alt+Shift+arrows ("Moved main.rs to position 3 of 4."), Delete ("Closed notes.md."; closing the last tab puts focus on +). Esc in a tab's body returns to the terminal. |
| 6. Web address | Pass. Over `http://127.0.0.1:<port>/` the hint reads "Ctrl+click to open in your browser"; a plain click sent no request; Ctrl+click brought one `GET /` from the system browser. Over an OSC 8 hyperlink the hint names the address ("Ctrl+click to open http://127.0.0.1:47650/osc8 in your browser") and clears when the pointer leaves. |
| 7. Forced colours and a 125% scale | Pass after finding 2, emulated (not a real high-contrast theme). Screenshots of the strip with a selected and a focused tab, and of the empty pane. At an emulated 125% the 80-column stop follows the scale (the widest pane 705 px against 721, cells 7.2 px wide); the screenshot tool resets the scale, so the screenshots are at 100%. A real 125% display wasn't checked. |
| 8. An old file | Pass. A `layout.json` in `main`'s shape (`arrangement`, `ratios` and `zoomed` present, no `pane`, the sidebar as a 304 px rail, session 1 selected) loaded with the rail, the 304 px width and session 1 selected. The pane was hidden; shown, it read "No tabs open in this session." with no recent files. The next save wrote `pane` and dropped the three fields. |
| 9. The user's judgement | Open: the motion's feel and the strip's look. |

## Findings

1. **The 80-column stop was at 89 columns.** The floor multiplied 80 by the font's measured glyph (7.8 px), but xterm's WebGL renderer draws cells of whole device pixels (`floor(glyph × scale) / scale`, 7 px at 100%). Fixed in `c29e5b32`: `columnsWidth` takes the screen's scale and rounds the cell the same way (`columns.test.ts`, "a cell is whole device pixels" and four more, failing first). Under xterm's DOM fallback renderer cells keep the glyph's width, so the stop lands near 71 columns there. Not fixed: the fallback is rare and the layout doesn't know a terminal's renderer.
2. **In forced colours the selected tab's title couldn't be read, and its focus ring didn't show.** The system painted its text backplate behind the title (white on white), and the ring was `CanvasText` inside the `Highlight` fill. Fixed in `d21f4bdc`: the selected tab opts out of the backplate, and its ring is `HighlightText`, set in from the edge (`SidePane.css.test.ts`, two tests failing first).
3. **After a restart the 80-column stop came one step early.** The floor was measured before JetBrains Mono had loaded, so it counted the fallback font's wider glyph: the divider's widest was 320 px, not 321, at a 1200 px window. Fixed in `41089413`: `useTerminalFloor` measures again when the page's fonts are ready and each time one loads (`useTerminalFloor.test.ts`, four tests failing first). After the next restart the divider read 320 to 321.
4. **When the window narrows until the pane covers the terminal while the divider has focus, focus falls to the page.** The divider goes away with the split. Not fixed.
5. **Typing can reach a covered terminal.** With the pane covering, a terminal that kept focus still takes keys. Ctrl+Shift+Tab moves focus into the pane. Fixed after the review for the one path this slice added: Ctrl+Tab that shows the hidden pane over the terminal now moves focus into the pane (`paneCommands.test.ts`, "paneTabFocus", failing first). Not fixed: in a covering pane Ctrl+Shift+Tab and Esc still hand the keyboard to the terminal underneath, as they did with the Preview.
6. **Recent files were kept by the row's project name.** Two folders with the same name shared one list. Fixed after the review: the list is kept by the repository's root (`paneCommands.test.ts`, "projectKeyOf", failing first).
7. **The empty pane's recent rows cut a long folder path at its end**, where the folders that tell two rows apart are. Not fixed.
8. **Small hitches in the dev build.** The first show of the pane had one 52 ms gap between frames, and expanding the sidebar and leaving focus mode one gap of about 22 ms each. Not measured in a release build.
9. The console showed only the dev server's favicon 404 and Tauri's `deny-internal-toggle-devtools` on Ctrl+Shift+I.

## After the review

A second run, 03:26 to 03:32, with a new scratch home and two PowerShell 7 sessions in two folders both named `proj`, checked the fixes that need the real page:

| Check | Result |
| --- | --- |
| Ctrl+Tab from inside a tab's body | Pass. With focus in `README.md`'s body, `main.rs` showed and focus was in its body, not in the hidden one. |
| Ctrl+Tab from the terminal | Pass. The tab changed and focus stayed in the terminal; showing the hidden pane beside the terminal (a 1200 px window) left focus in the terminal too. |
| Ctrl+Tab that shows the pane over the terminal | Pass. In a 1000 px window with the pane hidden and focus in the terminal, the pane showed covering and focus was in the shown tab's body. |
| Recent files of two folders with one name | Pass. After two files were opened in `other\proj`, the session in `proj` showed an empty pane with no recent files, and `layout.json` held the full folder as each recent file's project. |

Not run again: the drop behind a dialog (a drop needs a hand) and the oversized line number, which their unit tests cover.
