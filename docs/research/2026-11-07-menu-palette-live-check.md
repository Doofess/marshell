# Menu and palette live check (slice 6p)

- Date: 2026-11-07, 15:06 to 15:16 local time.
- Build: `phase-6p` at b17c22f under `pnpm tauri dev` on Windows 11 Pro (10.0.26300).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `MARSHELL_CLAUDE_SETTINGS` under `%TEMP%\lc-6p-1`, with CDP on port 9333. Onboarding was skipped for every agent ("Skip: use limited status"), so no hooks were installed.
- Data: none seeded. One Claude Code session was started from the launcher in the scratch `proj` folder; it stopped at Claude's first-run theme prompt and was never answered.
- How it was driven:
  - **Input.** CDP evaluations and CDP keys throughout, plus CDP mouse clicks on the sidebar's and the rail's Menu buttons, inside the webview only.
  - **Forced colours.** Emulated, not a real high-contrast theme.
  - **Window width.** Item 6 changed it with Playwright's `setViewportSize`, an emulated viewport rather than a real window resize.
  - **Restart.** Item 7 needed the app restarted, so `src-tauri/src/main.rs` was touched (its modified time only, no content change), and `tauri dev` rebuilt and restarted the app before any session existed.
  - **Screenshots.** Four, kept outside the repo history in `.playwright-mcp/lc-6p-1/`: `footer.png`, `submenu.png`, `submenu-forced.png` and `rail-forced.png`.

## Results

| Check | Result |
| --- | --- |
| 1. The footer shows Menu, then the ring, the 5h % and the Doctor count. | Pass for what a scratch home shows. The 40 px footer holds `sidebar__menu` (28 px, at x 14), the spacer and the Doctor button ("Doctor: no issues", ✓ 0). The ring and the 5h % need plan data, which a scratch home has none of, so they did not show; their order is pinned by `Sidebar.test.tsx`. |
| 2. Ctrl+Shift+M opens the menu, and the arrows, typeahead, Right, Left and Esc behave as decision 10 says. Esc returns focus to the terminal. | Pass. It opens above the button (menu bottom 743, button top 747) with "Settings" focused, and the button reads `aria-expanded="true"`. Down, Up, Home and End move. "d" goes to Doctor and "d" again to Departures. Right or Enter on Settings opens the six-item "Settings" submenu with "Appearance" focused, to the right of the menu (left 196 against the menu's right edge at 194). Left or Esc goes back to "Settings", and Esc again closes. With a session focused, focus goes back to `xterm-helper-textarea`. With no session it goes back to the page body, which is where it was before. |
| 3. Each menu item opens its place. "History" focuses History's search box. | Pass. "History" shows the History view with focus in "Search conversations". "Doctor" and "Departures" open their dialogs with focus inside. Settings › "Shell profiles" (reached by typing "s") opens Settings on "Shell profiles" with its heading focused. Picking "History" again while History is shown closes the menu and puts focus back. |
| 4. On the rail, the button at the bottom opens the menu above itself. | Pass. After "Collapse sidebar to the rail", `.rail__menu` (28 px, top 746) sits outside `nav.rail__nav` in the rail's last 40 px. A click opens the menu with its bottom at 742, and the button says `aria-expanded="true"`. |
| 5. In focus mode, Ctrl+Shift+M opens the menu at the bottom left. | Pass. Neither the sidebar nor the rail renders, so there is no button, and the menu opens at x 8 with its bottom 8 px above the window's (772 of 780). |
| 6. Shrinking the window below 960 px while the menu is open leaves focus somewhere real, never the page. | Pass. The menu was opened from the sidebar button, then the width went to 900 px. The sidebar became the rail, the menu stayed open, and focus stayed on "Settings". Esc then sent focus to the terminal, because the button it came from was gone. |
| 7. The empty palette starts with Recent after a command was run from it, and Recent survives an app restart. | Pass. After "Keyboard shortcuts" was run from the palette, the empty box opened with "Recent" first and "Keyboard shortcuts" active. `layout.json` held `"recent_commands": ["cheatsheet"]`. After the restart (new app PID), the empty box still led with Recent › "Keyboard shortcuts". |
| 8. No Settings or History rows show with an empty box, and "appear" finds "Settings: Appearance…". | Pass. With no session, the empty box had 15 rows under Session, Terminal, Window and App, with no Settings, History, Doctor, Departures or Mode rows. "appear" listed only "Settings: Appearance…". |
| 9. "Change mode…" opens the Modes list for a Claude Code session. Esc goes back, and Esc again closes. | Pass. With session 1 focused, the empty box shows "Change mode…" and no Mode rows. Running it shows one "Modes" group with the five Mode rows, an empty input labelled "Type a mode", and a listbox named "Modes". Esc returned to the commands with an empty box ("Type a command, or a session number"), and Esc again closed the palette with focus back on the terminal. |
| 10. With no session focused, "Close session" is not listed. | Pass. It is absent with no session and listed once session 1 is focused. |
| 11. Ctrl+Shift+Alt+F does nothing, and Ctrl+Shift+Y opens History with the search box focused. | Pass. Ctrl+Shift+Alt+F from the terminal opened no view or overlay. Ctrl+Shift+Y from the search box went back to the terminal, and pressed again it opened History with "Search conversations" focused. The Keyboard shortcuts sheet lists "Open menu" and no Alt+F binding, under Navigation, Session, Terminal, Window and App. |

Other things seen:
- **The Menu button as a toggle.** A second click on it closes the menu, and focus stays on the button.
- **The menu key inside the menu.** Ctrl+Shift+M pressed in the open menu closes it, with focus back on the terminal.
- **The palette's "Open menu".** It opens the menu once the palette has closed, and Esc returns focus to the terminal.
- **Forced colours.** The menu and the submenu take a CanvasText border, and the focused item a Highlight fill.

## Findings

1. **The Menu button reopened the menu instead of closing it, and the menu key did nothing inside the menu.** I found both while reading Task 6's diff, before the check.
   - **Why:** a press on the button closed the menu on pointerdown, or on blur, and then the button's click opened it again. Separately, the menu stops key propagation, so Ctrl+Shift+M never reached the window's handler.
   - **Fixed in b17c22f.** `leavesAppMenu` leaves a press on the toggle button to its click, and `menuKeyCloses` closes the menu from inside it.
   - **Tests:** `a_press_on_the_menu_button_leaves_the_closing_to_its_click` and `the_menu_key_pressed_in_the_menu_closes_it`, both seen failing first. The check above confirmed both live.
2. **Ctrl+Shift+M does nothing while focus is in a text box, such as History's search box.** History takes focus into that box when it opens.
   - **Why:** this is the window's existing rule. A text box keeps its keys, and only Ctrl+Shift+Y is exempt. Ctrl+Shift+K behaves the same there. Tab or Ctrl+Shift+Y leaves the box.
   - **Not fixed.** Exempting the menu key would open the menu over the launcher's fields, and would blur the rename box, which commits the rename.
3. **The buttons' `aria-keyshortcuts` reads "Ctrl+Shift+M" (⌘E on macOS), the shortcut label, rather than ARIA's "Control+Shift+M".** The existing context menus already do the same, so this follows them. Not fixed.
4. **Ctrl+Shift+I (focus mode) logs "permission: deny-internal-toggle-devtools" in the webview console.** Tauri refuses its devtools toggle for that chord. This predates the slice and is outside it. Not fixed.
