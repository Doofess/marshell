# Claude Code hook install and removal live check (slice 3c1)

- Date: 2026-10-19
- Build: `phase-3c` at 5eaa89c, `pnpm tauri dev` on Windows 11 Pro (26300), Claude Code 2.1.303
- Home: a scratch `MARSHELL_HOME` with a space in its path (`%TEMP%\marshell 3c1 home`)
- Target: `MARSHELL_CLAUDE_SETTINGS` set to `%TEMP%\marshell-3c1 project\.claude\settings.local.json`, so the real `~/.claude` was never read for writing
- The scratch settings file holds a user `Stop` hook (`echo stop`) and `"statusLine": {"type":"command","command":"echo user-line","padding":0}`
- Driven over CDP (`--remote-debugging-port=9333`) and the local API with the scratch home's token

## Results

| Check | Result |
|---|---|
| Onboarding, Detected | Pass: "Coding agents found", Claude Code, version 2.1.303 |
| Onboarding, Consent | Pass: the headline read "Add 8 hook entries and chain your status line in …settings.local.json.", both switches On. Allow applied it |
| The applied settings file | Pass: 8 groups tagged `mshl1` after the user's `echo stop` hook, `statusLine` pointing at the bridge, the chain file holding `echo user-line`, and a snapshot taken. Strict `JSON.parse` of the result succeeds |
| Set up again from the row's "set up" button (hooks off) | Pass: the same headline; the diff caption names the settings file (+96 −2); "Show all 104 lines" opened the full diff and became "Show fewer lines". The resulting hash matched the onboarding install's hash, so the install is deterministic |
| Shell session: `cmd /k set CLAUDE` | Pass: only the person's own `CLAUDE_*` settings, no `CLAUDE_CODE_CHILD_SESSION` |
| Claude session in the scratch project, prompt "Reply with only the word ok." | Pass: `GET /v1/hook/events` held SessionStart, UserPromptSubmit and Stop, and no status line records. Claude's status line showed `user-line` through the chain, and Claude replied "ok" |
| Agents dialog → Hooks off → Turn hooks off | Pass: the diff named the settings file (+2 −96), and the file's hash went back to the recorded one |
| `bin\marshell.exe uninstall`, answered `n` | Pass: exit 1, "Nothing changed.", hash unchanged, `bin` kept. The diff is headed `--- a/<path>` and `+++ b/<path>` |
| `bin\marshell.exe uninstall --yes` | Pass: exit 0, snapshot named, the settings hash back to the pre-install one, and the rest of the home kept. The running `marshell.exe` couldn't delete itself and said "delete it once this window is closed"; the bridge was removed |
| Install, delete `<home>\bin` with the app running, open the Agents dialog | Pass: the orphaned warning names the settings file and the missing bridge, with Set up again and Hooks off |
| Set up again on that orphaned setup | **Fail**, see finding 1. Re-check after the fix: Pass |
| Forced colours (emulated), Agents dialog | Pass: dialog border, bordered buttons, focus ring |
| Forced colours (emulated), the row's "set up" button | Pass: bordered and readable on the selected row. Like Review changes, it shows on hover, focus or selection only |

The latest status line has no HTTP route in 3c1 (3c2 reads it), so "the latest status line is present" was not observable live. The unit test `status_lines_keep_only_the_latest` covers it, and the visible `user-line` shows the bridge ran the chain.

## Findings

1. **Set up again can't repair a missing bridge.** Only the app's launch copies `marshell-hook.exe` into `<home>\bin`. With the hooks already in place, Set up again plans no change (`noChange`, `can_apply: false`), but the consent card still offers Allow. The apply then answers 409 `refused`, which the card shows as "The file changed after this preview was made", which is not what happened. To fix in this slice: set up copies the bridge back when it is missing, and a preview with nothing to change says so instead of offering Allow.
2. **Claude Code 2.1.303 rejects comments in `settings.local.json`** ("Invalid or malformed JSON. Files with errors are skipped entirely"). Marshell parses and keeps comments, so the plan's fixture comment was dropped for the Claude session steps. Comment preservation is still covered by the byte-identity tests. A later slice could warn that Claude Code skips such a file.
3. **Fixed during the check:**
   - `setup.found` was false on a fresh home, because sessions start `claude` through a shell (7c7db96).
   - The diff caption read "modified, modified" (fc27f04).
   - Show all did nothing (5eaa89c).
4. **Minor, left as they are:**
   - The dialog opened from the row's button leaves out the version line; the palette's Agents dialog shows it.
   - `exe_path` keeps the PATH lookup's casing (`claude.EXE`).
   - The consent diff's label names one path.
   - A closed "Start a session" dialog stays in the DOM.
   - The same home spelled two ways (`C:\Users\Home Office\…` and its 8.3 form `C:\Users\HOMEOF~1\…`, which `%TEMP%` gives) reads as another home, so a setup made under one spelling shows "Some hooks are missing" under the other. Only a hand-set `MARSHELL_HOME` can do this.
5. **By design:**
   - The removal diff never includes the chain file, because removal reads the chain and never changes it.
   - The reinstall diff leaves the chain file out when it already holds the same command.

## Notes

- **Folder trust:** the first Claude session stopped at Claude Code's folder trust prompt for the scratch project. Down went in through the input API and Enter as a trusted key to that tab only; Claude Code recorded the trust in its own state.
- **Slow first page:** the webview's first load showed an empty page for about 20 seconds while Vite served the app cold. Waiting, or one reload, showed it.
- **Step 7 order changed:** each launch copies the programs into `bin` again, so restarting the app as the plan wrote would have hidden the orphaned state. The check deleted `bin` while the app ran instead.
- **Re-check of finding 1** (after 37db6b7, 275c523 and 7ca4a2b): with the hooks installed, `bin` was deleted while the app ran and the dialog showed the orphaned warning. Set up again copied `marshell-hook.exe` and `marshell.exe` back into `bin`, and the card read "Already set up. Nothing in …settings.local.json changes." with no Allow and a Done button. Done went back to "Hooks on", and the settings hash stayed `aedaee02`. Hooks off now shows the core's sentence, "Removes the 8 hook entries Marshell added and puts your status line back."
- **Cleanup:** the sessions were started for this check, and Enter was sent only into them. The app was stopped through the task that started it, and both scratch folders were removed after the re-check.
