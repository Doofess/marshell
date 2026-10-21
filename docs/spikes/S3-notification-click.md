# S3: Banner click-to-focus (Windows)

Run 2026-10-21 on the dev box (Windows 11 Pro 26300), first under `pnpm tauri dev` and then on the NSIS per-user
install of the same commit, each with a scratch `MARSHELL_HOME`. Banners came from `tauri-plugin-notification` 2.7.0
through a temporary `debug_banner` command, which was removed afterwards. The banner had the title "Session 3 needs you",
the body ``Wants to run `npm test` <&"'>``, an Open button and `tab_id` in its extras. The main window was minimized
before each banner. Clicks were made with the mouse at the banner's position, because UI Automation can't find the
toast window. The webview listened for `focus-session` over CDP. Only Windows was checked (phase 4 decision 4).

| Check | Result |
|---|---|
| Banner shows, dev build | **Pass**, under the name and icon of **Windows PowerShell**. A dev build has no AUMID of its own, so Windows uses the one from the terminal that started it. |
| Banner shows, installed build | **Pass**, under the name and icon of **Marshell**. The AUMID comes from the Start-menu shortcut the installer creates. |
| Silent | **Pass.** With no sound set, the crate writes `<audio silent="true"/>`. The user didn't notice any sound. |
| Text shown literally | **Pass.** ``<&"'>`` and the backticks appeared as typed. The crate builds the toast XML through the DOM, which escapes text. Task 6 still adds a test that an injected `</text><action …>` stays text. |
| Click on Open | **Pass** in both builds. `on_action` got `tab_id` "tab-s3". The window was unminimized and focused, and the webview got one `focus-session {tab_id: "tab-s3"}`. |
| Click on the banner body | **Pass** in both builds, with the same result as Open. |
| Click on the entry in the notification centre | **Fail (known limit).** After the banner timed out, it stayed in the notification centre under Marshell's name. Clicking it there reported nothing: the window stayed minimized and no second instance started. The plugin's own docs say clicks are reported only while the toast is on screen. |
| Windows 11 do-not-disturb | Windows held the banner back, and nothing popped up. `SHQueryUserNotificationState` still returned `QUNS_ACCEPTS_NOTIFICATIONS` (5). |
| Allow and Deny on a banner | Not run. Slice 4a decision 1 puts them in a later slice. |
| macOS, Linux | Not run. Phase 4 verifies Windows 11 only. |
| Uninstall | **Pass.** The silent uninstall removed the Start-menu shortcut, the install folder and the uninstall registry entry. |

## Findings

1. **`tauri-plugin-notification` is the banner crate.**
   - What it uses: notify-rust 4.18.2 and tauri-winrt-notification 0.8.1 on Windows.
   - Licences: all three are MIT OR Apache-2.0. All three have active upstreams, and none sends telemetry.
   - What it does: it registers action types for the buttons, reports body and button clicks with the banner's extras, and leaves sound to us.
   - Permissions: the webview gets no `notification:` capability, so only the shell shows banners.
2. **Only installed builds carry Marshell's name.** A dev build's banners look like PowerShell's, or like whichever terminal started it, so live checks of how banners look need the installed build. This needs no code change, since releases ship through the installer.
3. **A banner in the notification centre doesn't focus a session.** Once the toast leaves the screen, a click on it is lost.
   - Today the taskbar badge covers this, and 4b adds the tray and the needs-you count in the window title.
   - Possible fix: protocol activation (a `marshell://` link in the toast's launch arguments, handled through single-instance). It can come with the slice that adds Allow and Deny on banners, which needs the same link handling.
4. **Windows do-not-disturb can't be read through `SHQueryUserNotificationState`.** Windows holds banners back during do-not-disturb on its own. Marshell's sounds, which it plays itself, still play. PLAN §9 slice 4a decision 3 records this as a known limit that Marshell's quiet hours cover. The query is still documented to report full-screen apps and presentation mode, so Task 6 keeps it.

Decision: [x] `tauri-plugin-notification`, banners offer Open and Review only, and clicks on the toast focus the session  [ ] deep-link fallback in 4a
