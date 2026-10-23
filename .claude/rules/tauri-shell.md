---
paths:
  - "src-tauri/**"
---

# Tauri shell rules

- `src-tauri/` is a thin application shell. The engine lives in `crates/core`. (AGENTS.md; PLAN.md §1)
- Ask the user before changing the CSP or window settings in `tauri.conf.json` and `tauri.windows.conf.json`. Never loosen the CSP. (SCOPE.md)
- Ask the user before changing anything in `src-tauri/capabilities/`. (SCOPE.md)
- The webview's CSP allows only the app and loopback. Any other request is blocked and shows only as a CSP error in the webview console. (CLAUDE.md, Gotchas)
- Tauri events are OS-level only: notification activated, single-instance argv, tray menu actions. (PLAN.md §3)
- The core decides notifications. The shell receives `CoreEvent::Notify` and shows the banner, sound or flash. (PLAN.md §3)
- Windows banners are sent silent, and the shell plays a system sound instead. (PLAN.md §7)
- `tauri-plugin-store` is dropped; the core owns app settings in `~/.marshell/`. (PLAN.md §Corrections 7)
- Enabling Tauri's native file drop disables in-page HTML5 drag and drop on Windows. (CLAUDE.md, Gotchas)
- Don't start `pnpm tauri dev` or a release build unless the task is a live check or a release. (AGENTS.md)
- A live check uses a scratch `MARSHELL_HOME`, never `~/.marshell`. Stop what you started by PID. (AGENTS.md; CLAUDE.md, Live check)
