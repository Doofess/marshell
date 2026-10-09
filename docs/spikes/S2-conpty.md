# S2: ConPTY + shells on Windows

Run each shell by changing `CreateSessionRequest.shell` (temporarily, in `startSession`).

Run on 2026-10-10.
- **Machine:** Windows 11 Pro 26300, WebView2, portable-pty 0.9 with stock ConPTY.
- **Build:** debug app built from `14a12a8` plus a throwaway `localStorage` shell switch in a scratch worktree (never committed).
- **Driver:** the app was driven over WebView2 remote debugging (CDP) for keys and screenshots. Win32 `MoveWindow` did real window resizes.

| Shell argv | Renders prompt | Colours | Resize reflows | ESC / Alt+key | PSReadLine prediction | `exit` -> code | Orphans after close |
|---|---|---|---|---|---|---|---|
| `pwsh.exe -NoLogo` | Yes | Yes (PSReadLine syntax colours, `Write-Host -ForegroundColor Red`, raw SGR 31) | Yes (884 px → 124x37, 600 px → 81x25, 1300 px → a 150-char line re-wraps onto one row) | Yes (ESC reverts the line; `[Console]::ReadKey` sees `Alt+B`) | Yes (grey inline history prediction) | `exit 3` → "Process exited with code 3." | None |
| `powershell.exe -NoLogo` | Yes | Yes (PSReadLine colours, red `Write-Host`) | Yes (starts at the window's size, 179x39) | Yes (`ReadKey` sees `Alt+B`) | n/a | `exit 5` → 5 | None |
| `cmd.exe` | Yes | n/a (no colour by default) | Yes (starts at the window's size) | Yes (ESC clears the typed line) | n/a | `exit 7` → 7 | None |
| `C:\Program Files\Git\bin\bash.exe --login -i` | Yes (coloured MINGW64 prompt) | Yes (`ls --color`, SGR 31) | Yes (`tput` → 179x39) | Yes (readline Alt+B moves back a word: `abc def` → `abc Xdef`) | n/a | `exit 9` → 9 | None |
| `wsl.exe` | Not run: this machine has no WSL distribution installed | | | | n/a | | |

**Orphan check:**
- The test opened five sessions in one app run: the startup pwsh, plus Windows PowerShell, cmd and Git Bash after reloads.
- The startup pwsh was still running when the window closed.
- After the window closed, no shell process whose parent was the app remained, and the app process itself exited.

Also check:
- **First output arrives without a hang (the DSR answer from Task 4):** yes for every shell.
- **`claude` runs inside pwsh, and its TUI renders and resizes correctly:** yes (Claude Code 2.1.296), with two notes:
  - Nerd Font glyphs in the user's status line render as boxes. The terminal uses the bundled mono stack. Importing the user's terminal font is planned for phase 2.
  - This check found the clipped-last-row bug, fixed in the same commit as this file (see Findings).
- **Ctrl+C interrupts `ping -t localhost`:** yes, in pwsh, Windows PowerShell and cmd. In Git Bash, Ctrl+C interrupted `sleep 30`.

## Findings

1. **Fixed: the last terminal row was clipped by about 7 px.**
   - Cause: FitAddon sizes rows from the *parent's* computed height, which is the border-box size because `base.css` sets `box-sizing: border-box`. It then subtracts only the `.xterm` element's own padding, so the 8 px gutter on `.terminal-host` was counted as usable space.
   - Fix: the gutter moved onto `.terminal-host .xterm` (`src/styles/app.css`).
   - Before: the screen ended at 608 px in a 601 px viewport. After: it ends at 593 px.
2. **Not a bug: `NO_COLOR` is inherited.**
   - When Marshell is started from an environment with `NO_COLOR=1`, pwsh sets `$PSStyle.OutputRendering = PlainText`, so `Write-Host -ForegroundColor` prints uncoloured. Raw SGR still renders.
   - Inheriting the launcher's environment is the correct behaviour. It is worth a line in the doctor later, because a GUI launch normally won't have it.

3. **Inbox ConPTY is slow on floods of short lines.**

   The benchmark was `bash -c "yes | head -n 2000000 | cat"`, which writes 2M lines of `y` (6 MB after ConPTY adds `\r`):

   | Path | Time |
   |---|---|
   | Windows Terminal 1.24, with its own OpenConsole | 0.38 s (timed inside bash) |
   | Marshell core, raw `Session::pull`, with no WebSocket, using the inbox `conhost` ConPTY | 6.58 s |
   | Marshell core, raw, with a side-by-side `conpty.dll` + `OpenConsole.exe` | **1.51 s** |
   | Marshell app end to end (WebSocket + xterm), inbox ConPTY | 10.1 s |

   - portable-pty 0.9 already prefers a `conpty.dll` found next to the exe (`psuedocon.rs`), so no code change is needed.
   - The files come from the NuGet package `Microsoft.Windows.Console.ConPTY` 1.25.260930003 (MIT). Both binaries are Authenticode-signed by Microsoft Corporation.
   - Long lines were unaffected: the S6 raw run measured 38 MB/s both ways.

4. **Deviation from PLAN §7: phase 1 kills the Windows process tree with `taskkill /T /F`, not a Job Object.** The orphan check above passed with it. It only runs while the app is alive, so it does nothing if the app crashes. A Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` is planned for phase 2, when tabs become closable.

## Decision
- [x] Stock ConPTY is functionally fine: every shell renders, resizes, takes keys and exits cleanly.
- [x] Ship a sideloaded conpty.dll + OpenConsole for throughput. The reason is not a bug: the inbox ConPTY is about 4× slower than OpenConsole on short-line floods.
  - This is packaging work for phase 2: bundle the two x64/arm64 files as resources next to the exe and add the MIT notice.
  - Until then, dev builds use the inbox ConPTY.

The WSL row is still open. It needs a distribution installed (see S1, which needs one for WSLg too).
