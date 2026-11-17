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

Not shown by a dry run, because those jobs run only for a published tag: `winget install --manifest` and `winget uninstall` on a fresh runner (`winget-check`), and `wingetcreate submit` for a package that isn't in the community repository yet (`winget-submit`). Section 3 has what the first release showed.

One thing the fixture check on the development PC showed: Marshell finds Claude Code's hooks file with `MARSHELL_CLAUDE_SETTINGS` and nothing else. `CLAUDE_CONFIG_DIR` moves Claude Code's skill, not its `settings.json`. The fixture refuses to run unless `MARSHELL_CLAUDE_SETTINGS` and `CODEX_HOME` are set, and the smoke test sets both.

## 3. The later dry runs and the first release (2026-11-14 to 2026-11-16)

| Run | What | Result |
| --- | --- | --- |
| [43639873043](https://github.com/Doofess/marshell/actions/runs/43639873043) | Dry run 2, pull request #125, with the added check before the uninstall | `RESULT 55 passed, 0 failed, 0 skipped` |
| [43641744963](https://github.com/Doofess/marshell/actions/runs/43641744963) | Dry run 3, pull request #125, after the review's fixes: the unhook runs after the running-app check, the uninstall runs the exact `QuietUninstallString`, the installer carries `LICENSE` and `THIRD_PARTY_NOTICES.md` | 55 passed, 0 failed, 0 skipped |
| [43928986084](https://github.com/Doofess/marshell/actions/runs/43928986084), [43934722937](https://github.com/Doofess/marshell/actions/runs/43934722937) | Dry runs on `main` before the tag (674e5dc3, f9bfb91f) | 55 passed, 0 failed, 0 skipped, both |
| [43942007171](https://github.com/Doofess/marshell/actions/runs/43942007171), attempt 1 | The tag `v0.1.0-alpha.1`: `build` | The smoke step did not end. It was cancelled after 39 minutes, and GitHub kept no log for the job, so what it waited on isn't known. Nothing was published. |
| The same run, attempt 2 | `build`, `publish` | Smoke test 55 passed, 0 failed, 0 skipped, in 25 s. The pre-release was published with the installer, its `.sha256` and the manifest folder's zip. |
| The same run, attempt 2 | `winget-check` | `winget install --manifest` did not return. The job's 30-minute limit ended it. `winget-submit` was skipped. |

### Why winget did not return

A probe workflow on a throwaway branch (draft pull request #128, never merged) installed the published release with winget 1.11.510 under a 4-minute limit and printed the processes, the visible windows and a screenshot.

| Question | What the runner showed |
| --- | --- |
| Where does winget stop? | It downloads the installer, prints "Successfully verified installer hash" and "Starting package install...", and waits. The uninstall key never appears: the installer has not started. |
| What is it waiting on? | Microsoft Defender SmartScreen's dialog, "Windows protected your PC. Microsoft Defender SmartScreen prevented an unrecognized app from starting", with "More info" and "Don't run" ([run 43959477047](https://github.com/Doofess/marshell/actions/runs/43959477047), the screenshot is in its artifact). `smartscreen.exe` and `CHXSmartScreen.exe` are running, and Explorer owns an `ApplicationFrameWindow` and a `ProxyModalWindow` that were not there before. |
| Why does the dialog come up under winget and not in the `build` job? | The file winget downloads carries the internet mark (`Zone.Identifier`, `ZoneId=3`, with the release's address), and winget starts it through the shell. The `build` job's installer was built on the runner and has no mark. |
| Does turning SmartScreen off by policy help? | No. With `EnableSmartScreen=0` and `SmartScreenEnabled=Off` the shell shows its own dialog, "Open File - Security Warning", owned by `winget.exe`, and winget waits on that ([run 43960558467](https://github.com/Doofess/marshell/actions/runs/43960558467), variant `policy`). |
| Does `SEE_MASK_NOZONECHECKS=1` in winget's environment help? | Yes. The shell skips the check, and winget prints "Successfully installed" and exits 0 after 22 s (the same run, variant `nozonechecks`). |
| Does `winget uninstall --manifest` work without more switches? | No. With `--disable-interactivity` it stops at the Microsoft Store source's agreements and exits 0x8A150046 ("One or more of the source agreements were not agreed to"), even after an install that passed `--accept-source-agreements`. It needs `--accept-source-agreements` itself. |

### The smoke test through winget, after the fix

`scripts/release/smoke.ps1` now sets `SEE_MASK_NOZONECHECKS=1` for winget and what winget starts (the app and the fixture run without it), passes `--accept-source-agreements` to `winget list` and `winget uninstall`, and runs every program with a time limit and its output in files. A program still running at its limit is stopped together with what it started.

The probe ran the script from the fix's branch against the published release, on a fresh runner with the fixture from the release's build ([run 43961947372](https://github.com/Doofess/marshell/actions/runs/43961947372)). Both attempts ended `RESULT 47 passed, 0 failed, 2 skipped`: the first at f3c24f50, with the variable set for the whole script, the second at 4d3fa89f, with it set for winget alone. The two skips are the reinstall and the in-place update, which only the installer mode runs. The one later change to the script lowers the limit on winget's install from 600 s to 300 s. The release's own `winget-check` job runs only on `main`, so it runs after the merge.

| Question | What the runner showed |
| --- | --- |
| Does `winget install --manifest` install per-user, and does the app start? | Yes, in both cycles: the key under HKCU, none under HKLM, `marshell list --json` answers. |
| Does `winget uninstall --manifest` run the unhook? | Yes. The command exited 0, the key went, the app ended, the install folder was empty, and Claude Code's `settings.json` was byte for byte the known one. |
| Does winget pass the shell's environment to the uninstaller? | Yes. The unhook worked on the scratch `MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS` and `CODEX_HOME`, so the release notes' advice to uninstall from the shell that sets a CLI's folder holds. |
| Does winget know the package by its id after an install from a manifest folder? | No. `winget list --id Doofess.Marshell --exact` exited 0x8A150014 (no package found), as `docs/RELEASING.md` says. |

### The check run by hand, without the fixture

After the fix merged (444ea9b9), `gh workflow run release.yml -f action=check -f tag=v0.1.0-alpha.1` ran `winget-check` on `main` ([run 43970750829](https://github.com/Doofess/marshell/actions/runs/43970750829)). winget installed the release, the app started and winget uninstalled it, but the smoke test ended `RESULT 20 passed, 1 failed, 5 skipped`. The failed check was "A8 after uninstall: the home snapshots folder is still there".

A check run by hand has no `build` job, so it has no fixture, and the smoke test skips its hook checks. The snapshots folder holds the backups of files Marshell changed, and in the smoke test only the fixture changes a file. Without the fixture nothing made the folder, so the check asked for something that was never there. It had not run this way before: the tag's own `winget-check` gets the fixture from `build`, and the probe took it from the tag's run. The check now runs only with the fixture.

With that merged (bccd3e69), the same command passed ([run 43975803189](https://github.com/Doofess/marshell/actions/runs/43975803189)): `RESULT 20 passed, 0 failed, 5 skipped`. The five skips are the fixture's steps and checks, cycle B, and the two steps only the installer mode runs. So the hooks' removal through winget is shown by the probe's runs with the fixture, and not by a check run by hand.

### The submission

The token was added on 2026-11-17, and `gh workflow run release.yml -f action=submit -f tag=v0.1.0-alpha.1` ran `winget-submit` ([run 44038920114](https://github.com/Doofess/marshell/actions/runs/44038920114)).

| Question | What the runner showed |
| --- | --- |
| Does `wingetcreate submit` (v1.28.1.0) take a manifest folder for a package that isn't in the repository yet? | It validates it and opens the pull request. It printed "Manifest validation succeeded: True" and opened [microsoft/winget-pkgs pull request 482075](https://github.com/microsoft/winget-pkgs/pull/482075), "New version: Doofess.Marshell version 0.1.0-alpha.1", from the owner's account, with the folder's three files. |
| Does it need anything but the token? | No. The token came from `WINGET_CREATE_GITHUB_TOKEN`, a classic token with `public_repo`, and the job took 41 s. |
| Was the pull request reviewed? | No. `microsoft-github-policy-service[bot]` closed it 46 s after it opened (07:28:42Z to 07:29:28Z), without a comment or a label. Nothing is waiting with Microsoft. |

Why the bot closed it isn't known. What the repository shows about that hour:

- Every pull request opened up to 07:24:25Z (482067 to 482073) stayed open and got the bot's usual first comment, the validation badges.
- Every pull request opened from 07:27:27Z to 07:35:13Z (482074 to 482078) was closed by the same bot within 29 to 46 s, with no comment and no label. They came from first-time and from established contributors, with "New package:" and with "New version:" titles.
- The rule in the repository's own policy files that closes a pull request when it opens (`.github/policies/labelManagement.issueOpened.yml`, for one that changes no file) also posts a reply. None of these got one.

That points away from Marshell's manifest and its title, and at something on Microsoft's side that changed or broke at about 07:25Z. The submission has to be run again once new pull requests there stay open.

Two things about the pull request itself, for the next run (issue #133 tracks both, and the notes' sentence below):

- Its title said "New version:". The repository's template asks for "New package: Publisher.Name version X.Y.Z" for a package that isn't there yet. `release.yml` passes the title, so the first submission of a package needs the other one.
- Its body was the repository's template with nothing ticked. The checklist asks for the contributor agreement, and for `winget validate` and `winget install --manifest`, both of which `winget-check` runs.

The release page of alpha.1 has a line, added by hand, that the package isn't in winget's repository yet. `scripts/release/notes.mjs` doesn't write it. What it writes is "If winget can't find this version yet, Microsoft is still reviewing it", which is false while no submission is open, so on alpha.1's page that sentence was shortened by hand too. Until the generator's sentence is reworded (issue #133), each new release page needs the same two edits. The hand-added line goes when Microsoft has the package.

Still not shown: what a person sees after "More info" and "Run anyway" when winget started the installer (nobody can press it on a runner); a submission that reaches Microsoft's review, and whether they accept an unsigned alpha; whether an install from the community repository meets the dialog; why the first attempt's `build` job stopped; and the tag's own `publish`, `winget-check`, `winget-submit` chain passing in one run, which alpha.2 will show.
