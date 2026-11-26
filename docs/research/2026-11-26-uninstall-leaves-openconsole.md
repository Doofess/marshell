# An uninstall with the app open leaves OpenConsole.exe (issue #212, slice 9b)

- Date: 2026-11-26.
- Where: the release workflow's dry runs on GitHub's Windows runners, and NSIS 3 on the development PC (stand-in files in the temp folder only; nothing was installed or uninstalled there).
- Outcome: the cause is confirmed and fixed in the uninstaller's last hook. The installer has the same gap and is left as issue #214.

## What failed

The dry run for alpha.2 on `main` (`48970ea5`, run 45848380904) failed in the installer smoke test:

```
PASS A7 uninstall: the app has ended
FAIL A7 uninstall: the install folder holds no file: left OpenConsole.exe
PASS B4 uninstall: the install folder holds no file
RESULT 56 passed, 1 failed, 0 skipped
```

Cycle A uninstalls while the app is open; cycle B stops the app first. The last green dry run was slice 6u's on 2026-11-23 (run 45176375145), and no pull request since touched the installer. What changed is the app: since slice 6v-1 (2026-11-25) the sidebar's shell starts with no session open, so a freshly started app has a terminal, and with it an `OpenConsole.exe` running from the install folder.

A dry run before that one (45837513717, same commit) stayed in the smoke step for 45 minutes until the runner lost its connection, and left no log. Whether that was the same fault is not known.

## The order in Tauri's uninstaller

Read from the template of the CLI the project uses (`@tauri-apps/cli` 2.12.1; `crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi` and `utils.nsh` at tag `tauri-cli-v2.12.1`). `Section Uninstall`:

1. `NSIS_HOOK_PREUNINSTALL`.
2. `CheckIfAppIsRunning` for the main exe only: the Restart Manager finds the processes that use `marshell-app.exe` and, when silent, shuts them down with the force flag.
3. `Delete` for the main exe, every resource and `uninstall.exe`; `RMDir "$INSTDIR"`.
4. Shortcuts and registry.
5. `NSIS_HOOK_POSTUNINSTALL`.

No hook sits between steps 2 and 3. The `Install` section has the same shape: `NSIS_HOOK_PREINSTALL`, `CheckIfAppIsRunning`, then `File` for each file.

## Why the file is left

The app's own process ends in step 2. The programs it started end after it: on a normal exit the app ends its sessions and then lets its children go (`release_children`, `src-tauri/src/main.rs`), and when it is killed its Job Object ends them (`crates/core/src/job.rs`). Either way `OpenConsole.exe` can still be running for a moment when step 3 starts, and Windows doesn't delete the file of a running program. That moment was not watched directly: it is what the two results below fit. A silent NSIS `Delete` that fails says nothing and goes on, and `RMDir` then leaves the folder because it isn't empty. Nothing tries again.

Tried on the development PC with a small NSIS program and files held open by the test itself:

| What the silent program did | Result |
| --- | --- |
| `Delete` of a file that is held open | The file stays, the error flag is set, the program goes on and exits 0. |
| `File` over a file that is held open | The old file stays, the error flag is set, the program goes on and exits 0. |

## The evidence from a runner

The first commit of the slice only made the smoke test say what runs from the install folder. Its dry run (45853118012, pull request #213):

```
PASS A7 uninstall: the app has ended
NOTE A7 once the uninstaller has ended, running from the install folder: nothing
FAIL A7 uninstall: the install folder holds no file: left OpenConsole.exe; running from the folder 30 s later: nothing
NOTE B4 once the uninstaller has ended, running from the install folder: nothing
PASS B4 uninstall: the install folder holds no file
RESULT 56 passed, 1 failed, 0 skipped
```

By the time the uninstaller had ended, `OpenConsole.exe` was no longer running, and its file was still there. A second try at the file would have deleted it.

## The fix

`MARSHELL_REMOVE_HELD_PROGRAMS` in `src-tauri/windows/hooks.nsh`, inserted in `NSIS_HOOK_POSTUNINSTALL`: it tries each of Marshell's six program files again every 250 ms until none is left or 20 s have passed, then removes the folder if it is empty. It names the files, so nothing else in the folder is deleted or waited for, and it looks at files in the install folder only, so another program's `OpenConsole.exe` (Windows Terminal runs one per tab) doesn't matter.

`scripts/release/hooks-check.ps1` builds a program that runs that macro on a folder (with a 3 s limit) and holds stand-in files open itself:

| Case | With a macro that tries once | With the fix |
| --- | --- | --- |
| Nothing in use | Pass | Pass: the files and the folder are gone, no wait. |
| A file freed after 1.2 s | Fail: "left OpenConsole.exe" | Pass: gone, and it stops waiting well before the limit. |
| A file never freed | Fail: "took 31 ms" where the limit was expected | Pass: it gives up after 3 s, exits 0, and only that file is left. |
| `uninstall.exe` held and `notes.txt`, neither Marshell's program | Pass | Pass: both stay, no wait. |

The pull request's dry run with the fix (45855845988, at `4d3dceb3`) is the acceptance check. At that commit the release build job ran the check above, and it passed on the runner as well (`RESULT 11 passed, 0 failed`, with the NSIS Tauri had just downloaded). That step was taken out again afterwards: a change to CI is the user's to decide. The smoke test:

```
PASS A7 uninstall: the app has ended
NOTE A7 once the uninstaller has ended, running from the install folder: nothing
PASS A7 uninstall: the install folder holds no file
RESULT 57 passed, 0 failed, 0 skipped
```

### Where in the hook it runs

The wait was built as the hook's last step, after `marshell uninstall --yes` has taken the hooks out of the agent CLIs, and the run above had it there. The pre-merge review led to a fault in that order. Tauri's uninstaller removes the uninstall entry before the last hook starts, and taking the hooks out lasts seconds, or up to a couple of minutes when a WSL distro has to start. A person who waits until `winget list` no longer shows Marshell and then installs again, as the release notes advise, could have the fresh install's programs deleted by a try that late. Before this slice nothing touched the folder after the entry was gone.

The wait now runs first in the hook, a moment after the uninstaller's own delete. What is left of that gap is the time the wait itself takes: nothing when no file was in use, and 20 s at most. The dry run on the commit that moved it is in pull request #213's checks; it is the evidence that the helpers end within the limit without the extra time the unhook used to give them.

## Not fixed here

- **The installer.** A silent install over a running Marshell (`winget upgrade`, the smoke test's step A5) can skip `OpenConsole.exe` or `marshell-host.exe` in the same way and still exit 0. The template has no hook between closing the app and copying the files; a later hook could only carry a second copy of each program and would need the same wait. The bundled ConPTY, `marshell-host.exe`'s code (`crates/host`) and its protocol (`crates/protocol/src/host.rs`) have no commit since `v0.1.0-alpha.1`, so a kept file does no harm in alpha.2. Issue #214.
- **An administrator session's helper.** `marshell-host.exe` in an elevated session isn't closed by the installer or the uninstaller. The uninstaller now waits 20 s for it, gives up, and leaves its files, as before.
- **The dry run that hung.** No log survived it. The smoke step's 15-minute limit didn't end it.
