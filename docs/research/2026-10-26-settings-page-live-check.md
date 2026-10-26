# Settings page live check (slice 6e1)

- Date: 2026-10-26, 22:50 to 23:00 local time.
- Build: `phase-6e1` under `pnpm tauri dev` on Windows 11 Pro (10.0.26300).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `MARSHELL_CLAUDE_SETTINGS`, with CDP on port 9333. Onboarding was left with "Skip: use limited status" for every CLI, so nothing was installed.
- Data: `tools.json` written into the scratch home after start, with one valid entry and one broken entry: `{"version":1,"tools":[{"id":"lc-good","name":"Live good","commands":["lc-good-marshell-test"]},{"id":"lc-typo","name":"Live typo","comands":["x"]}]}`. Windows Terminal's own settings supplied the "Match my terminal" family (`JetBrainsMono Nerd Font`), read only. One PowerShell 7 session, started from the launcher. No agent session.
- How it was driven: CDP evaluations and CDP key events from a small Node script, because the `webview` MCP server failed to connect this session. The tray was driven by emitting its `tray-action` event inside the webview, which exercises the webview's routing but not the native menu. Forced colours were emulated with `Emulation.setEmulatedMedia` (not a real high-contrast theme). Three screenshots were taken. No desktop input.

## Results

| Check | Result |
| --- | --- |
| The bundled fonts load | Pass. `document.fonts` lists JetBrains Mono, Cascadia Mono and Atkinson Hyperlegible Mono at 400 and 700, each `loaded` with `display: block`. `document.fonts.check` is true for all three, regular and bold. |
| Each choice changes the terminal's cell width; Match my terminal gives Windows Terminal's family | Pass. xterm's `fontFamily` follows each pick, and Match my terminal resolves to `"JetBrainsMono Nerd Font", "JetBrains Mono", monospace`. The canvas advances at 13px are 7.8 (JetBrains Mono and the Nerd Font), 7.617 (Cascadia Mono) and 8.216 (Atkinson). xterm rounds the cell down, so the cell is 7px for the first three and 8px for Atkinson, which reflows the main terminal from 210 columns to 184. |
| Ctrl+Shift+1 with Settings open closes it and focuses session 1 | Pass. The page closed, focus went to the terminal, and session 1 is selected. |
| The palette's six old commands and "Settings…" open the right sections | Pass. "Shell profiles…" opens Shell profiles, "Appearance…" Appearance, "Agents…" Agents, "Diagnostics…" Advanced, "Notifications…" Notifications, "History settings…" History, and "Settings…" Appearance. Each time focus is on the section's `h2`. |
| The tray's "Settings…" opens the last section shown | Pass. Opened at History, switched to Agents, closed with Esc; the tray event `settings` reopened at Agents. |
| Esc closes the page, and focus returns to the terminal | Pass, for all seven palette openings and for "Back to sessions". |
| Esc inside the own-tool remove question doesn't close the page | Pass. The question closed, the page stayed, and focus went to that row's Edit, as in own agents. |
| Esc inside the "Another font…" field doesn't close the page | Pass. The typed "Fira" was reverted and the page stayed. Leaving the empty field went back to Match my terminal. |
| A tray `goto` closes Settings | Pass. The tray event `session-1` closed the page and focused the terminal. |
| Saving an own tool writes `tools.json`, and the new preset appears in the launcher's Tools group | Pass. "Add your own tool" with a preset saved "Live added", announced "Saved Live added", and left `tools.json.bak` beside the file. The broken "Live typo" entry was written back in its raw shape (`"comands"`). "Live preset" is listed last in the launcher's Tools group once Settings closes. |
| A hand edit while the form is open gives the 409 copy | Pass. "tools.json changed since you opened it. Reload to see it." was announced and shown above a Reload button. The form kept its draft ("Live good v2"), and the hand edit stayed on disk. |
| The broken entry is listed with its reason | Pass. "Live typo" shows "It has an unexpected shape (unknown field `comands`, expected one of `id`, `name`, …)". |
| Keyboard-only pass through every section | Pass after fix 1. Every section was reached from the nav with Space. Tab stayed inside the page, never reached a hidden section, and wrapped to "Back to sessions". Every stop showed an outline. |
| Forced colours: Terminal and Agents sections | Pass after fix 2. System colours throughout; the picked font has its Highlight outline. |
| 720 by 480 at text size Larger | Pass. "Back to sessions" (551,56 to 696,90) and the whole nav (to y 439) are in view. The document doesn't scroll, `.page__body` scrolls, and the nav stays at y 124 after the body scrolls 368px. |

Not exercised live: the row's "Status limited · set up" link (no agent session was started) and the shell profiles close refusal. Both are covered by unit tests.

## Findings

1. **Every native radio was a Tab stop on the Settings page.** The page's own Tab trap walked every focusable element, so Appearance had 29 stops and Terminal 18. The browser's own Tab stops once per radio group. The old dialogs had the same trap, so the problem predates the slice. Fixed in 546b270: `visibleControls` keeps only a group's checked radio (else its first) and leaves out controls with a negative `tabIndex`. Tests "stops once on a radio group…" and "leaves out a control taken out of the tab order" failed first. Live afterwards: 14 stops in Appearance and 13 in Terminal.
2. **The nav didn't mark the current section.** `SelectedMark.css` switches on `aria-current="true"` or `data-selected`, and the page's nav sets `aria-current="page"`. So the current section had no fill, no end bar and, in forced colours, no Highlight outline. Fixed in 35a98b6: the current item also gets `data-selected="true"`. The test "draws the shared selected mark on the current section…" failed first. The forced-colours screenshot afterwards shows the Highlight outline on Terminal.
3. **Editing an own tool's name changes its id** ("Id: live-good-v2" for the entry saved as `lc-good`). Own agents behave the same way, since both forms derive the id from the name. Not fixed; listed under Known limits.
4. **Saving rewrites untouched valid entries with their defaults spelled out** (`"scripts": []`, `"presets": []`, and so on). This is the same meaning in a longer form. Broken entries keep their raw shape, as the plan requires. Not fixed.
