# Slice 9d: the installer waits for Marshell's programs (issue #214)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** a silent update over a running Marshell replaces every program file, or stops and says so, and never again exits 0 with an old `OpenConsole.exe` left in place.

**Architecture:** Tauri's `Section Install` runs `NSIS_HOOK_PREINSTALL`, then `CheckIfAppIsRunning`, then `File` for each program. The hook now closes the app itself with that same macro, then waits (every 250 ms, up to 20 s of pauses) until none of the six program files in `$INSTDIR` is in use. "In use" means that opening the file for writing fails with error 32 or 33. At the limit, a silent install aborts with exit code 32, which the winget manifest maps to `fileInUse`; a windowed one asks Retry or Cancel. One list macro feeds this wait and the uninstaller's from slice 9b.

**Tech Stack:** NSIS 3 (the hooks file Tauri includes, the System plugin), PowerShell 7 (release scripts), Vitest, GitHub Actions, the winget manifest schema 1.10.0.

**Spec:** `docs/PLAN.md` §9, "Slice 9d decisions (2026-11-26)"; issues #214 and #215.

## Global Constraints

- Never run `scripts/release/smoke.ps1`, an installer, an uninstaller, or `marshell uninstall --yes` on the development PC. A template copy compiled for a check writes no setup file, or has it deleted unrun.
- The waits look only at files in `$INSTDIR`, named from `MARSHELL_EACH_PROGRAM`. They never look a process up by name and never end a process. The only thing that closes the app is the template's own `CheckIfAppIsRunning`.
- Limit `MARSHELL_HELD_WAIT_MS` 20000 (under `!ifndef`), step `MARSHELL_HELD_STEP_MS` 250, `MARSHELL_HELD_WAIT_S` = limit / 1000 (`!define /math`). A silent install always ends by itself.
- Exit code `MARSHELL_HELD_EXIT` 32: `SetErrorLevel ${MARSHELL_HELD_EXIT}` comes straight before each of the wait's `Abort`s. The template's own aborts keep 2. The installer manifest maps 32 to `fileInUse` at its root.
- Copy, verbatim (`$R9` is the first program still in use, `N` is `${MARSHELL_HELD_WAIT_S}`). "Removed" means `$INSTDIR\${MAINBINARYNAME}.exe` doesn't exist when the wait gives up:
  - details line, in both waits: `Waiting for Marshell's programs to end (up to N seconds)`
  - dialog (MB_RETRYCANCEL, MB_ICONEXCLAMATION): `Marshell has closed, but its program $R9 is still running, so the installer can't replace it.$\n$\n$R4`, where `$R4` is `Retry waits for it again. Cancel stops the installer and changes nothing.`, or when removed `Retry waits for it again. Cancel stops the installer; the earlier version has already been removed.`
  - the stop: `Marshell wasn't updated: $R9 was still running N seconds after Marshell closed. Nothing was changed. Run the update again.`, or when removed `Marshell wasn't installed: $R9 was still running N seconds after Marshell closed. The earlier version had already been removed. Run the installer again.`
  - release notes, after "…so finish what your agents are doing first.": `If an update stops because one of Marshell's programs is still running, run it again; its message says whether the earlier version is still installed.`
- The user approved one CI change: a `hooks-check.ps1` step in `release.yml`'s `build` job. Nothing else in `.github/` changes. The manifest's return code is in `scripts/release/winget-manifest.mjs` (the user's choice).
- No tag in this slice. Commits read `type(scope): sentence.` and end with the session's attribution lines.

## What was verified before planning

All of these ran on stand-ins in `%TEMP%\9d-facts`, with the NSIS that Tauri downloads (`%LOCALAPPDATA%\tauri\NSIS`), or were read in the sources named:

- `NSIS_HOOK_PREINSTALL` runs before `CheckIfAppIsRunning` in `Section Install` (`target/release/nsis/x64/installer.nsi`, Tauri CLI 2.12.1), and the first `File` after it is the 34 MB main program, then `OpenConsole.exe`. `hooks.nsh` is included after `utils.nsh`. A template copy whose hook inserts `CheckIfAppIsRunning` compiled (exit 0): the template's labels use `${__LINE__}`, so the two copies don't clash. The review compiled the wait sketch below under NSIS 3.11 as written.
- `CreateFileW(GENERIC_WRITE, share 7, OPEN_EXISTING)` gives these errors: a running exe 32, a file held with `FileShare.Read` 32, a read-only file 5, a missing file 2, a free file opens. `File` opens with `FILE_SHARE_READ` and `CREATE_ALWAYS`, which is stricter, so a 32 from the probe means `File` would fail too. The final review found two gaps, and the probe was changed for both: a read-only file gives 5 even when it is held or loaded, so the probe clears the flag, tries again and puts it back; and a holder that writes and shares writing passes share 7 but not `File`, so the probe uses share 1, as `File` does.
- `File` with `SetOverwrite on` overwrites a read-only file and clears the flag. NSIS `FileOpen … a` calls that file in use, which is why the probe reads the error code.
- A silent `Abort` in a section exits 2, and `SetErrorLevel 7` before it makes the exit code 7.
- A macro parameter can name another macro (`!insertmacro ${do} "a.txt"`).
- `AttachConsole(-1)` with `GetStdHandle(-11)` and `FileWrite`: the line lands in a file passed as `-RedirectStandardOutput`.
- A running exe's write time can be set, and the exe can be renamed. It can't be opened for writing or deleted.
- `File` restores the source's write time (`SetDateSave` is on by default), and `WriteUninstaller` writes the current time.
- An administrator session's `marshell-host.exe` ends once the core is gone (`crates/host/tests/core_killed.rs`).
- A windowed upgrade's reinstall page checks "uninstall before installing" by default (`installer.nsi`, `PageReinstall`), and leaving the page runs the installed uninstaller in place before `Section Install`. Passive mode skips the page; silent mode shows no pages.
- winget has no default return codes for `nullsoft` installers (`GetDefaultKnownReturnCodes` in winget-cli `src/AppInstallerCommonCore/Manifest/ManifestCommon.cpp`). `ExpectedReturnCodes` may sit at the installer manifest's root. `fileInUse` prints "One or more files are being used. Exit the application then try again." (`InstallFlowReturnCodeFileInUse`, winget-cli `winget.resw`, read verbatim).

## Review Focus

1. **A read-only program file** (a person set the flag): not counted as in use when nothing holds it, so the update doesn't wait, and `File` replaces it. When it is held or loaded it is in use, and the flag is put back when the install stops. Pinned by the `hooks-check.ps1` install cases "a read-only program", "a read-only program that is held" and "a read-only loaded DLL" (the last two from the final review).
2. **A first install, or a folder with only some programs:** no wait for a file that isn't there. Pinned by the install case "no programs yet".
3. **Cancel on the template's own "Marshell is running" question** (windowed install): the template aborts inside the hook, as it did before, and nothing of ours runs after it. Read in review; the hook's body is pinned to exactly the two inserts.
4. **Retry, then Cancel, and a passive install**: Retry starts a fresh 20 s wait and asks again; Cancel aborts with the stop's line and exit 32. With `/P` the abort leaves the progress page open until someone presses Close (the template's `SetAutoClose` is never reached). Nothing runs these (Global Constraints), so they are read in review and listed in the PR's Known limits.
5. **Windows Terminal's own `OpenConsole.exe`**, or any other program of that name: never waited for. Pinned by the `$INSTDIR`-only test in `hooks.test.ts`.

---

### Task 1: the smoke test catches a skipped file (failing first)

**Files:**
- Modify: `scripts/release/smoke.ps1`

**Interfaces:**
- Produces, in `smoke.ps1`:
  - `$Mark`, a UTC `[datetime]` for 2001-01-01T00:00:00Z, and `$HoldAfterMs = 8000`.
  - `Set-Marks([string]$Dir)`: sets the write time of every `$Programs` file that exists in `$Dir` to `$Mark`. Gives the names it couldn't set, as an array.
  - `Get-Marked([string]$Dir)`: the `$Programs` names whose write time still equals `$Mark`.
  - `Start-Hold([string]$Path, [System.Diagnostics.Process]$App)`: caches `$App.Handle` first, so a reused PID can't fool it. Opens `$Path` with `FileAccess.Read` and `FileShare.Read`; when that fails it writes `NOTE could not hold <name>: <message>` and gives `$null`. Otherwise it starts a thread job (`Start-ThreadJob -ArgumentList`) with the stream, the process and `$State = [hashtable]::Synchronized(@{ Stop = $false })`. The job sets `$ErrorActionPreference = 'Continue'`. It polls `$App.HasExited` every 100 ms, for up to 120 s or until `$State.Stop`. When the app has ended and nothing stopped it, it sleeps `$HoldAfterMs`. Then it disposes the stream. Its only output is the UTC time of the close, or `$null` when the app never ended or `Stop` was set. Gives `[pscustomobject]@{ Stream; Job; State }`.
  - `Stop-Hold($Hold, [switch]$Now)`: `$null` gives `$null`. `-Now` sets `$Hold.State.Stop` first. Then `Receive-Job -Wait -AutoRemoveJob` runs inside try/catch: on an error it writes `NOTE the hold's job failed: <message>` and gives `$null`. Then it disposes the stream again, which is harmless. Gives the last `[datetime]` the job wrote, or `$null`. It never throws.
  - `Start-Setup([string]$File, [string]$ArgString)`: the start half of `Invoke-Setup`. It gives the process with its handle cached, and `Invoke-Setup` calls it.
  - `Uninstall-Marshell([System.Diagnostics.Process]$HoldUntil = $null)`.

- [ ] **Step 1: A5.** Change A5 in `Invoke-CycleA` (installer mode only):
  1. Print `NOTE A5 before the update, running from the install folder: …` with `Get-RunningFrom`.
  2. `Set-Marks $script:Inst`. If any name fails: `Report-Fail 'A5 reinstall over the running app: the write-time marks' "could not set <names>"`.
  3. `$hold = Start-Hold (Join-Path $script:Inst 'OpenConsole.exe') $script:App`. When it is `$null`: `Report-Fail 'A5 reinstall over the running app: the hold on OpenConsole.exe' 'could not open it'`, and carry on without the hold.
  4. `$p = Start-Setup $Installer '/S'`, then `WaitForExit(300 s)`. Stop it by PID on timeout, as `Invoke-Setup` does. Stop-Hold runs on every path: on an exception it runs with `-Now` before the exception is rethrown.
  5. `$released = Stop-Hold $hold`. Print `NOTE A5 the hold on OpenConsole.exe ended at <released, o>, the installer at <exit time, o>`. Keep the existing checks for exit 0 (now `$p.ExitCode`) and for the app ending.
  6. Add `A5 reinstall over the running app: every program was replaced`. It passes when `Get-Marked` is empty; the failure text is `kept the old <names>`.
  7. Add `A5 reinstall over the running app: the installer waited for the held OpenConsole.exe`. It passes when `$released` isn't null and `$p.ExitTime.ToUniversalTime() -gt $released`. The failure texts are `it exited <n> ms before the file was let go` and `the old app never ended, so the file was let go at the limit`.
- [ ] **Step 2: A7.** When `$HoldUntil` is given, `Uninstall-Marshell` calls `Start-Hold` on `OpenConsole.exe` in `Get-InstallDir` before the first command, and `Stop-Hold` after the uninstaller copies have ended. The body sits in try/finally, so the `throw` for a missing QuietUninstallString still stops the hold (with `-Now`). It prints `NOTE the uninstall: OpenConsole.exe was held until <time, o>, 8 s after the app ended`, or `NOTE the uninstall: the hold on OpenConsole.exe ended without the app ending`. Cycle A's A7 call passes `$app`; cycle B's call stays as it is.
- [ ] **Step 3: parse check.** Run `pwsh -NoProfile -Command "$null = [System.Management.Automation.Language.Parser]::ParseFile('scripts/release/smoke.ps1', [ref]$null, [ref]$e); $e.Count"`. Expected: `0`. Never run the script.
- [ ] **Step 4: the helpers on stand-ins.** In a new folder `%TEMP%\9d-facts\holds`, copy only `$Mark`, `$HoldAfterMs`, `$Programs` and the five new functions into a scratch `.ps1` (never the whole script), with `$ErrorActionPreference = 'Stop'` as the script has. Check:
  - `Set-Marks` and `Get-Marked` round-trip on three stand-in files;
  - `Start-Hold` on a missing file gives `$null`;
  - a hold on a stand-in text file, with a hidden `pwsh -NoProfile -Command Start-Sleep 60` that the check starts as the process: opening the file for writing throws while it is held. After that pwsh is stopped by its own PID, `Stop-Hold` gives a time 8 s (±1 s) after the stop, and the file then opens for writing;
  - `Stop-Hold -Now` on a hold whose process still runs gives `$null` within 1 s.
- [ ] **Step 5: commit, push, draft PR.** Commit `test(release): the smoke test marks the programs before the update and holds OpenConsole.exe through it, so a skipped file fails.` (after the plan and PLAN §9 commit). Then `git push -u origin phase-9d`, and `gh pr create --draft` with the body from a file.
- [ ] **Step 6: read the dry run.** The gate is `A5 reinstall over the running app: every program was replaced`: expected FAIL, with `kept the old` naming at least `OpenConsole.exe`. Also expected: A5's installer exits 0, and `A7 uninstall: the install folder holds no file` passes (9b's wait, now with the hold). The waited-for check is information only on today's build; copy its line too. Copy the A5 and A7 lines and the NOTEs, with the run id, into the ledger. If the gate passes, the hold didn't reach the copy: stop and look before Task 2.

### Task 2: the installer's wait

**Files:**
- Modify: `src-tauri/windows/hooks.nsh`, `scripts/release/hooks.test.ts`, `scripts/release/hooks-check.nsi`, `scripts/release/hooks-check.ps1`

**Interfaces:**
- Produces, in `hooks.nsh`:
  - `!macro MARSHELL_EACH_PROGRAM do`: one `!insertmacro ${do} "<name>"` line for each of the six programs, in today's order.
  - `!macro MARSHELL_PROBE_PROGRAM file`: when `$R9` is empty and the file exists, it calls `System::Call 'kernel32::CreateFileW(w "$INSTDIR\${file}", i 0x40000000, i 7, p 0, i 3, i 0x80, p 0) p.R7 ?e'` and always pops the error into `$R6`. The handle decides: when `$R7 P<> -1` it calls `CloseHandle` and nothing else. Only when the handle is -1 is `$R6` read: 32 or 33 sets `$R9` to `${file}`, and any other error leaves `$R9` alone. The final review changed two things, both pinned by new `hooks-check.ps1` cases (40 checks): the share mode is `i 1`, the one `File` uses. When the first try gives 5 on a read-only file, the probe clears only the read-only bit (`GetFileAttributesW`, `SetFileAttributesW`, in `$R4` and `$R5`), tries again, and puts the attributes back after `CloseHandle`.
  - `!macro MARSHELL_WAIT_FOR_HELD_PROGRAMS`: no parameters. It uses `$INSTDIR`, `${MAINBINARYNAME}` and `$PassiveMode`, and pushes and pops `$R4`–`$R9`.
  - `MARSHELL_HELD_WAIT_S`, and `!define MARSHELL_HELD_EXIT 32`.
  - `NSIS_HOOK_PREINSTALL` is exactly two lines: `!insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"`, then `!insertmacro MARSHELL_WAIT_FOR_HELD_PROGRAMS`.
  - `MARSHELL_REMOVE_HELD_PROGRAMS` keeps its behaviour, but its six lines become `!insertmacro MARSHELL_EACH_PROGRAM MARSHELL_DELETE_PROGRAM`, and its details line follows the copy rule.
- Produces, in `hooks-check.nsi`: `/DCHECK=uninstall` runs `MARSHELL_REMOVE_HELD_PROGRAMS`. `/DCHECK=install` runs `MARSHELL_WAIT_FOR_HELD_PROGRAMS`, then `!insertmacro MARSHELL_EACH_PROGRAM CHECK_WRITE_PROGRAM`. That macro, defined in the `.nsi`, stands in for `File`: for a program that exists it calls `SetFileAttributes … NORMAL`, then `FileOpen … w`, and writes `replaced` when the open worked. The `.nsi` declares `Var PassiveMode`, and its header says what each mode does.

The wait, the one algorithm here:

```nsis
StrCpy $R8 0
${Do}
  StrCpy $R9 ""
  !insertmacro MARSHELL_EACH_PROGRAM MARSHELL_PROBE_PROGRAM
  ${If} $R9 == ""
    ${ExitDo}
  ${EndIf}
  ${If} $R8 >= ${MARSHELL_HELD_WAIT_MS}
    ${If} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
      StrCpy $R4 "<the dialog's second paragraph>"
      StrCpy $R5 "<the stop's line>"
    ${Else}
      StrCpy $R4 "<the removed second paragraph>"
      StrCpy $R5 "<the removed stop's line>"
    ${EndIf}
    ${If} ${Silent}
    ${OrIf} $PassiveMode = 1
      ; AttachConsole(-1), GetStdHandle(-11) into $R6; when it isn't 0 or -1, FileWrite "$R5$\r$\n"
      SetErrorLevel ${MARSHELL_HELD_EXIT}
      Abort "$R5"
    ${EndIf}
    ${IfNot} ${Cmd} `MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "<the dialog's first sentence>$\n$\n$R4" /SD IDCANCEL IDRETRY`
      SetErrorLevel ${MARSHELL_HELD_EXIT}
      Abort "$R5"
    ${EndIf}
    StrCpy $R8 0
  ${EndIf}
  ${If} $R8 = 0
    DetailPrint "<the details line>"
  ${EndIf}
  Sleep ${MARSHELL_HELD_STEP_MS}
  IntOp $R8 $R8 + ${MARSHELL_HELD_STEP_MS}
${Loop}
```

- [ ] **Step 1: the failing tests in `hooks.test.ts`.**
  - "names every program the installer puts in the install folder, once": the names in `MARSHELL_EACH_PROGRAM` equal `installedPrograms()`.
  - "both waits go through that list and name no program themselves": `macroBody("MARSHELL_REMOVE_HELD_PROGRAMS")` contains `!insertmacro MARSHELL_EACH_PROGRAM MARSHELL_DELETE_PROGRAM`, and `MARSHELL_WAIT_FOR_HELD_PROGRAMS` contains `!insertmacro MARSHELL_EACH_PROGRAM MARSHELL_PROBE_PROGRAM`. No line in the file outside `MARSHELL_EACH_PROGRAM` matches `/!insertmacro MARSHELL_(DELETE|PROBE)_PROGRAM "/`.
  - "the installer closes the app the template's way, then waits, and does nothing else first": `macroBody("NSIS_HOOK_PREINSTALL")` equals the two lines exactly.
  - "looks only at files in the install folder, never at a process": first, each of the five bodies (`MARSHELL_EACH_PROGRAM`, `MARSHELL_DELETE_PROGRAM`, `MARSHELL_PROBE_PROGRAM`, `MARSHELL_REMOVE_HELD_PROGRAMS`, `MARSHELL_WAIT_FOR_HELD_PROGRAMS`) is non-empty. Then `onlyInstallFolder(body: string[]): string | null` checks every line that isn't a comment (trimmed start `;` or `#`). It drops NSIS escapes (`/\$\\./g`) first, then removes every `$INSTDIR\`, and fails on any `\` left. That covers every quoting and unquoted paths. It also fails on any line matching `/nsExec|nsProcess|FindProcess|KillProcess|ExecWait|ExecShell|^\s*Exec\b|RmShutdown|RmGetList|RmStartSession|nsis_tauri_utils|taskkill|OpenProcess|TerminateProcess|CreateToolhelp32Snapshot|Process32|EnumProcesses/i`. The function gives the bad line or null. Then the self-test:
    - it catches `Delete "$TEMP\x.exe"`, `Delete $TEMP\x.exe`, ``Delete `C:\x` ``, `System::Call 'kernel32::OpenProcess(i 1, i 0, i r0) p.r1'`, `nsExec::Exec 'taskkill'`, `nsis_tauri_utils::FindProcess "OpenConsole.exe"` and `Exec "x"`;
    - it passes `FileWrite $R6 "$R5$\r$\n"`, `Delete "$INSTDIR\${file}"` and the probe's `System::Call` line.
  - "says what the person is told (PLAN §9, slice 9d decision 3)": `hooks.nsh` contains each copy string from Global Constraints: the details line, the dialog's first sentence with `$\n$\n$R4`, both second paragraphs and both stops, with `${MARSHELL_HELD_WAIT_S}` for N. The text `Waiting for Marshell's programs to end"`, without the "(up to …)" part, no longer appears.
  - "the copy's seconds come from the limit": the file matches `/^!define \/math MARSHELL_HELD_WAIT_S \$\{MARSHELL_HELD_WAIT_MS\} \/ 1000$/m`.
  - "a stop exits 32": the file matches `/^!define MARSHELL_HELD_EXIT 32$/m`. `MARSHELL_WAIT_FOR_HELD_PROGRAMS` has exactly two `Abort` lines, and the line before each is `SetErrorLevel ${MARSHELL_HELD_EXIT}`.
  - Keep "gives up after 20 seconds unless a build says otherwise", the POSTUNINSTALL tests and `programsOf`. Drop the old per-name list test, which the first test replaces.
- [ ] **Step 2:** `pnpm test scripts/release/hooks.test.ts`. Expected: the new tests fail, the kept ones pass.
- [ ] **Step 3: install cases in `hooks-check.ps1`.** It builds both programs (`/DCHECK=uninstall` and `/DCHECK=install`, each with `/DMARSHELL_HELD_WAIT_MS=3000`). `Invoke-Macro` takes the exe and redirects stdout to `$Scratch\<case>.stdout.txt`, never inside the case folder: the program holds that file while it runs, so it would stop the folder's removal. The uninstall cases stay as they are, and the header no longer says "No workflow runs it". The install cases each get a fresh folder:
  - **"install, nothing in use"**: exit 0, every program reads `replaced`, under 2000 ms.
  - **"install, a program that ends after 1.2 s"** (OpenConsole.exe held, released at 1200 ms): exit 0, every program reads `replaced`, took 1200 ms or more and less than 2500 ms.
  - **"install, a program that never ends"** (marshell-host.exe held): exit 32, between 3000 and 8000 ms, every program still reads `stand-in`. Stdout trimmed is exactly `Marshell wasn't updated: marshell-host.exe was still running 3 seconds after Marshell closed. Nothing was changed. Run the update again.`
  - **"install, the earlier version already removed"** (every program but `marshell-app.exe`, conpty.dll held): exit 32, and stdout trimmed is exactly `Marshell wasn't installed: conpty.dll was still running 3 seconds after Marshell closed. The earlier version had already been removed. Run the installer again.`
  - **"install, a loaded DLL"**: `conpty.dll` is a copy of `C:\Windows\System32\version.dll`, loaded into the check's own process with `[System.Runtime.InteropServices.NativeLibrary]::Load` and freed once the program has ended. Expected: exit 32, and stdout names `conpty.dll`. This tries what the probe gives for an image mapping, which nothing else has tried.
  - **"install, a read-only program"** (conpty.dll read-only, nothing held): exit 0, under 2000 ms, every program reads `replaced`.
  - **"install, no programs yet"** (only `uninstall.exe` held and `notes.txt`): exit 0, under 2000 ms, the folder holds just those two.
  Clean up every name the script made, the stdout files too, as it does now.
- [ ] **Step 4: see the install cases fail.** Add `MARSHELL_EACH_PROGRAM` and an empty `MARSHELL_WAIT_FOR_HELD_PROGRAMS`, then run `./scripts/release/hooks-check.ps1`. Expected: at least "a program that ends after 1.2 s: every program reads replaced" (OpenConsole.exe) and "a program that never ends: exit 32" fail. Every uninstall case passes.
- [ ] **Step 5: write the macros, the hook and the copy** per the Interfaces and the sketch.
- [ ] **Step 6:** `./scripts/release/hooks-check.ps1` gives `RESULT <n> passed, 0 failed`, and `pnpm test scripts/release` passes everything.
- [ ] **Step 7: the real template compiles.** Copy `C:\dev\Airport\target\release\nsis\x64\*` into a new folder `%TEMP%\9d-facts\tpl2`, point its `!include` of `hooks.nsh` at this worktree's file, and run makensis `/V2` with `"/XOutFile NUL"` after the script. Expected: exit 0. If a setup file was written anyway, delete it without running it.
- [ ] **Step 8:** commit `fix(installer): an update waits up to 20 seconds for Marshell's programs to end, and stops with exit code 32 when one is still running.`

### Task 3: the build job runs the check, winget explains the stop, and the notes say what to do

**Files:**
- Modify: `.github/workflows/release.yml`, `scripts/release/workflow.test.ts`, `scripts/release/notes.mjs`, `scripts/release/notes.test.ts`, `scripts/release/winget-manifest.mjs`, `scripts/release/winget-manifest.test.ts`

- [ ] **Step 1: the failing tests.**
  - `workflow.test.ts`, "the build job tries the hooks' waits before the smoke test": `build` has a step named `Check the installer hooks' waits`, with `timeout-minutes: 5`, `shell: pwsh` and `run: ./scripts/release/hooks-check.ps1`. It comes after `Build the installer` and before `Smoke test (install, start, update over it, uninstall)`.
  - `notes.test.ts`: the notes contain the release-notes line from Global Constraints, right after "so finish what your agents are doing first.".
  - `winget-manifest.test.ts`, "a stopped update is winget's file-in-use": the installer manifest has, at its root after `UpgradeBehavior: install`, the lines `ExpectedReturnCodes:`, `- InstallerReturnCode: 32` and `  ReturnResponse: fileInUse`. The 32 equals the `MARSHELL_HELD_EXIT` that `src-tauri/windows/hooks.nsh` defines, read from the file.
- [ ] **Step 2:** `pnpm test scripts/release`. Expected: those three fail.
- [ ] **Step 3:** add the step, with a two-line comment above it (what it tries, and that it installs nothing). Add the notes line, and the manifest's lines with a comment naming the hooks' define.
- [ ] **Step 4:** `pnpm test scripts/release` passes. Commit `ci(release): the build job tries the installer hooks' waits, winget reads exit 32 as a file in use, and the notes say what to do when an update stops.`
- [ ] **Step 5:** push and read the dry run. Expected:
  - the build job's hooks-check step ends with `RESULT … 0 failed`;
  - "Validate the manifest" passes with the new lines;
  - in the smoke test, both new A5 checks pass, `A7 uninstall: the install folder holds no file` passes, and the run ends with `RESULT … 0 failed`.

### Task 4: the records, the review, the merge

**Files:**
- Modify: `docs/RELEASING.md` (the hooks-check paragraph, and the known limit at "The installer and the uninstaller close a running Marshell…")
- Create: `docs/research/2026-11-26-installer-skips-held-file.md`

- [ ] **Step 1:** in `RELEASING.md`, say that the build job now runs `hooks-check.ps1`, which covers both waits. Rewrite the known limit:
  - the installer and the uninstaller close a running Marshell and wait up to 20 s for its programs;
  - an update that still meets one stops with exit 32, which winget reads as a file in use, and changes nothing;
  - a windowed upgrade that removed the earlier version first says so instead;
  - with `/P`, the progress page stays open until Close;
  - the uninstaller still gives up silently and leaves the file;
  - not watched: an administrator session's helper, and the Retry dialog.
  Drop "the release notes don't say that yet".
- [ ] **Step 2:** the research note covers:
  - the verified facts above, with their commands;
  - Task 1's failing run (run id and lines) and Task 3's green run;
  - what stays unknown: winget showing the line and its own message, the Retry dialog, and an elevated holder's error code.
  Commit.
- [ ] **Step 3:** run `marshell-ship`'s checks, then the final whole-branch review (`marshell-reviewer`, Opus) with this plan's Review Focus, then one fix pass.
- [ ] **Step 4:** write the PR body (What it does, Live check: "none; the dry runs are the check", Known limits, Checks), with `Closes #214`. Then run `pre-merge-review` and merge. Comment on #215 with what 9d settled, and leave it open for the administrator helper and the dry run that hung.
