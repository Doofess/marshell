# Window buttons strip live check

- Date: 2026-11-20, 12:55 to 13:31 local time.
- Build: `window-strip` under `pnpm tauri dev` on Windows 11 Pro (26300), one 3440-wide display at 100%. Windows itself was in dark mode.
- Home: a scratch `MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `COPILOT_HOME` and `USERPROFILE`, with CDP on port 9333. Onboarding was finished with "Skip: use limited status" for every agent. One plain PowerShell 7 session was started for the overlay checks; no agent session was started.
- Data: none.
- How it was driven: the page through CDP (evaluations, key presses, clicks inside the webview). The strip is a native window that a CDP capture doesn't show, so it was read from PowerShell with Win32 queries (`FindWindowEx`, `GetWindowRect`, `IsWindowVisible`, `SendMessage(WM_NCHITTEST)`) and a `BitBlt` of its own device context. No mouse or keyboard input was sent to the desktop: where a check needed a button to act, the strip was sent its own action message (`WM_APP`), after its class and owning process were checked.

## What the check compares

The strip shows pictures the page paints of its own three buttons. "Match" below means a pixel-by-pixel compare of two captures of the same 138 by 40 rectangle: the page's own buttons (a CDP capture, which leaves the strip out) and either the strip's window (`BitBlt`) or the pictures `paintStrip` hands to the shell.

## Results

| Check | Result |
| --- | --- |
| The strip exists and sits right | One child window of the main window, class `MarshellWindowStrip`, 138 by 40, at the client area's right edge, above the webview. `set_window_strip` ran with no console error. |
| C1, the pictures match the page, at rest | 0 of 5,520 pixels differ in dark, light, dark with High contrast, light with High contrast and emulated forced colours. The hairline row is part of the compare. |
| C1, hover | 0 of 5,520 differ in the same five looks, with the page's own buttons given the hover look and the strip's hover pictures. |
| C1, the strip's own window | 0 of 5,382 pixels differ between the `BitBlt` of the strip and the page (dark, at rest; the rows above the hairline). |
| C2, scale | Not checked. An emulated scale (`Emulation.setDeviceMetricsOverride`) lays the page out in CSS pixels and rounds there, where a scaled display lays out in device pixels: at an emulated 200% the page put its icon one device pixel lower than the same arithmetic gives on a real one. So an emulated compare says nothing about a real display. See finding 4. |
| C3, the middle button, by the strip's action | Maximized: the strip is at the right edge (3302 to 3440 of 3440), shows Restore, 0 of 5,382 pixels differ from the page. Again: restored, Maximize, 0 differ. |
| C3, by the page's button with the keyboard | Enter on Maximize: 3440 by 1392 and the button reads "Restore". Enter again: 1200 by 780. |
| C3, a double click on the bar, Win+Up, Win+Down | Not checked: they need real input. |
| C4, the edges | Not checked: the user's hands. |
| C5, Close | The strip's Close action with a session open brought the page's first-close question; the strip's window was hidden under it. Escape hid the window to the tray, and the app kept running. |
| C5, real clicks with the keyboard in the terminal, in a browser page, and with another app in front | Not checked: they need real input. |
| C6, overlays | The strip is hidden (the page counts as covered) and nothing reaches its rectangle for the app menu and its Settings submenu, the command palette, the launcher, Doctor, and a session row's context menu. `Peek` doesn't cover by design and sits clear of the strip (295 to 855 across, 68 down, in a 1200-wide window). Onboarding covers since finding 2. |
| C6, read from the code, not opened | Toasts sit at the bottom-start corner. A file drag marks only the terminal and the side pane. The all-sessions menu is mounted by no app screen. Settings, the close question, the shortcuts sheet and the tray menu each cover the page. |
| C7, the keyboard alone | Tab to Minimize and to Maximize: `:focus-visible` is on, the ring is a solid 2 px outline, and the strip's window is hidden. After Shift+Tab off the buttons the strip is back in place and visible. |
| C8, before and after the page | The strip's window is made hidden and shows only with its first pictures. A reload of the webview sent new pictures (the 0-pixel compare above was taken after one). After the window came back from the tray the strip was in place and visible. |
| Task 6, the hit test | `WM_NCHITTEST` across the strip reads client, maximize button, client (`1, 9, 1`), at the top row and in the middle. |
| Task 6, the Snap Layouts flyout | Not checked: it needs a real pointer over Maximize. This is the gate for closing #23. |
| Task 7, Win+Left and Win+Right | Not checked: real input. The main window's styles read `GWL_STYLE` `0x14CF0000` and `GWL_EXSTYLE` `0x40110`. |

## Findings

1. **The plan's top band had nothing under it.** The plan had the strip pass its top rows through (`HTTRANSPARENT`) to a top-edge resize window of Tauri's. Under Tauri's `unstable` feature the app's webview is a child webview, and Tauri makes that resize window only for a window's own webview, so the band handed its points to the page's buttons instead: a click there worked, and the hover never lit. The band is gone; the strip takes the pointer over its whole height, as the page's buttons do, and the top edge resizes on the row above the client area, as on `main`.
2. **Onboarding lay over the bar with the strip floating on it.** Onboarding covers the whole window, bar included, but didn't count as covering the page. It now calls `useCoverPage`, and the scan test knows the layer (`coverScan.test.ts`, failing first).
3. **The pictures were off the page's by 114 of 5,382 pixels.** Three causes: the page paints an icon's box on whole device pixels (14.5 down becomes 15), it cuts the icon to its box (the ends of Close's strokes), and a canvas made with `willReadFrequently` draws a thin diagonal stroke differently from the page. The glyph's origin is rounded as the page's, the stroke is cut to the box, and the canvas is an ordinary one (`tiles.test.ts`). 0 differ since.
4. **At a fractional scale the layout was measured from the wrong end.** 138 CSS pixels are 172.5 device pixels at 125%. The page's buttons end at the window's right edge and each edge is rounded from there, so the pictures are now 57, 58 and 57 wide (they were 58, 57, 58). The icon's box gets the same treatment: each of its edges goes to a whole device pixel on its own (a 12.5-pixel icon becomes 12 or 13 wide) and the drawing is fitted to the box. Both are reasoned from how the page lays out, not measured: see C2.
5. **The performance panel was under the strip.** Found by the branch's review, not by this check: the panel (Ctrl+Alt+Shift+P) was fixed in the window's top-right corner and covers nothing, so the strip lay over its first lines. It now starts below the bar (`perfOverlay.css.test.ts`).
6. **The rest look was read from a button the pointer can be on.** Also from the review: with the strip hidden the page's own Minimize is under the pointer, and a read then took its hover colours for the rest look. A third hidden probe gives the rest look now, and the probes are found by their marks (`tiles.test.ts`). Not seen live.
7. **The check's own script was wrong twice.** It marked the page's buttons with the mark the strip's hover probes carry, so the looks reader took a real button for a probe (a whole-button "mismatch" on Close's hover), and Playwright's own screenshot dropped the emulated scale. Both were faults of the script, not of the app. The first showed that `readLooks` relied on the order of the marked elements, which finding 6's change removed.

## Seen on the way, not part of this slice

- The page's own Minimize glyph is blurred over two rows at 100%, on `main` too: its stroke is centred on a pixel boundary. The strip shows the same picture.
- The bar's brand button (`aria-label="Marshell menu"`, `aria-haspopup="menu"`) has no click handler; the menu opens from the sidebar's Menu button and its shortcut.
- The launcher opened by Ctrl+Shift+T about half a second after the palette closed left focus on the palette's button, and Escape didn't close it until focus moved in. Seen twice through CDP keys, not tried by hand.
- While Doctor or Settings is open the buttons are the page's own, since the strip hides under any cover. They lag the edge on a resize there, as they do on `main`.

## Not checked

- Everything marked "Not checked" above: the edges by hand, real clicks, the Snap Layouts flyout, Win+Left and Win+Right, a double click on the bar, and a display at 125% or 150%.
- Real forced colours. They were emulated; whether Windows' own contrast themes read the same through the probes is open.
- Moving the window between two displays of different scale: one display here.
