# Window theme live check (soft surfaces)

- Date: 2026-11-20, 10:52 to 10:55 local time.
- Build: `soft-surfaces` under `pnpm tauri dev` on Windows 11 Pro (26300). Windows itself was in dark mode.
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR` and `MARSHELL_CLAUDE_SETTINGS`, with CDP on port 9333. Onboarding was left at step 1; no session was started.
- Data: none.
- How it was driven: CDP evaluations. The theme was changed by setting `data-theme` on the root from script, which is what Appearance's setting does; the Settings page itself was not used. The right-click menu is native, so a guarded script right-clicked inside the check's own window, captured the window's rectangle, read one pixel inside the menu and closed it with Esc. Three captures.

## The report

The user right-clicked the light app and got WebView2's menu in dark.

The cause: WebView2 draws its menu in the theme of the native window. Tauri gives every webview the window's theme when it is made and again when the window's theme changes, and the window's theme was the system's, whatever Appearance said. Only the browser pages were told the app's theme (`browser_theme`).

## Results

| Check | Result |
| --- | --- |
| Appearance follows the system, Windows dark | The menu is dark (41, 41, 41) on the dark app (18, 18, 18). |
| The app set to Light | The menu is light (255, 255, 255) on the light app (246, 245, 243). `prefers-color-scheme` in the app's webview reads light. |
| The app Light, the window sent back to the system's theme by hand | The menu is dark (41, 41, 41) on the light app: the report, on the same build, with the one call undone. |
| Light, Dark, Light, follow system, Light, each by the root's attribute | The window followed each within 800 ms: `prefers-color-scheme` read light, dark, light, dark, light. |
| The page reloaded while the window was Light and the setting follows the system | The window went back to the system's theme: the first send after a load is never skipped. |
| A theme name the command doesn't know (`sepia`) | Refused with "unknown theme: sepia"; the window kept its theme. |

## Findings

1. **The native window never got the Appearance theme.** Fixed by `set_window_theme` and the watcher in `windowTheme.ts` (`windowTheme.test.ts`, failing first; `window_theme.rs` has the name's parse under test).

## Not checked

- Changing the theme through Settings, and Windows changing its own theme while the app runs.
- A browser page's own right-click menu. Pages are in the same window, and get the app's theme from `browser_theme` as before.
