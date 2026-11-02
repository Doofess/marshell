# Elevated helper live check (slice 6j2)

- Date: two runs.
  - **Run 1:** 2026-11-01, 20:13 to about 22:00 local time (scratch `lc-6j2-1`).
  - **Run 2:** started 2026-11-01 at 22:31 and driven 2026-11-02 from 10:45 to 10:49 (scratch `lc-6j2-2`).
- Build: `phase-6j2` under `pnpm tauri dev` on Windows 11 Pro (10.0.26300). Run 2 includes the fix in 2fad03e.
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `USERPROFILE` and `MARSHELL_CLAUDE_SETTINGS`, with CDP on port 9333.
  - Run 1's onboarding allowed hooks for all three CLIs. Every target path was in the scratch folder.
  - Run 2's onboarding skipped every CLI ("Skip: use limited status").
- Data: none seeded. Claude Code ran with the scratch config, so it stopped at its first-run theme picker and was never signed in.
- How it was driven:
  - CDP evaluations, CDP keys and typing into the terminal through the webview.
  - Forced colors were emulated, not a real high-contrast theme.
  - No desktop input from the check. The user answered every UAC prompt (decision 7), and each prompt was announced first.
  - Process trees were read with `Get-CimInstance Win32_Process`. Exit times were polled every 100 ms.
- The real `~/.claude/settings.json` kept its modified time (2026-10-25 14:04:04) through both runs. The user's own `claude.exe` processes (9308 and 25872) were left alone.

## Results

| Check | Result |
| --- | --- |
| 1. The decline path | **Pass (run 2).** <br>• Shown: "Administrator rights were declined. Nothing was started.", as a `role="alert"` inside the launcher. The launcher stayed open, with no session row. <br>• Tree: no `marshell-host.exe` was running. The only `pwsh`/`OpenConsole` children of the app were the sidebar's own Shell panel (20164, 28624), which started before the prompt. <br>• Run 1: the user declined the first prompt before the check watched for it, so only a 409 on `POST /v1/sessions` was seen in the console. |
| 2. An approved shell | **Pass (run 1).** <br>• `whoami /groups` showed "Mandatory Label\High Mandatory Level". <br>• The row and header show the shield and "Administrator". <br>• `MARSHELL_TAB_ID` was set, and `CLAUDE_CONFIG_DIR`, `MARSHELL_HOME` and `USERPROFILE` pointed into the scratch folder. <br>• The prompt drew and typing in the window worked. <br>• The core-only variables were not listed by hand. The host test `the_shell_sees_the_core_computed_environment` covers them. |
| 3. API input is refused | **Pass (run 1).** <br>• `marshell send 1 "echo from-api"` with the scratch `MARSHELL_HOME` printed "This session runs as administrator, so it takes input only from Marshell's own window." and exited 1. <br>• Nothing reached the terminal. |
| 4. An approved agent | **Pass, with the Allow driven by a synthesized hook payload (run 1).** <br>• Launch: "New session as administrator" from the palette opens the launcher with Claude and forced Manual mode, noting "An agent running as administrator starts in Manual.", and shows "Waiting for administrator approval…" while UAC is up. <br>• Approve card: the scratch Claude was not signed in, so a PermissionRequest with `permission_suggestions` was piped into the scratch `marshell-hook.exe permission --agent claude` with the session's `MARSHELL_TAB_ID`. The card had the Administrator shield and Deny N / Allow once Y / Answer in terminal T, with no Always allow. Allow once returned `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}` and saved no rule. <br>• Bypass: choosing it shows "I understand this agent runs as administrator and Bypass lets it run admin commands without asking." Restart stays `aria-disabled` until that is ticked (Space ticks it). Restart then raised UAC again, the row read "Administrator. Bypass permissions.", and a new helper replaced the old one. |
| 5. Pause is unavailable; Close | **Pass.** <br>• Pause: the menu item is `aria-disabled` with "Pause isn't available for a session running as administrator." <br>• Close (run 2): the session ran `pwsh -NoProfile -c "… Start-Sleep 900"`. After Close from the row menu at 10:47:15.501, helper 3884, OpenConsole 2348, conhost 38036, shell 4020 and its child 4020→40484 were all gone at 10:47:15.567. |
| 6. Restart Marshell | **Pass (run 1).** <br>• After a `tauri dev` rebuild, the row came back as "Ended last time. Claude, Ended. Administrator. Bypass permissions." with "Reopen as administrator". <br>• No UAC prompt appeared. <br>• See findings 2 and 3. |
| 7. Kill the app by PID | **Failed in run 1, pass in run 2 after 2fad03e.** <br>• Run 1: the helper and OpenConsole died at once, but the elevated `pwsh` and `claude` survived until they were stopped by PID (finding 1). <br>• Run 2: an elevated shell ran `pwsh -NoProfile -c "Start-Sleep 900"`, and app 14136 was stopped with `Stop-Process -Id 14136 -Force` at 10:48:46.045. Gone at 10:48:46.142: helper 24348, OpenConsole 20352, conhost 10172, shell 30668, child 32912, plus the sidebar's unelevated shell 20164 and its OpenConsole 28624. |
| 8. A `subst` drive | **Not run.** The scratch `subst` was not set up. The copy is covered by `a_missing_cwd_fails_with_the_copy` and `a_missing_cwd_comes_back_as_the_helpers_words`. |
| Keyboard and forced colors | **Pass.** <br>• Alt+A toggles the Administrator chip (`aria-checked`), and the summary reads "Start PowerShell 7 in proj, as administrator." <br>• With forced colors emulated, the shields on rows and header and the Bypass chip stay visible. |

## Findings

1. **A Store PowerShell outlived a killed Marshell, elevated and in Bypass.**
   - **What happened:** in run 1, killing the app took down the helper and its OpenConsole, but the elevated `pwsh` and the `claude` it ran kept running with nothing controlling them. A plain `pwsh -c "Start-Sleep 900"` child reproduced it.
   - **Cause:** PowerShell 7 from the Microsoft Store (`%LOCALAPPDATA%\Microsoft\WindowsApps\pwsh.exe`, a packaged app), started in a ConPTY from the bundled OpenConsole, is not placed in its creator's job. The helper's kill-on-close job therefore never held it.
     - System conhost and `cmd` do not show this.
     - Assigning the shell to the helper's own job fails with "Access is denied." (os error 5).
     - A new, empty kill-on-close job takes it.
   - **Fix:** 2fad03e. The helper puts the started program in a job of its own right after `spawn_command` and holds that job for the helper's life.
   - **Tests:** `a_killed_core_ends_a_store_shell_and_its_child` failed first and passes now. `closing_ends_a_store_shell_and_its_child` passed before and after. Both skip on machines without Store PowerShell 7, such as CI.
   - **Gap:** a process the shell starts before the assign escapes. Closing that needs a suspended start, which portable-pty does not offer.
2. **Reopen restores Bypass without asking again.** Run 1's restored row read "Administrator. Bypass permissions." "Reopen as administrator" would start the agent elevated in Bypass with only UAC in between, not the Bypass checkbox. Not fixed here; see the slice's final review.
3. **A restored elevated row keeps the wrong resume copy.** Beside "Reopen as administrator" the body still says "Resume runs claude --continue --permission-mode bypassPermissions …". Not fixed.
4. **The pending line names the wrong session number.** While the second session waited for UAC, the launcher said "It becomes session 3". Not fixed.
5. **The terminal socket retries while a create is pending.** The webview logged WebSocket 404s on `/v1/pty/<id>` until the helper connected. They are harmless. Not fixed.
6. **`marshell list` does not mark administrator sessions.** Not fixed.
7. **A medium-integrity process can end the elevated processes.** `OpenProcess(PROCESS_TERMINATE)` from the unelevated side succeeded on the elevated shell. That is how the leftovers were stopped in run 1. It is no new risk, because the job closes them anyway. Recorded for the threat notes.
8. **Unelevated sessions have the same Store PowerShell gap.** In probes outside the app, a local pty with the bundled OpenConsole left a busy Store `pwsh` and its child running after its creator was killed. The core's own job did not hold them. This predates 6j2 and is out of its scope.
   - In run 2, the sidebar's idle shell did end with the app. The probe's case was a shell running a long child.
