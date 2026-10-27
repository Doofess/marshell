# Company tab parity live check (slice 6g)

- Date: 2026-10-27 to 2026-10-28, 21:48 to 00:39 local time (most of it waiting on agy's first run).
- Build: `phase-6g` under `pnpm tauri dev` on Windows 11 Pro (26300).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR` and `MARSHELL_CLAUDE_SETTINGS`, a short scratch `CODEX_HOME` under `%TEMP%`, and a scratch `USERPROFILE` (so agy's `~/.gemini` was a scratch folder), with CDP on port 9333. Started with `-AgyAuth -CodexHome`: agy's two sign-in files were copied into the scratch `.gemini` and deleted at stop. Onboarding was stepped through with all three hook consents skipped, so no hooks were installed anywhere. The real `~/.claude/settings.json`, `~/.codex/config.toml` and `~/.gemini/antigravity-cli/settings.json` kept their modified times.
- Data: the scratch files were written by hand between checks with `printf` (a commented Claude file, a Claude file with a repeated key, Claude and Codex files starting with a BOM, a Codex file with `check_for_update_on_startup = "yes"` and `project_doc_max_bytes = "abc"`, and an Antigravity file with `"colorScheme": "dracula"`). Settings was reopened after each write, since switching company tabs inside an open Settings doesn't read the file again.
- How it was driven: CDP evaluations and CDP keys from a script (the `webview` MCP server didn't connect), emulated forced colours (not a real high-contrast theme), 5 screenshots of the agy terminal, which draws on a canvas. No desktop input. One agy session was started for the check, in Manual mode; the check pressed Enter once in it, on agy's first-run colour scheme screen.

## Results

| Check | Result |
| --- | --- |
| Antigravity on a missing file | Pass. The tab showed the scratch path, "<path> doesn't exist yet, so every setting has its default." and "Antigravity is running in 1 tab. The update may fail while it runs.". Verbosity Low by keyboard, then Review, showed a new three-line file. Apply wrote `{ "verbosity": "low" }` (25 bytes) and showed "Saved. New Antigravity sessions use it.". |
| Tool permission → strict, Review, Apply, Undo | Pass. Review showed only the added `"toolPermission": "strict"` line. Undo showed "Put <path> back as it was." and the file was the same 25 bytes. |
| An Antigravity raw edit | Pass. The raw view was editable with no lock line. Adding `"model": "Gemini 3.1 Pro"` applied, and Undo gave back the 25 bytes. |
| The Telemetry row | Pass. Read-only with an en dash and "Antigravity keeps this with your Google account, not in this file. Change it with /config in Antigravity.". |
| A seeded `"dracula"` colour scheme | Pass. The select showed "dracula (from the file)" as its last option, selected, and nothing counted as an edit. |
| A commented Claude file | Pass. The raw view showed "Raw editing is off: this file isn't plain JSON, and Marshell saves it as plain JSON." and the file's own text, read-only. A form edit of Effort applied and kept the comment. Undo put back the exact bytes and focused "Put <path> back as it was.". |
| A Claude file with a repeated key | Pass. "Raw editing is off: a key appears twice in this file.", read-only. |
| A raw save keeps one BOM | Pass. The raw views showed the text without the BOM. After a raw save, the Claude file and the Codex `config.toml` each started with exactly one `EF BB BF`. |
| Odd values show as they are | Pass. `check_for_update_on_startup = "yes"` showed "yes (from the file)"; `project_doc_max_bytes = "abc"` showed `abc` in a text input with `inputmode="numeric"`. Neither counted as an edit. |
| `-` in a number | Pass. "Use a whole number of at least 0.", and Review kept focus on the field. |
| Discard returns focus | Pass. With focus moved elsewhere, Discard by keyboard focused the number field changed last and put `abc` back. |
| Reload returns focus | Pass. With the file changed underneath, Review showed "<path> changed since you opened it. Reload to see it.". Reload by keyboard focused the model field, the control focused before, and kept the Reasoning effort draft. |
| Busy buttons in forced colours | Pass. Captured while each request was in flight: busy Review, Apply and Undo, and the busy Discard and Back to editing beside them, computed both `color` and `outline-color` to the system GrayText. |
| Does agy undo a change Marshell makes while it runs? | Not run. agy's first run in the scratch home stops at "Terms of Service & Data Use", which agrees to Google's terms with the signed-in account and holds the telemetry choice. The check doesn't answer that for the user, and it wasn't answered before the check ended. agy wrote no `settings.json` through its first-run screens. |

## Findings

1. **The overwrite question stays open.** Without the probe there is no evidence either way that agy saves its settings over a change Marshell made while it runs. agy documents that it replaces the whole file in one step when it saves, so slice 6g adds decision 3's warning anyway (decision 9): Review says "Antigravity is running in 1 tab. If it saves its settings, it may undo this change." while agy runs. A later check can answer the question from a scratch home where agy's first run is already done.
2. **Switching company tabs inside an open Settings shows the file as it was when Settings opened.** A file changed by hand only shows after Settings is reopened, or as a conflict at Review, which then offers Reload. This predates slice 6g, and the conflict path keeps it safe. Not fixed.
3. **The review says "A backup is saved first, in ~/.marshell/snapshots/" under a scratch `MARSHELL_HOME`.** It is the fixed, approved copy used since 2026-10-10, and it is right whenever `MARSHELL_HOME` isn't set. Not fixed.
