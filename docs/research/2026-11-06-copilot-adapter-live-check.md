# Copilot CLI adapter live check (slice 6n1)

- Date: 2026-11-07, 00:27 to 00:51 local time (2026-11-06, 22:27 to 22:51 UTC).
- Build: `phase-6n1` under `pnpm tauri dev` on Windows 11 Pro (26300.9818, 26H2), with GitHub Copilot CLI 1.0.106 from winget (`%LOCALAPPDATA%\GitHubCopilotCLI\copilot.exe`).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `MARSHELL_CLAUDE_SETTINGS`, `COPILOT_HOME` and `USERPROFILE` under `%TEMP%\lc-6n1-3`, with CDP on port 9333. Copilot talked to the plan's fake OpenAI-compatible model on loopback (decision 8), so there was no GitHub sign-in and no cost. Onboarding was left with Copilot CLI's hooks set up, then turned off at the end.
- Data: the scratch Copilot `settings.json` was seeded with a status line of the person's own, `{"statusLine": {"type": "command", "command": "echo mine"}}`, beside a comment and two other keys. A copy was kept as `settings.before.json` (SHA-256 `237CAE74…26C9`). The project `proj` had the reference alias `notes` → `notes.txt`.
- How it was driven:
  - CDP evaluations and CDP key events from small node scripts. The `webview` MCP server did not connect.
  - The scratch core's local API, with the token from the scratch `endpoint.json` only.
  - A watcher that logged every session's live state every 150 ms.
  - One screenshot. No desktop input.
- Earlier attempts:
  - `lc-1` was a spike.
  - `lc-2`, under `C:\dev`, was stopped unused: see finding 1.

## Results

| Check | Result |
| --- | --- |
| Set up Copilot's hooks from the Agents section. Read the diff for `marshell.json`, `settings.json` and the chain, and the approve switch's reason. | Pass. `hooks\marshell.json` holds 9 entries, one each for `sessionStart`, `userPromptSubmitted`, `preToolUse`, `postToolUse`, `postToolUseFailure`, `notification`, `agentStop`, `errorOccurred` and `sessionEnd`, and no `permissionRequest`. Each runs `mh/bin/marshell-hook.exe event --agent copilot --event <name> --tag mshl1`. In `settings.json` only `statusLine.command` changed, to `marshell-hook.exe statusline --agent copilot --tag mshl1`, and the comment and the other keys were kept. `mh\agents\copilot\statusline-chain.json` holds `"command": "echo mine"`. The consent card's approve switch is off with "Copilot CLI asks in its own terminal. Its hook can't tell which requests Copilot would allow by itself." |
| Open a Copilot tab in Manual from the launcher. The argv runs `copilot.exe` by its full path, with `--session-id <uuid>`. | Pass. `"C:\Users\Home Office\AppData\Local\GitHubCopilotCLI\copilot.exe" --session-id 7bf20250-6e19-4f4a-8512-f88dd724cd66`. The row was Idle with the model `marshell-fake` from the status line at launch, before any prompt. |
| Send `RUN New-Item a.txt`. The row goes Working, then Needs permission, and the lane shows the one-liner with no card. | Pass. "Thinking", then "Running \`New-Item a.txt\`", then "Wants to run \`New-Item a.txt\`" within 0.4 s. The lane read "Needs you · 1" with one row and no approve card. |
| Answer Copilot's dialog in the terminal. The row goes Working, then Idle with the model and context %. | Pass, with Done in place of Idle. After the answer the row read "Done" with model `marshell-fake` and context 1%, which is the state every adapter CLI ends a turn in. `a.txt` was created. |
| Send `RUN echo marshell`, which Copilot allows by itself. No Needs permission and no card. | Pass. "Thinking", then "Running \`echo marshell\`", then "Done", with no Needs permission and nothing in the lane. |
| With a project alias, send a prompt naming it: the note is in the fake log. | Pass. The fake model's request held "References: 'notes' = C:\Users\Home Office\AppData\Local\Temp\lc-6n1-3\proj\notes.txt." inside Copilot's `<system_reminder>`. |
| `/clear` in the tab: the row is Idle within a second, and Restart's argv is `--resume=<the new id>`. | Pass after one prompt; see finding 3. The row went "Ended", then Idle, 0.17 s later. Copilot went on under the new id `666739ee-…`, which had no `events.jsonl` until a prompt, so Restart started fresh with `--session-id b5b6e70c-…`. In a second tab, `/clear` and then one prompt gave the new id `8fea4ec8-…`, and Restart in Plan ran `& '…\copilot.exe' --resume=8fea4ec8-3a04-4c89-9da4-bdb30ded74f9 --plan`, with that command shown in the Restart bar. |
| Restart the tab after `/exit` and read the argv: `--resume=<the same uuid>`. The row is Idle before the first prompt. | Pass. After `/exit`, Restart in Manual ran `"…\copilot.exe" --resume=8fea4ec8-3a04-4c89-9da4-bdb30ded74f9`, the same id, and the row was Idle 2.8 s later, before any prompt. That `/exit` came before the resumed session's first prompt, so the row stayed Idle at the shell prompt until the restart (finding 4). |
| Two Copilot tabs ask at once. Each row shows Needs permission, and the lane lists both. | Pass after a fix (finding 2). Rows 1 and 2 read "Wants to run \`New-Item c.txt\`" and "Wants to run \`New-Item b.txt\`". The lane read "Needs you · 2" and, after f381af1, showed both rows: "Copilot CLI 2 Wants to run \`New-Item b.txt\` 4m. Needs permission." and "Copilot CLI Wants to run \`New-Item c.txt\` 3m. Needs permission." in a 100 px lane. |
| Turn the hooks off: `hooks\marshell.json` is gone, the scratch `settings.json` is byte-equal to what it was before the setup, and the chain file is gone. | Pass. The preview, "Turn Copilot CLI hooks off", named the three files (+1 −156). After it, `marshell.json` and `statusline-chain.json` were gone and `settings.json` hashed `237CAE74…26C9`, the same as `settings.before.json`. The Agents entry read "Not set up. Sessions show limited status." |
| The real homes come out untouched. | Pass. `~/.claude/settings.json` kept its modified time (2026-11-06T15:24:29Z), `~/.copilot` still had its 8 files (newest 2026-10-24), and `~/.claude/agents` and `~/.marshell` had nothing newer than before the check. |

## Findings

1. **A scratch folder outside the real profile is refused by the config guard.** In `lc-2` under `C:\dev`, every card said "The path is outside the home folder and the project folders.": `Guard::for_home` allows `dirs::home_dir()`, which is the real profile, not `USERPROFILE`. The check moved to `%TEMP%`, inside the real profile, as the live-check skill's scratchpad rule already implies. Not a product bug. Not fixed.
2. **The lane kept a card's 256 px for sessions that ask in their own terminals.** With two Copilot tabs waiting and a 740 px sidebar, the lane was 284 px tall, showed one 36 px row and hid the other behind "+1 more, show all 2 requests". Copilot asks only in its own terminal (decision 12), so no card can ever be targeted. Fixed in f381af1: `laneLayout` takes whether a card can be targeted, and Lane and Sidebar pass it. Failing first: `laneLayout.test.ts` "keeps no room for a card when no card can be targeted…" and `Lane.test.tsx` "shows two sessions asking in their own terminals as two rows…".
3. **Restart after a `/clear` with no prompt starts a fresh session.** Copilot writes `session-state/<id>/events.jsonl` only at a session's first prompt, and Restart resumes an id only when that file exists, so it can't resume the post-`/clear` id. It doesn't fall back to the id from before `/clear`, because that conversation was cleared. By design (decision 10). Not fixed.
4. **A Copilot that exits before its first prompt sends no `sessionEnd`, so its row stays Idle at the shell prompt.** Copilot creates its hook session at the first prompt; before that, `/exit` logs "no hook session for session id" and fires no hook. This holds for a resumed session too. An agent tab has no process sampling, so nothing else tells the core the agent is gone. The row is right again once the tab is restarted. Known limit. Not fixed.
5. **Copilot in a tab reads the real `~/.claude/agents/*.md`.** Copilot loads Claude-format agent definitions from the profile, and the tab sees the real profile, not the app's scratch `USERPROFILE`. Copilot only reads them; nothing under the real `~/.claude` changed. Not fixed.
6. **The consent card says "Approve from anywhere needs live status…" for Copilot,** though its approve switch is off with its own reason. Cosmetic: the line comes from the shared card. Not fixed.
7. **The launcher names it "GitHub Copilot" and the rest of the app "Copilot CLI".** The launcher shows the manifest's display name. By design. Not fixed.
8. **The Restart bar shows the core's real command,** `& '…\copilot.exe' --resume=<id> --plan`, with the full path. By design. Not fixed.
9. **Pressing Enter for Copilot's folder trust also answered its first permission dialog.** At 22:43:00 tab 2 went to Done and `b.txt` was created the same second. Later, a palette chord that was the wrong one (Ctrl+K, where the palette is Ctrl+Shift+K) sent its text and Enter into tab 1, which approved `New-Item c.txt`. Both were the check's own input to sessions it started. Not a product bug.

## Known limits

- Esc in Copilot's dialog fires no hook (seen in the spike), so the row keeps Needs permission until the next event.
- Each of autopilot's continuations is a `userPromptSubmitted` with Copilot's own text, so each counts as a prompt and gets the references note.
- A Copilot that exits before its first prompt keeps its row Idle (finding 4).
