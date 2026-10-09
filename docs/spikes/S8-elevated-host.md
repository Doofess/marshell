# S8: Elevated session helper (Windows)

Run 2026-10-10 on the dev box (Windows 11 Pro 26300, UAC at its default: consent prompt on the secure desktop).
Driver: `spikes/elevated-host`, with `launch` started non-elevated and its stdin fed by a script that waits for
the helper process to appear. The user answered the UAC prompts.

| Check | Pass criterion | Result |
|---|---|---|
| UAC prompt from non-elevated parent | Prompt appears; decline is reported as an error | **Prompt: pass.** The parent printed `elevated before UAC? false`, then UAC appeared. **Decline: not observed.** Each prompt was approved. The decline path is `ShellExecuteExW` returning 0 (`ERROR_CANCELLED`), followed by `ensure!`. Re-run when convenient. |
| Helper is elevated | `is_elevated()` true; `whoami /groups` shows High Mandatory Level | **Pass.** The helper refuses to connect unless elevated. `whoami /groups` shows `Mandatory Label\High Mandatory Level` and `BUILTIN\Administrators … Enabled group`, and the window title reads `Administrator: …pwsh.exe`. |
| Loopback + one-time token | Helper connects; wrong token is rejected | **Connect: pass.** Five runs connected over 127.0.0.1 with the token. **Wrong token: not run.** The `ensure!` exists. To exercise it, inject a connection while the UAC dialog is up (steps in the phase 1 task 12 report). |
| Input reaches elevated pwsh | Commands typed in the parent run in the elevated shell | **Pass.** `whoami /groups \| findstr Mandatory` and `exit`, typed in the parent, ran in the elevated shell. |

## Findings

1. **The ConPTY startup DSR applies here too.**
   - portable-pty 0.9 makes ConPTY ask for the cursor position (`ESC[6n`) and hold all output until it gets an answer. The first runs showed no output at all.
   - The spike now answers it on the `launch` side. The production helper must reuse the core's DSR filter (`crates/core/src/pty/dsr.rs`).
2. **An orphaned elevated shell.**
   - In a run where the helper exited early, its elevated `pwsh.exe` kept running with no parent. A non-elevated process cannot stop it, so the user had to end it by hand.
   - The production helper must start its shell in a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, so the shell dies with the helper.
3. **The token is on the helper's command line.**
   - Other processes of the same user can read it while the helper lives.
   - Production must pass it another way: over an inherited pipe, or on stdin.

## Threat notes for phase 6
- **Any non-elevated process holding the token can drive an admin shell.** So:
  - The token is one-time and kept only in core memory, never written to `endpoint.json` or to a command line (finding 3).
  - The helper accepts one connection only.
  - The helper checks that the peer's owning pid is the Marshell core (`GetExtendedTcpTable`).
  - The helper accepts only input and resize.
- **The helper owns its shell's lifetime** through a Job Object (finding 2).
- **Fallback:** if these can't be made safe, open elevated sessions in a separate elevated Marshell window, which is Windows Terminal's approach.

Decision: [x] helper approach viable for phase 6, with the requirements above  [ ] use separate elevated window
