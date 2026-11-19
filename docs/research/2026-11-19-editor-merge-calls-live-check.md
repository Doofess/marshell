# Editor merge: the user's three calls, live check (slice 6q-4b follow-up)

- Date: 2026-11-19, 11:51 to 11:58 local time.
- Build: `editor-merge-calls` (from `main` at `93cba4c6`, the code of `05d596fe` before its last fix, then with it through Vite's reload) under `pnpm tauri dev` on Windows 11 Pro (10.0.26300).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `USERPROFILE` and `MARSHELL_CLAUDE_SETTINGS`, with CDP on port 9333. Onboarding was skipped for every agent ("Skip: use limited status"), so no hooks were installed, and no agent CLI was started. `~/.claude/settings.json` kept its modified time (11:38:58).
- Data: a scratch project folder with `ro.txt` and `rw.txt` (10 lines each) and `conf.md`, `conf2.md` and `clean.md` (a heading and three paragraphs), written by PowerShell. The same commands changed each file on disk when the check called for it and set and cleared `ro.txt`'s read-only attribute. The files were opened with `marshell open` typed into a plain PowerShell session.
- How it was driven: CDP keys and CDP evaluations, and a recorder on the announcer's live region. No screenshots, no emulated media, no desktop input.

## Results

| Check | Result |
| --- | --- |
| 1. A read-only tab's bar has no Merge (decision 7.1) | Pass. With `line 2 mine` typed in `ro.txt`, line 9 changed on disk and the file then made read-only, the tab read "ro.txt is read-only on disk." and the bar "ro.txt changed on disk." with "Keep mine" and "Take disk" only. Once the attribute was cleared, the notice went and the bar offered "Merge", "Keep mine" and "Take disk" again. |
| 2. A merge with a conflict in the Rendered view shows the Source view, the cursor on the conflict and the keyboard in the editor (decision 7.2) | Pass after finding 1. `conf2.md` with "First paragraph. Mine." typed in Source, the tab back in Rendered, and "First paragraph, from disk." written on disk: Enter on Merge switched the toggle to Source, put the cursor at Ln 3, Col 1 (`<<<<<<< mine`) and the keyboard in the editor (`cm-content`). The line read "Merged conf2.md. 1 conflict to resolve. F7 goes to the next conflict, and Ctrl+Z undoes the merge." Both of its keys then did what it says: F7 from Ln 1 went to Ln 3 and announced "Conflict 1 of 1."; Ctrl+Z put "First paragraph. Mine." back, the bar came back with its three buttons, and "conf2.md changed on disk." was announced. |
| 3. The same merge while the person is in the terminal: the view switches, the keyboard stays | Pass. Merge was pressed in the Rendered view and the keyboard moved to the session's terminal in the same script, before the answer. The toggle went to Source, the cursor to Ln 3, the merge's line was announced, and the keyboard was still on "Terminal input" a second later. |
| 4. A clean merge in the Rendered view stays there and leaves Ctrl+Z out (decision 7.3) | Pass. `clean.md` with the first paragraph edited and the third changed on disk: after Merge the tab stayed in Rendered and showed "First paragraph. Mine." and "Third paragraph, from disk."; the line and the announcer read "Merged clean.md. No conflicts."; the keyboard was on "clean.md, contents". |
| 5. Nothing else changed: a clean merge in a text tab, and the toggle pressed by the person | Pass. In `rw.txt` (line 2 edited, line 9 changed on disk) Merge left both changes, the keyboard in the editor, and "Merged rw.txt. No conflicts. Ctrl+Z undoes the merge." was shown and announced. Pressing the Source toggle in `clean.md` still put the keyboard on the tab's body ("clean.md, contents"), as before. |

## Findings

1. **The pane took the keyboard back from the editor after the switch to Source.** In the first run (`conf.md`, before the last fix) the view switched and the cursor went to the conflict, but the keyboard ended on the tab's body. The pane focuses its body whenever the tab's entry changes (`Preview.tsx`, "Focus goes into the scroll body when the tab goes somewhere"), a view switch makes a new entry, and that effect runs after the text tab's own. It would also have pulled the keyboard out of the terminal when the answer came while the person was there. Fixed in `05d596fe`: a view switch the text tab asks for after a merge is quiet (`bodyTakesFocus`), with failing-first tests in `Preview.test.tsx`; a quiet switch that never comes keeps no later move quiet. Checks 2 and 3 are the runs after the fix.
2. **The browser console showed one error, a 404 for `/favicon.ico`** from the dev server at the first load, before anything was driven. Not related.

## Recheck after the review (2026-11-19, 12:11 to 12:17)

The review of PR #143 found that the waiting jump and the pane's quiet view switch could outlive a switch that never came, and that "the keyboard on the body" missed a keyboard inside the rendered text. The fixes tie the jump to the draw that brings the merge's answer, end a quiet switch with the person's own, and test that the keyboard is inside the body. A merge answered after the file became read-only is now a failure. A fresh scratch home (`lc-6q4b-fix-2`) under `pnpm tauri dev`, onboarding skipped as before, `a.md` and `b.md` (a heading and three paragraphs) opened with `marshell open`, and line 3 changed on disk by PowerShell after an edit in the Source view:

| Check | Result |
| --- | --- |
| A. A merge with a conflict in the Rendered view, Enter on Merge | Pass. `a.md`: the toggle went to Source, the cursor to Ln 3, Col 1 (`<<<<<<< mine`), the keyboard to "a.md, editor", and the announcer read "Merged a.md. 1 conflict to resolve. F7 goes to the next conflict, and Ctrl+Z undoes the merge." |
| B. The person's own toggle after that merge | Pass. With the cursor moved to Ln 12, Rendered and then Source put the keyboard on "a.md, contents" each time, and the cursor stayed at Ln 12. |
| C. The keyboard moved to a link in the rendered text while the merge ran | Pass. `b.md`: Merge clicked and the keyboard put on the rendered "a link" before the answer. The toggle went to Source, the cursor to Ln 3, Col 1, and the keyboard to "b.md, editor". |

The read-only race was not run live; `a_merge_answered_after_the_tab_became_read_only_is_not_taken` covers it. The keyboard-in-the-terminal case (check 3) was not run again; its path changed only from "is the body" to "is inside the body". The app was stopped by PID (23 processes, no agent CLIs), and `~/.claude/settings.json` kept its modified time (11:38:58).

## Second recheck (2026-11-19, 12:23 to 12:26)

The second review asked for two keyboard fixes. "The keyboard is in the body" is now read when the merge's answer comes, before the Source view hides the rendered text, instead of after. Keep mine takes the keyboard when Merge goes from under it because the file became read-only. A fresh scratch home (`lc-6q4b-fix-3`), set up as above, with `r.md` and `a.md`:

| Check | Result |
| --- | --- |
| D. The keyboard on Merge when the file becomes read-only | Pass. `r.md` with an edit and line 3 changed on disk, the keyboard on Merge, then the read-only attribute set by PowerShell: the tab read "r.md is read-only on disk.", Merge went, and the keyboard was on "Keep mine". |
| E. Check A again with the new capture | Pass. `a.md`: Enter on Merge in the Rendered view gave Source, Ln 3, Col 1, the keyboard on "a.md, editor" and "Merged a.md. 1 conflict to resolve. F7 goes to the next conflict, and Ctrl+Z undoes the merge." |

The app was stopped by PID (23 processes, no agent CLIs), `r.md`'s attribute was cleared, and `~/.claude/settings.json` kept its modified time (11:38:58).
