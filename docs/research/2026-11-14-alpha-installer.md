# The alpha installer: what Tauri's NSIS script does

- Date: 2026-11-14
- Purpose: slice 9a ships the first installer. This note records what the generated installer script does, for the installer hooks (`src-tauri/windows/hooks.nsh`) and the smoke test (`scripts/release/smoke.ps1`). The decisions are PLAN §9 "Slice 9a decisions".
- How it was gathered: `pnpm tauri build --bundles nsis --config src-tauri/tauri.release.conf.json` on the development PC (Tauri CLI 2.12.1), then reading `target/release/nsis/x64/installer.nsi` and `utils.nsh`. Line numbers are that file's. Nothing was installed on the development PC; the runs are in section 2.

## 1. The generated script

1. **The helper programs are in the installer.** The `Install` section copies `marshell-hook.exe`, `marshell-host.exe` and `marshell.exe` (lines 648 to 650) beside `OpenConsole.exe`, `conpty.dll` and `licenses\conpty-LICENSE.txt` (645 to 647). The release config's `bundle.resources` was merged with the Windows config's entries, not put in their place. Before this slice the bundle held only the app and ConPTY.
2. **The uninstall key** is `Software\Microsoft\Windows\CurrentVersion\Uninstall\Marshell` (line 62), under HKCU because the install mode is `currentUser` (41, 824). It gets `DisplayName` "Marshell", `DisplayVersion` (the full version string), `Publisher` "Doofess" (36, 684) and `UninstallString` (686). With no `bundle.publisher` the publisher was "marshell", the second part of the app's identifier.
3. **No `QuietUninstallString` is written.** `UninstallString` is `"<folder>\uninstall.exe"` with no switch. The post-install hook adds the quiet one.
4. **The hooks.** `hooks.nsh` is included at the top (line 32), so its macros can use `${UNINSTKEY}`, `$UpdateMode` and `$INSTDIR`.
   - `NSIS_HOOK_PREINSTALL` (633) runs before the running-app check and the file copies.
   - `NSIS_HOOK_POSTINSTALL` (713) runs after the registry values and shortcuts.
   - `NSIS_HOOK_PREUNINSTALL` (758) runs before the running-app check (762) and before any file is deleted, so `marshell.exe` is still there.
   - `NSIS_HOOK_POSTUNINSTALL` (853) runs last.
5. **Which installs run the old uninstaller.**
   - A silent install (`/S`, what winget runs for an install and an upgrade) shows no page, so the reinstall page never runs. The files are copied over the old ones.
   - The reinstall page (`PageLeaveReinstall`, 306) runs the old uninstaller when the person keeps the first choice, "uninstall before installing", which is the one selected at the start (289). It passes `/UPDATE` only when the installer itself was started with `/UPDATE` (353), which is what Tauri's updater does. A newer installer run by hand therefore runs the old uninstaller without `/UPDATE`.
   - That uninstaller is started in place, `uninstall.exe _?=<folder>` (355), and the installer goes on to install afterwards (380). An uninstall started from Add/Remove Programs or by winget has no `_?=`: NSIS copies the uninstaller to the temp folder and runs the copy. So `$EXEDIR` is the install folder only for an uninstall the installer started.
   - The pre-uninstall hook skips the unhook when `$UpdateMode = 1` or `$EXEDIR == $INSTDIR`, and an update by hand keeps the hooks. What it gives up: a person who cancels the installer right after that step has the program removed and the hooks left. `marshell uninstall` from `~/.marshell/bin` still takes them out.
6. **A running app.** `CheckIfAppIsRunning` (`utils.nsh` 24) asks the Restart Manager about `marshell-app.exe` only. When silent it skips the question and calls `RmShutdown` with the force flag (42, 47); if that fails the install or uninstall aborts. `marshell-host.exe` (an administrator session's helper) and a `marshell-hook.exe` that is running at that moment aren't registered, so a file of theirs that is in use isn't replaced or deleted.
7. **App data.** The "Delete application data" box is a page of the uninstaller's window, so a silent uninstall never ticks it. Ticked, it removes `%APPDATA%\dev.marshell.app` and `%LOCALAPPDATA%\dev.marshell.app` (849, 850), the webview's own data. It never touches `~/.marshell` (decision 9).
8. **The installer's file** is `target/release/bundle/nsis/Marshell_<version>_x64-setup.exe`, 13.2 MiB for this build.

## 2. Runs on a clean runner

Filled in by the release workflow's dry runs (plan Task 6).
