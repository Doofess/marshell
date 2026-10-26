# Accounts and company tabs live check (slice 6e2)

- Date: 2026-10-27, 01:12 to 01:23 local time.
- Build: `phase-6e2` under `pnpm tauri dev` on Windows 11 Pro (build 26300).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR` and `MARSHELL_CLAUDE_SETTINGS`, and a short scratch `CODEX_HOME` under `%TEMP%`, with CDP on port 9333. Onboarding was stepped through with "Skip: use limited status" for all three CLIs, so no hooks were installed anywhere. Antigravity's consent named the real `~/.gemini`, and it was skipped; afterwards `~/.gemini/config/hooks.json` still did not exist.
- Data, all fake values:
  - `version.json` in the scratch `CODEX_HOME` with `latest_version` `99.0.0`.
  - The scratch `MARSHELL_CLAUDE_SETTINGS` file with `model`, `env.ANTHROPIC_API_KEY`, `apiKeyHelper` and `cleanupPeriodDays`.
  - A broken copy of that file, written later.
- How it was driven:
  - CDP evaluations from a script, because the `webview` MCP server didn't connect.
  - CDP keys: Tab, Shift+Tab, arrows, Enter, Escape and Ctrl+C.
  - Emulated forced colours (not a real high-contrast theme).
  - 6 screenshots.
  - API reads made from inside the webview, with its own endpoint, so the token never left the page.
  - No desktop input.
- The CLIs on this machine are all native installs: Claude Code 2.1.311, Codex 0.164.0 and Antigravity CLI 1.4.7. No Update button was pressed, and no CLI was updated.

## Results

| Check | Result |
| --- | --- |
| Accounts shows `Not signed in, checked just now` for Claude Code and Codex, each with its Sign in button, and Antigravity's sentence. Check again announces `Checked the sign-in status.` | Pass. The rows read "Not signed in, checked just now" with "Sign in to Claude Code", and "Not signed in, checked just now" with "Sign in to Codex". Antigravity reads "Antigravity has no sign-in status command, so Marshell can't show one." Check again announced "Checked the sign-in status." |
| Sign in to Codex opens a tab named "Sign in to Codex", Settings closes and the tab is selected. Ctrl+C ends it, and the status is announced. `GET /v1/sessions/{tab}/recording` is 404. | Pass. The tab "Sign in to Codex" opened and was selected, and Settings closed. `codex login` started its local login server and printed its sign-in URL; nothing was signed in. Ctrl+C in the tab ended it, exit code -1073741510 (Ctrl+C). The announcements were "Sign in to Codex has ended" and then "Codex: Not signed in, checked just now". The recording route answered 404. |
| The three company sections are listed only for the CLIs found. Each shows `Version <v>` and its Update button. Codex, with `version.json` seeded, shows `Version <n> is available.` | Pass after finding 1. The nav listed Accounts, Claude Code, Codex and Antigravity, between Agents and Advanced. The sections showed "Version 2.1.311" with "Update Claude Code", "Version 0.164.0" with "Version 99.0.0 is available." and "Update Codex", and "Version 1.4.7" with "Update Antigravity". |
| The seeded Claude file shows `hidden` for both secrets in the form, in Other settings and in the raw view, and the model in the form. A broken copy shows the parse line and nothing else. | Pass. Default model reads "opus" (User), and Delete transcripts after (days) reads "30". Under "Other settings in this file", `apiKeyHelper` reads "hidden" and `env` reads `{"ANTHROPIC_API_KEY":"hidden"}`. The raw view showed the same values as pretty JSON, read-only, with the note "Shown as Marshell reads it: comments and layout are left out, and secret values show as hidden." Neither fake value appeared anywhere in the page's HTML. The broken copy showed "…settings.local.json doesn't parse (line 6), so Marshell shows nothing from it.", every curated row as "–", no Other settings, and Raw file disabled. |
| The running note. | Not checked live. Unit tests cover it. |
| 720×480 at text size Larger: the nav scrolls on its own and the current section stays in view. | Pass. The nav was 340 px tall with 497 px of content. On Advanced it had scrolled 157 px and Advanced was in view; back on Appearance it returned to 0. The page had no horizontal overflow. |
| Keyboard only through Accounts and one company section. | Pass. On Accounts, Enter in the nav moved focus to the "Accounts" heading. Tab then went to "Sign in to Claude Code", "Sign in to Codex", "Check again" and "Back to sessions". On Claude Code, Tab went to "Update Claude Code" and then the Form radio. ArrowDown chose Raw file and showed the read-only text, and ArrowUp went back to Form. The radio pair is one Tab stop. In Codex, Raw file is disabled because `config.toml` doesn't exist, so ArrowDown stayed on Form. |
| Forced colours on both. | Pass, emulated. The Accounts cards stay told apart by their headings and gaps. Buttons and the nav items have system-colour edges. The disabled Raw file shows in GrayText. |

## Findings

1. **Codex's native install read as unknown, so Codex had no Update button.** Codex's installer makes `%LOCALAPPDATA%\Programs\OpenAI\Codex\bin` a junction into `~\.codex\packages\standalone\current\bin`. `current` is itself a junction to `releases\<version>\bin`. The core canonicalizes the exe before matching, which is what lets a WinGet `Links` path be recognised, so it saw the release folder, and that matched no native pattern. Fixed in 46c1365: a `.codex\packages\standalone\…\bin` path is native too. `native_paths` failed first, on the canonical path.
2. **A Claude settings file with a missing comma reads as valid.** This is not a bug from this slice. The shared JSONC reader (`config::jsonc`) accepts a missing comma between members, so the view shows the file's values. The broken-file check used a file with two values in a row and no closing brace instead. Not fixed.
