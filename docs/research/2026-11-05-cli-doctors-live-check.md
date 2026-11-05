# CLI doctors and duplicate notifications live check (slice 7b)

- Date: 2026-11-05, 21:07 to 21:14 local time.
- Build: `phase-7b` at 447e355 under `pnpm tauri dev` on Windows 11 Pro (10.0.26300). Claude Code 2.1.318 and Codex CLI 0.164.0 were on PATH.
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR` and `MARSHELL_CLAUDE_SETTINGS`, a short scratch `CODEX_HOME` (`%TEMP%\lc-7b-cx`) and a scratch `USERPROFILE` for Antigravity and Qwen Code, with CDP on port 9333. Onboarding set up all four CLIs' hooks. Codex's hooks were left untrusted, and Codex had no login.
- Data: before the start, `%TEMP%\lc-7b-cx\config.toml` was seeded with `notify = ["notify-test.exe", "turn-ended"]`. Nothing else was seeded.
- How it was driven: CDP evaluations returning small JSON, a CDP Enter key and emulated forced colours (not a real high-contrast theme). The banners were turned off through the scratch app's own `PUT /v1/settings`. One screenshot judged the look. There was no desktop input.

## Results

| Check | Result |
| --- | --- |
| Codex's own notify command is reported while banners are on | Pass. Info lists "Codex runs its own notify command" with "After each turn it runs notify-test.exe turn-ended. If that shows a desktop notification, you get two." and no checkbox. |
| "Run CLI doctors" runs both doctors and folds their findings into the groups | Pass. The button read "Running CLI doctors…" with `aria-disabled="true"` and no `disabled`, kept focus, and came back after about 2.2 s. Broken gained "Codex's doctor: no Codex credentials were found". Info gained the Claude Code doctor's warnings (one item each), Codex's `git.worktree.dev_drive`, websocket and `security.endpoint` warnings, "Antigravity has no doctor command." and "Qwen Code has no doctor command.". |
| The items are report-only | Pass. No CLI item has a checkbox. Each one reads "Marshell reports it and doesn't change it." and shows the CLI's own fix text. |
| A second click while they run changes nothing | Pass. |
| The finish is announced, and the sidebar count follows | Pass. The announcer said "The CLI doctors found 1 issue.", and the sidebar went to "Doctor: 2 issues". |
| The CLI items survive a background run | Pass. After "Run again" the page read "Checked 2 s ago", the status line still said "CLI doctors ran 19 s ago. Codex's doctor contacts OpenAI.", and all 10 CLI rows were kept. |
| Antigravity's own toast (decision 4) | Not run as a session. The binary has no toast code (finding 4), so with the user's OK the item was dropped rather than made Info, and there is no fix or Undo to check. |
| Banners off: no `notify.*` item | Pass. With the needs-you, done and error banners off, a fresh run (21:13:19) had no `notify.*` item, and the 10 CLI items were still there. |
| Keyboard only | Pass. Enter on the focused "Run CLI doctors" started a run. Focus stayed on the button through "Running CLI doctors…" and back, with a solid outline. |
| Forced colours (emulated) | Pass. The status line and buttons draw in system colours. |
| The real `~/.claude/settings.json` | Pass. Its modified time was `2026-11-05T20:29:11.9580386+02:00` before and after. |

## Findings

1. **The onboarding loop's path check matched nothing, so Antigravity and Qwen Code were set up without their target paths read.** The regex in the driving script was wrong, not the product. Checked afterwards: the real `~/.gemini/antigravity-cli` files were last written on 17 October, the real `~/.gemini/config/hooks.json`, `~/.agents/skills/marshell` and `~/.qwen` don't exist, and the writes landed in the scratch home. No product fix needed.
2. **Claude Code's doctor warns about the scratch home's `.local\bin`.** With `USERPROFILE` pointed at the scratch home, `claude doctor` reports that the native install's folder is missing from PATH and that the install method isn't set. These warnings are true for that home, and they show the long text wrapping in a row title. Not a product issue.
3. **Codex's remediation for the Dev Drive warning reads "https://learn.microsoft.com/en-us/<redacted>".** Codex's own JSON prints that text, and Marshell shows what it gets. Nothing in Marshell redacts doctor output. Not fixed (Codex's choice).
4. **agy 1.4.12 has no desktop notification of its own.** Its binary holds no `Windows.UI.Notifications`, toast or AppUserModelID code, and its setting "Bell + desktop notification on tool confirmation and agent completion" sends BEL and OSC 9 into the terminal. Inside Marshell those are ignored on hooked rows, so they never double. That is why Antigravity's `notifications` gets no item (decision 4 amendment, 447e355).
