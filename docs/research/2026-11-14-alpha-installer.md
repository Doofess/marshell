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

From the release workflow's dry runs on pull request #125 (`build` job, `scripts/release/smoke.ps1 -Installer`). The first run, [43638410084](https://github.com/Doofess/marshell/actions/runs/43638410084), passed without a change to the script: `RESULT 54 passed, 0 failed, 0 skipped`.

The runner: `windows-latest` is Windows Server 2025 (image `windows-2025-vs2026`, version 20261103.298.1). The job runs as `runneradmin`, an administrator.

| Question | What the runner showed |
| --- | --- |
| Is winget there? | Yes. `winget validate --manifest` ran in `build` and printed "Manifest validation succeeded." with exit code 0. The `Install-Module Microsoft.WinGet.Client` fallback in `winget-check` wasn't needed here; it stays for an image without winget. |
| Does the app start (a desktop, WebView2)? | Yes. `marshell-app.exe` started and `marshell list --json` answered within the 90 s wait, in both cycles. The app copied `marshell-hook.exe` and `marshell.exe` into `<home>\bin`. |
| Is the install per-user though the account is an administrator? | Yes. The uninstall key is under HKCU and not under HKLM, and the folder is under `%LOCALAPPDATA%`. `DisplayName`, `Publisher` (Doofess) and `DisplayVersion` (`0.1.0-alpha.0`) are as the manifest says. |
| Does a silent install over a running app work (what `winget upgrade` does)? | Yes. The installer exited 0, the running app ended, and the hooks in Claude Code's file stayed. |
| Does an update by hand keep the hooks (section 1's guard)? | Yes. `uninstall.exe /S _?=<folder>` exited 0, Claude Code's file still named `marshell-hook`, and `<home>\bin\marshell-hook.exe` stayed. So `$EXEDIR` equals `$INSTDIR` in that flow, as the generated script suggested. |
| Does `QuietUninstallString` uninstall with the app open? | Yes. It exited 0 at once (the uninstaller hands over to a copy of itself), the key went within the 120 s wait, the app ended and no file was left in the install folder. |
| What does the uninstall leave? | Claude Code's `settings.json` byte for byte as before the hooks; Codex's `hooks.json` without `marshell-hook`; no `<home>\bin`; `<home>\snapshots` kept; no `marshell://` link into the install folder; no Start menu shortcut. |
| A CLI file that doesn't parse? | The uninstall finished, Claude Code's file was restored, Codex's broken `hooks.json` was byte for byte the same, and `<home>\bin\marshell-hook.exe` stayed. |
| Does the scratch environment reach the uninstaller? | Yes. The unhook ran against the scratch `MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS` and `CODEX_HOME`, which the uninstaller's copy inherited from the script. |

Times: the installer build took 12 min 44 s with a cold cache, the smoke test 23 s. The installer is 13.15 MiB.

Not shown by a dry run, because those jobs run only for a published tag: `winget install --manifest` and `winget uninstall` on a fresh runner (`winget-check`), and `wingetcreate submit` for a package that isn't in the community repository yet (`winget-submit`). The first release, `v0.1.0-alpha.1`, shows both, and what it shows is added to this note.

One thing the fixture check on the development PC showed: Marshell finds Claude Code's hooks file with `MARSHELL_CLAUDE_SETTINGS` and nothing else. `CLAUDE_CONFIG_DIR` moves Claude Code's skill, not its `settings.json`. The fixture refuses to run unless `MARSHELL_CLAUDE_SETTINGS` and `CODEX_HOME` are set, and the smoke test sets both.
