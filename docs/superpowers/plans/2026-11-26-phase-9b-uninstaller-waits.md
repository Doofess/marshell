# Slice 9b: the uninstaller waits for Marshell's helper programs (issue #212)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** an uninstall with Marshell open leaves no program file behind, so the release dry run passes and `v0.1.0-alpha.2` can be tagged.

**Architecture:** Tauri's NSIS template closes the running app and deletes the program files straight after, with no hook between the two. The uninstaller's last hook (`NSIS_HOOK_POSTUNINSTALL` in `src-tauri/windows/hooks.nsh`) therefore tries the program files again, every 250 ms for up to 20 s, and removes the folder once they are gone. Nothing is killed and nothing is looked up by process name: the test for "the helper has ended" is that its file can be deleted.

**Tech Stack:** NSIS (the hooks file Tauri includes in its installer), PowerShell 7 (the release scripts), Vitest.

**Spec:** `docs/PLAN.md` §9, "Slice 9b decisions (2026-11-26)"; issue #212.

## Global Constraints

- `scripts/release/smoke.ps1` runs only on GitHub's runners. Never run it, an installer, an uninstaller or `marshell uninstall --yes` on the development PC.
- The uninstaller never deletes or waits for a file that isn't one of Marshell's own programs, and never ends a process.
- The wait has a limit (20 000 ms). A silent uninstall must end by itself whatever holds a file.
- A pull request that touches `src-tauri/windows/**`, `scripts/release/**` or `release.yml` gets the release dry run by itself; that run is this slice's acceptance check (cycle A, step A7).
- No tag until this slice is merged and a dry run on `main` is green.

## What was verified before planning

- Tauri CLI 2.12.1's template (`crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi`, tag `tauri-cli-v2.12.1`), `Section Uninstall`: `NSIS_HOOK_PREUNINSTALL`, then `CheckIfAppIsRunning` (Restart Manager, the main exe only), then `Delete` for the main exe, each resource and `uninstall.exe`, `RMDir "$INSTDIR"`, the shortcuts and registry, then `NSIS_HOOK_POSTUNINSTALL`. No hook sits between closing the app and deleting the files. The `Install` section has the same shape (`NSIS_HOOK_PREINSTALL`, `CheckIfAppIsRunning`, `File`).
- A silent NSIS `Delete` of a file in use leaves it and goes on; a silent `File` over a file in use skips it, sets the error flag and the installer still exits 0 (tried with NSIS 3 on the development PC, on stand-in files in the temp folder).
- On a normal exit the app ends its sessions and then calls `release_children` (`src-tauri/src/main.rs`); when it is killed, its Job Object ends the children (`crates/core/src/job.rs`). Either way `OpenConsole.exe` ends after the app's own process has gone.
- The bundled ConPTY has not changed since `v0.1.0-alpha.1`.

## Review Focus

1. A program that never ends (an administrator session's `marshell-host.exe`): the uninstaller gives up at the limit and exits 0; it must not hang. Pinned by `hooks-check.ps1`, "a program that never ends".
2. An uninstall run in place (`uninstall.exe _?=<folder>`) has its own `uninstall.exe` in the folder, in use: no wait for it. Pinned by `hooks-check.ps1`, "files that aren't Marshell's programs".
3. Another program named `OpenConsole.exe` (Windows Terminal runs one per tab): never waited for, since only the files in `$INSTDIR` are tried.
4. A program added to the bundle later and not to the hook's list: `hooks.test.ts` fails.
5. An uninstall that is part of an update (`/UPDATE`, or in place before an install): the wait runs there too, so the install that follows meets free files; it must not run the unhook or remove the `marshell://` link there (unchanged code, read in review).

---

### Task 1: evidence from a runner

**Files:**
- Modify: `scripts/release/smoke.ps1` (`Get-RunningFrom`, `Test-Uninstalled`)

- [x] **Step 1:** add `Get-RunningFrom([string]$Dir)`: the running programs whose file is in `$Dir`, as `name (PID n, parent m)`, or `nothing`. `Test-Uninstalled` prints it once the uninstaller has ended and again in the failure text of "the install folder holds no file".
- [x] **Step 2:** the script still parses and the helper answers for a folder with a running program, an empty folder and no folder.
- [x] **Step 3:** commit, push, open the draft pull request, read the dry run's A7 lines.

Expected: A7 fails as on `main` ("left OpenConsole.exe"), and the new text says whether anything still runs from the folder 30 s later. "nothing" means a second try would have deleted the file, which is what Task 2 builds; a running `OpenConsole.exe` means waiting is the wrong fix and the plan stops here.

### Task 2: the wait

**Files:**
- Modify: `src-tauri/windows/hooks.nsh`
- Create: `scripts/release/hooks.test.ts`, `scripts/release/hooks-check.nsi`, `scripts/release/hooks-check.ps1`

**Interfaces:**
- Produces: `!macro MARSHELL_REMOVE_HELD_PROGRAMS` (no parameters; uses `$INSTDIR`, keeps `$R8` and `$R9`), `!macro MARSHELL_DELETE_PROGRAM file`, `!define MARSHELL_HELD_WAIT_MS 20000` under `!ifndef`, `!define MARSHELL_HELD_STEP_MS 250`.

- [x] **Step 1:** `hooks.test.ts`: the macro has one `!insertmacro MARSHELL_DELETE_PROGRAM "<name>"` for `${MAINBINARYNAME}.exe` and for every `.exe` and `.dll` target in `bundle.resources` of `tauri.windows.conf.json` and `tauri.release.conf.json`, and no other; the default limit is 20000 under `!ifndef`; `NSIS_HOOK_POSTUNINSTALL` inserts the macro outside every `${If}`.
- [x] **Step 2:** run `pnpm test scripts/release/hooks.test.ts`. Expected: 3 failed.
- [x] **Step 3:** `hooks-check.nsi` builds a program that runs the macro on the folder given with `/D=`; `hooks-check.ps1` builds it with a 3000 ms limit and checks four cases on stand-in files it holds open itself: nothing in use; a file freed after 1.2 s; a file never freed; files that aren't Marshell's programs (`uninstall.exe` held, `notes.txt`).
- [x] **Step 4:** with a macro that tries once and doesn't wait, run `./scripts/release/hooks-check.ps1`. Expected: FAIL "a program that ends after 1.2 s: the programs and the folder are gone: left OpenConsole.exe" and FAIL "it gives up after the limit".
- [x] **Step 5:** write the macro and insert it last in `NSIS_HOOK_POSTUNINSTALL`.
- [x] **Step 6:** `./scripts/release/hooks-check.ps1`. Expected: `RESULT 11 passed, 0 failed`. `pnpm test scripts/release/hooks.test.ts`. Expected: 3 passed.
- [x] **Step 7:** commit.

### Task 3: the workflow and the records

**Files:**
- Modify: `.github/workflows/release.yml` (a step after "Build the installer"), `docs/RELEASING.md`, `docs/PLAN.md` §9
- Create: `docs/research/2026-11-26-uninstall-leaves-openconsole.md`

- [x] **Step 1:** ~~the build job runs `./scripts/release/hooks-check.ps1` after the installer is built (5 minutes at most).~~ Built, ran green on the pull request's dry run (45855845988), then taken out in the fix pass: the final review pointed at `SCOPE.md`, which asks for the user's word before a change to CI. The script stays, run by hand.
- [x] **Step 2:** `docs/RELEASING.md`: the check, and the known limit rewritten (what the uninstaller now waits for, what it still leaves, and that the installer doesn't wait).
- [x] **Step 3:** `docs/PLAN.md` §9 "Slice 9b decisions (2026-11-26)" and the research note with the runs' evidence.
- [x] **Step 4:** `pnpm test`, commit, push.

Expected on the pull request's dry run: the new step passes, and `PASS A7 uninstall: the install folder holds no file`, `RESULT … 0 failed`.

### Task 4: after the merge

- [ ] `main`'s CI green, a dry run on `main` green.
- [ ] `git tag v0.1.0-alpha.2`, push the tag, watch the run. Once `publish` has succeeded, cancel the run while `winget-check` runs so `winget-submit` is skipped; then `gh workflow run release.yml -f action=check -f tag=v0.1.0-alpha.2`.
