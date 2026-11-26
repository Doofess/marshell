# An update over a running Marshell skips a file that is still in use (issue #214)

Slice 9d, 2026-11-26. Plan: `docs/superpowers/plans/2026-11-26-phase-9d-installer-waits.md`; decisions: `docs/PLAN.md` §9, "Slice 9d decisions (2026-11-26)".

## What happened

Tauri's NSIS installer (CLI 2.12.1) closes a running Marshell in `Section Install` with `CheckIfAppIsRunning`, which closes only the main program through Restart Manager, and then copies the files straight away. The programs Marshell started (`marshell-host.exe`, and the `OpenConsole.exe` it starts) end a moment after the app. A file that is still in use can't be written. In a silent install, `File` then skips it and the installer still exits 0, so `winget upgrade` over a running Marshell could leave an old `OpenConsole.exe` beside a new `conpty.dll`.

## What was tried, and how

Every fact below was tried on stand-ins in a scratch folder under `%TEMP%`, with the NSIS that Tauri downloads (`%LOCALAPPDATA%\tauri\NSIS\Bin\makensis.exe`, 3.11). No installer, uninstaller or smoke test ran on the development PC.

| Fact | How it was tried | Result |
|---|---|---|
| The hook runs before the template's own close | Read `target/release/nsis/x64/installer.nsi`: `NSIS_HOOK_PREINSTALL` is inserted at the top of `Section Install`, then `CheckIfAppIsRunning`, then `File "${MAINBINARYSRCPATH}"` (34 MB), then `OpenConsole.exe` and the rest | as read |
| The hook can call the template's close itself | A copy of the template whose hook inserts `CheckIfAppIsRunning`, compiled with `makensis /V2 installer.nsi "/XOutFile NUL"` | exit 0, no setup file written; the template's labels use `${__LINE__}`, so two copies don't clash |
| What "in use" looks like | A small NSIS program calls `CreateFileW(GENERIC_WRITE, share 7, OPEN_EXISTING)` on each kind of file and writes the error from `?e` | running exe 32; file held with `FileShare.Read` 32; read-only file 5; missing file 2; free file opens |
| A loaded DLL | `hooks-check.ps1` loads a copy of `System32\version.dll`, named `conpty.dll`, with `NativeLibrary.Load` | counted as in use: the install check exits 32 and names `conpty.dll` |
| A read-only program file | `File "/oname=ro.txt"` over a read-only file, `SetOverwrite on`, silent | written, and the read-only flag cleared. NSIS's `FileOpen … a` calls that file in use, which is why the probe reads the error code instead |
| The exit code of a silent stop | A silent program that runs `Abort`, then one that runs `SetErrorLevel 7` before it | exit 2, then 7 |
| A line on the standard output | `AttachConsole(-1)`, `GetStdHandle(-11)`, `FileWrite`, with the program started with `-RedirectStandardOutput` | the line is in the file |
| A macro named by a parameter | `!insertmacro ${do} "a.txt"` inside a list macro | runs the named macro once per name |
| Telling a replaced file from a skipped one | `[IO.File]::SetLastWriteTimeUtc` on a running exe; `File` with the default `SetDateSave on`; `WriteUninstaller` | the write time of a running exe can be set; `File` puts back the build's own time; `WriteUninstaller` writes the current time |
| A windowed upgrade removes the old version first | Read `PageReinstall` and `PageLeaveReinstall` in the template | "uninstall before installing" is checked by default and runs the installed uninstaller in place (`_?=`) before `Section Install`; passive mode skips the page; silent mode shows no pages |
| winget and NSIS exit codes | winget-cli `src/AppInstallerCommonCore/Manifest/ManifestCommon.cpp`, `GetDefaultKnownReturnCodes`; `winget.resw`, read verbatim | no default return codes for `nullsoft`. `fileInUse` prints "One or more files are being used. Exit the application then try again." `ExpectedReturnCodes` may sit at the installer manifest's root |

## The dry runs (pull request #218)

The smoke test marks every installed file with the write time 2001-01-01 before A5's silent reinstall over the running app. `File` puts back the build's own time, so a file that still has the mark was skipped. The test also holds `OpenConsole.exe` open, the way a program that is still ending holds its file, from before the update until 8 seconds after the old app has ended. A7 holds it the same way through the uninstall with the app open.

**Before the fix** (commit 8e46b303, only the smoke test changed; release run 45899953975, failed as planned, `RESULT 57 passed, 2 failed`):

```
NOTE A5 before the update, running from the install folder: marshell-app.exe (PID 9532, parent 6460), OpenConsole.exe (PID 6316, parent 9532)
NOTE A5 the hold on OpenConsole.exe ended at 2026-11-26T21:53:22.9430222Z, the installer at 2026-11-26T21:53:17.2277271Z
PASS A5 reinstall over the running app: the installer exited 0
FAIL A5 reinstall over the running app: every program was replaced: kept the old OpenConsole.exe
FAIL A5 reinstall over the running app: the installer waited for the held OpenConsole.exe: it exited 5715 ms before the file was let go
NOTE the uninstall: OpenConsole.exe was held until 2026-11-26T21:53:36.5975525Z, 8 s after the app ended
PASS A7 uninstall: the install folder holds no file
```

Today's installer exited 0 and kept the old `OpenConsole.exe`, which is issue #214. The uninstaller's wait from slice 9b waited out the hold.

**After the fix** (commit f5c88b7b; release run 45901310193, success). The build job's new step "Check the installer hooks' waits" gave `RESULT 31 passed, 0 failed`, "Validate the manifest" passed with `ExpectedReturnCodes`, and the smoke test gave `RESULT 59 passed, 0 failed, 0 skipped`:

```
NOTE A5 the hold on OpenConsole.exe ended at 2026-11-26T22:05:45.0576571Z, the installer at 2026-11-26T22:05:47.3519050Z
PASS A5 reinstall over the running app: the installer exited 0
PASS A5 reinstall over the running app: every program was replaced
PASS A5 reinstall over the running app: the installer waited for the held OpenConsole.exe
NOTE the uninstall: OpenConsole.exe was held until 2026-11-26T22:06:00.9669554Z, 8 s after the app ended
PASS A7 uninstall: the install folder holds no file
```

The installer waited for the file and ended 2.3 s after it was let go.

## What stays unknown

- Whether winget shows the stop's line, and whether it prints its own `fileInUse` message for exit 32. That shows only when a version with the mapping is installed through winget over a file that stays in use.
- The Retry and Cancel dialog: `hooks-check.ps1` runs silently, and no installer runs on the development PC.
- The error an administrator's process gives when it holds the file. Sharing isn't checked per user, so it should be 32 too; nobody has tried it.
- With `/P` the stop leaves the progress page open until someone presses Close, because the template's `SetAutoClose` is never reached. This was read in the template, not watched.
