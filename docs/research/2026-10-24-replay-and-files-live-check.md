# Replay and files changed live check (slice 5c)

- Date: 2026-10-24, 02:50 to 03:11 local time.
- Build: `phase-5c` (08f7255, then the fixes 5d52c94, 1f96d8b and 443b727 through Vite's hot reload) under `pnpm tauri dev` on Windows 11 Pro (26300).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR` and `MARSHELL_CLAUDE_SETTINGS` from `live-check.ps1 start`, with CDP on port 9333. The onboarding was left with Continue, then "Skip: use limited status", so no hooks were installed.
- Data: a driver script against the core API (`node drive-5c.mjs <scratch>`). It opened two PowerShell tabs with `POST /v1/sessions` that printed timed output, and posted Claude Code hook events with a hand-written transcript, so one conversation (`5c000000-…c0de`) spans both recordings:
  - Tab A: turn 1, an Edit of `src/app.ts`, then turn 2 with no edits. Its output includes a fake key `sk-ant-abc` + `defgh12345678` written in two pieces 5 ms apart, then a 12 s idle.
  - Tab B: a `SessionStart` with `source: "resume"`, then turn 3, an Edit of `notes.md`.
- How it was driven: the webview over CDP, with evaluations returning small JSON, CDP key events, emulated forced colours and light scheme (not a real high-contrast theme), and five screenshots. No desktop input was sent, and the native Save dialog was not opened.
- Real config: `~/.claude/settings.json` had the same modified time (01:47:20.3745995) before and after.

## Results

| Check | Result |
| --- | --- |
| The sidecar names the recordings | Pass. `meta.recordings` lists `t99ee7ed6b680-1792799925175.cast` and `t0e5c8dcc1e8b-1792799949945.cast` at version 2. |
| Parts and the conversation's span | Pass. `GET …/recordings` gives two parts at 100x30, part 0 clipped to [1.6 s, 28.9 s] of a 102 s file, part 1 to [0, 4.1 s]. Frames from part 0 give a 30-row screen and 36 events. |
| Tabs | Pass. "Conversation", "Recording" and "Files changed (2)" form a tablist with a roving tabindex, and End selects Files changed. |
| Play from here | Pass after finding 1. Every turn has "Play turn N from here". Turn 1 opens part 1 at 0:00 and plays; turn 3 opens "Part 2 of 2" at 0:01. |
| Playback, speed and idle cap | Pass. At 1× with "Up to 2 s", the 12 s idle takes about 2 s. At 4× with "Keep", the first 3 s take under 1 s and the 12 s idle about 3 s. Play becomes Pause while it runs and Play at the end; Space in the screen toggles it, and at the end starts again from the range's start. |
| Marks | Pass after finding 1. Previous mark from the end lands on turn 2 (0:17), Next mark returns, and each button is `aria-disabled` when it has no target. |
| Whole recording | Pass. The slider goes from "0:24 of 0:27" to "0:25 of 1:42" and keeps the position; unchecking returns to the span. |
| Parts | Pass. "Part 1 of 2, started Oct 24, 2026, 2:58 AM", with Previous part disabled on part 1 and Next part disabled on part 2. |
| Esc in the player | Pass after finding 2. Esc in the screen moves focus to the Recording tab and leaves History open; Esc there closes the conversation back to the list. |
| Files changed | Pass. "2 files changed", the note "Only edits made with Claude Code's Edit and Write tools are listed. Changes made through shell commands, such as sed or a code generator, aren't.", `src/app.ts` and `notes.md` each "+2 −1" with their diff, and "Go to turn" switches to Conversation with turn 3 in view. |
| Asciicast export | Pass. Through the API: valid v2 with the title "Fix the greeting in src/app.ts", the parts joined by `[17.614,"m","Part 2"]`, the key hidden with `redact=true` and shown with `false`. In the dialog: "Asciicast (.cast)" is reached with the arrow keys, "Hide secrets and personal paths" is on, the preview starts with the header and `[0.000000,"r","210x55"]`, "The preview shows the first 200 lines." shows, and there is no Copy button. |
| Forced colours | Pass (emulated). Buttons, selects and the screen carry system-colour outlines, and disabled buttons use GrayText. |
| Light and dark | Pass. The screen takes the terminal theme's background (white in light, black in dark), and playback text stays readable in both. |

## Findings

1. **"Play from here" for a turn after a resume opened the old tab's recording.** Tab A kept recording after the conversation moved to tab B, so both parts covered turn 3's time, and the first match (tab A, idle) won. Turn 3's start also showed as a mark in part 1. Fixed in 5d52c94: of the parts that cover a moment, the one that started last owns it, for the request and for the marks. Failing first: "opens the later of two parts that both cover the request, as after a resume in a new tab" and "leaves a turn that belongs to a later part out of an older part's marks" in `RecordingTab.test.tsx`.
2. **Esc in the player did nothing.** xterm cancels Esc on its helper textarea (`preventDefault` and `stopPropagation`), so neither the tab panel's handler nor History's saw it. Fixed in 1f96d8b: the player's custom key handler leaves Esc to the page. Failing first: "leaves Esc to the page…" in `castPlayer.test.ts`; the focus move was checked live.
3. **The recording was confined to the 80ch reading column.** Tab A had been resized to 210x55 once the window attached to it, so at the 8 px floor half of each line was clipped in the 513 px column, while the rest of the pane was empty. Fixed in 443b727: the conversation drops its width cap while the Recording tab is open (a `:has()` rule). The same commit makes a later change to the terminal font size refit the player rather than override the fit. Checked live (the screen is 1,097 px wide, and 210 columns fit at 8 px); no unit test, since the fit needs a DOM.
4. **The Files changed tab shows each path three times:** the file heading, the shared diff view's caption, and its `---`/`+++` lines. Not fixed; a minor.
5. **The webview console had a 404 for `ws://…/v1/pty/t99ee7ed6b680`.** The route answers 404 only for an unknown tab, and this branch doesn't touch the pty socket or the session code. Not investigated here.

## Re-check after the review fixes

- Date: 2026-10-24, 03:43 to 03:50 local time, on `phase-5c` at ccf30fd, with a new scratch home and the same driver script. No desktop input was sent; `~/.claude/settings.json` kept its modified time (03:18:31, set before the re-check started) throughout.

| Check | Result |
| --- | --- |
| Recording links (ccf30fd) | Pass. The sidecar gives tab A `first_ms` at its SessionStart and `last_ms` at its last Stop, and tab B its own. Both parts stay inside their links. Part 0 is unclipped here, as tab A's last Stop came after its last output. |
| Play from here keeps focus (68b6df7) | Pass. "Play turn 2 from here" leaves focus on the Recording tab, which is selected. |
| Tab leaves the screen (244952a) | Pass. Tab from the screen's textarea moves focus to Play, outside the xterm. |
| Go to turn keeps focus (68b6df7) | Pass. "Go to turn 3" selects Conversation and focuses turn 3's prompt, "Update the notes after the resume", in view and `:focus-visible`. |
| Seeks during playback (a47e059) | Pass. Four ArrowRight presses on Position while playing end at 0:20, paused, with no problem line, and the screen shows each line of tab A once, with nothing from before the seeks. Ending paused is the deferred minor. |
