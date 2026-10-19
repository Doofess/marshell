# Claude Code live status live check (slice 3c2)

- Date: 2026-10-19
- Build: `phase-3c2` up to 67ae4f1, `pnpm tauri dev` on Windows 11 Pro (26300), Claude Code 2.1.303
- ConPTY: the dev log read "ConPTY files not fetched; using the inbox ConPTY"
- Home: a scratch `MARSHELL_HOME` with a space in its path (`%TEMP%\marshell 3c2 home`)
- Target: `MARSHELL_CLAUDE_SETTINGS` set to `%TEMP%\marshell-3c1 project\.claude\settings.local.json`, which started as `{}` (no status line of the person's own). The project was the 3c1 check's, because Claude Code already trusted it
- Driven over CDP (`--remote-debugging-port=9333`) and the local API with the scratch home's token. Each Claude session was created with the prompt "Reply with only the word ok."

## Results

| Check | Result |
|---|---|
| Onboarding, Consent | Pass: "Add 8 hook entries and a status line in …settings.local.json." Allow applied it: 8 groups tagged `mshl1` and a `statusLine` pointing at the bridge |
| Row before the first hook | Pass: "Status arrives with hooks", then "Working" from the activity fallback while Claude Code started |
| SessionStart | Pass: the row went Idle |
| Status line | **Fail** in the first session, see finding 1. After the fix: the row read "Opus 5.5" before Idle, and the context ring appeared |
| UserPromptSubmit | Pass: "Opus 5.5 · Thinking", Working |
| Stop | Pass: Done, with an age counting 0s, 1s, 2s… |
| The core's record after Stop | Pass: `{state: done, phrase: "Done", recap: {text: "ok"}, model: "Opus 5.5", effort: "high", context_pct: 5, five_hour_pct: 5, seven_day_pct: 46, subagents: 0}` |
| Context ring | Pass: the row's label ends "Context 5% used" |
| Footer plan ring | Pass: this account sends rate limits, so the footer shows the ring and "5h 5%". It was announced as context, see finding 2 |
| The recap with its age | Not visible in the live app, see finding 4. The core holds it, and the expanded row draws it (`SessionRow` and `labels` tests) |
| Hook log | Pass: SessionStart, UserPromptSubmit and Stop |
| Agents dialog → Live status Off | Pass: the switch read Off, with "Rows use output activity only: Working or Idle.", and `settings.json` held `"status_off": ["claude"]` |
| Row with live status off | Pass: the core dropped `live` and set `status_note` to "Live status is off for Claude Code". The row read Idle with no model or ring, and its tooltip ended "Status limited · Live status is off for Claude Code" |
| A prompt with live status off | Pass: the row went Working, then Idle about 4 s later. Claude answered "ok" and ran its Stop hook, and the core kept `live: null` |

## Findings

1. **No status line reached the core when the person has none of their own.** The bridge's status line role returned before forwarding when the chain file held `statusLine: null`, the common case, so the model and the rings never arrived. Fixed in 543d923: the forward starts first, whatever the chain holds, and the chain decides only what is printed. Tests `a_null_chain_still_reaches_the_core` and `a_missing_chain_file_still_reaches_the_core` failed first.
2. **The footer's plan ring was announced as "Context N% used".** `ContextRing` now takes a label, the footer passes "5-hour plan", and the "5h N%" text is hidden from screen readers while the ring shows, so it is said once (8b8b283).
3. **An idle live row read "Opus 5.5 · "** with nothing after the separator. The separator now shows only when a phrase or "muted" follows, in the row and in its label (67ae4f1).
4. **The recap is not visible in the live app.** PLAN.md puts the recap in the expanded row ("Expanded … adds … the recap with its age"), and the live sidebar always draws rows comfortable: per-row expansion (→ or the disclosure control) is not built yet. Left as specified; it shows once rows can expand.

## Notes

- **Typing with live status off:** the input API refuses line breaks, so the prompt went in through `POST /v1/sessions/{tab}/input` and Enter as a trusted key to that tab only.
- **Reading the terminal:** xterm draws to a canvas here, so the terminal's text was read from the session's recording.
- **Cleanup:** the sessions were started for this check, and Enter was sent only into them. The last one was closed through the API, the app was stopped through the task that started it, and the app, the bridge, ports 1420 and 9333 and the check's Claude were confirmed gone. Both scratch folders were removed.
