# Doctor live check (slice 7a)

- Date: 2026-11-05, 01:57 to 02:11 local time.
- Build: `phase-7a` under `pnpm tauri dev` on Windows 11 Pro (10.0.26300). It started at 35f9960. The Rust fix below (9d358df) rebuilt and restarted the app at 02:07, and that restart was the check's app restart.
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR` and `MARSHELL_CLAUDE_SETTINGS`, a short scratch `CODEX_HOME` (`%TEMP%\lc-7a-cx`) and a scratch `USERPROFILE` for Antigravity and Qwen Code, with CDP on port 9333. Onboarding set up all four CLIs' hooks after each preview's target paths were read, and it showed only scratch paths.
- Data: after setup, `break_7a.py` (scratchpad) broke two files by hand. It removed Claude Code's `Stop` and `Notification` entries from the scratch `settings.local.json`, and it pointed Codex's `UserPromptSubmit` entry at `mh/bin-moved/marshell-hook.exe`. Claude Code's `cleanupPeriodDays` stayed unset. The seeded hashes were `34931A00A689` (Claude Code) and `CB9D4339A407` (Codex), and `managed.json` was `D4F57A31EC29`.
- How it was driven: CDP evaluations returning small JSON, CDP keys (Tab, Shift+Tab, Space, Enter, Escape) and emulated forced colours (not a real high-contrast theme). Four screenshots judged the look. There was no desktop input.

## Results

| Check | Result |
| --- | --- |
| The Doctor opens from the sidebar count and the palette | Pass. The sidebar count is a button labelled "Doctor: 3 issues". Typing "doctor" in the palette lists "Open Doctor", and Enter opens the page. |
| Groups, and which items are pre-checked | Pass. "Broken 3" lists "Claude Code's hooks are incomplete" and the Codex hooks item (both checked), plus "Codex doesn't trust Marshell's hooks yet" with no checkbox and "Marshell can't trust hooks for you. Open /hooks in Codex and trust them.". "Info 2" lists "Claude Code deletes transcripts after 30 days" (unchecked, naming the other three CLIs) and the Codex version note. |
| "Fix 2 issues" becomes "Fix 3 issues" when retention is ticked | Pass. Done with Tab and Space alone. |
| The fix sheet shows the exact changes | Pass after fix 1. The Claude Code diff adds back `"Notification"` and `"Stop"` and adds `"cleanupPeriodDays": 365`. The Codex diff points the moved entry back at `mh/bin/marshell-hook.exe`. |
| Allow repairs the files, ticks the rows and shows the toast | Pass. Three rows read "FIXED", the toast says "Fixed 3 issues." with Undo, and the announcer said "Fixed 3 issues.". Codex's `hooks.json` is back to its setup hash `7E21C58F96BA`. Claude Code's file has all 11 events and `cleanupPeriodDays: 365`. `mh/doctor/last-fix.json` was written. |
| Run again leaves only Codex trust | Pass. "1 issue · Nothing to fix", with "Undo last fix" in the header. |
| Undo puts every file back | Pass. "The last fix is undone." Both files match their seeded hashes, `managed.json` is back to `D4F57A31EC29`, `last-fix.json` is gone, and the check ran again ("3 issues · 2 can be fixed"). |
| Undo last fix survives an app restart | Pass. Fixed again (`hooks.claude`, `hooks.codex`), then the app restarted. The startup run gave "Doctor: 1 issue", the page offered "Undo last fix", and Enter on it brought back the seeded hashes and `D4F57A31EC29`. |
| Keyboard only | Pass after fix 4. Tab reaches every control in order, and Space and Enter work. In the fix sheet, Tab walks "Changes to review", "Show all 154 lines", Cancel and the fix button, then wraps. Escape closes the sheet only and gives focus back to the page's fix button. A second Escape closes the Doctor. |
| Forced colours (emulated) | Pass. Rows, checkboxes, glyphs and buttons draw in system colours. The sheet is outlined, and the diff marks lines with `+` and `-`, not by colour alone. |
| The sidebar count follows the summary | Pass after fix 2. It read "Doctor: no issues", then "Doctor: 3 issues" and "Doctor: 1 issue", following each run. |
| The tray's Doctor item | Not checked live: it needs desktop input. The `trayItems` tests cover its label. |
| "Restart them" in the toast | Not checked live: no agent session was started. The `doctorRestart` tests cover it. |
| WSL distros | Not checked live (decision 7, plan Task 5). `doctor_api` covers them with a pinned distro list. |

## Findings

1. **A long fix pushed the sheet's buttons below the window.** The sheet's row was sized to its content, so the sheet's `max-block-size: 100%` had nothing to cap against. Cancel and the fix button sat at y = 1501 in a 780 px window, where the mouse can't reach them. Fixed in ae94855 (`LiveDoctor.css.test.ts`, failing first): the row is `minmax(0, 1fr)` and the sheet is placed with `safe center`. The sheet now spans y = 56 to 764, and its body scrolls.
2. **One issue read in the plural.** The sidebar said "Doctor: 1 issues", and the summary after a fix said "1 remain". Fixed in ab32007 (`Sidebar.test.tsx` "names one doctor issue in the singular" and `DoctorPage.test.tsx`, both failing first).
3. **The Codex row called this home's bridge missing while it was there.** Codex and Antigravity read an entry that runs another bridge as Orphaned, and the row's copy always said "run <this bridge>, which isn't there". Fixed in 9d358df (`hooks_pointing_at_a_moved_bridge_row`, failing first). A new fact, `bridge_exists`, picks the copy: "Codex's hooks point at a moved program" with "Some of Marshell's entries in <file> run a program other than <bridge>. The fix points them at it again.". This was seen live after the restart.
4. **Escape in the fix sheet closed the Doctor too, and each Tab in the sheet moved twice.** The sheet is a dialog inside the Doctor, and both focus traps handled the same keydown. After the sheet closed itself, the Doctor's trap saw no sheet and closed as well. Fixed in 9651e5d (`dialogFocus.test.ts` "leaves a key that a dialog inside it already handled", failing first): a dialog skips a key whose default an inner dialog already prevented.
5. **The fix sheet names each file three times:** in the sheet's heading, in the diff's file row with the counts, and in the `---`/`+++` lines. Not fixed (minor).
6. **"3 issues · 2 fixed · 1 remains" beside "Fixed 3 issues.".** The summary counts issues (Broken and Waste), and the toast counts the fixed items, as decision 2 says "Fix X issues" does. The retention item is Info, so the two numbers differ when it is ticked. Not fixed (minor).
7. **The sidebar said "Doctor: no issues" right after onboarding set up hooks.** The startup run happened before the setup, and decision 3 doesn't list a setup among the times the check runs. The count was right as soon as the page opened. Not fixed (minor).
8. **Vite's hot reload of `dialogFocus.ts` left stale key handlers in the open Doctor,** so Tab still moved twice and Escape did nothing until a full reload. This happens only in dev, and the results above come from the full reload.
