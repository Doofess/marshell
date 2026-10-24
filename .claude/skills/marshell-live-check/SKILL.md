---
name: marshell-live-check
description: Use when a plan step needs the real Marshell app (pnpm tauri dev): live checks, exit checks, reproducing a UI bug.
---

# Marshell live check

A live check runs the dev app against scratch homes, drives the webview over CDP, writes what it found to `docs/research/`, and stops only what it started. The person's desktop runs real work beside it (other Claude sessions, Photoshop, the Codex app), and their real `~/.claude`, `~/.codex` and `~/.marshell` must come out untouched.

## Start

1. Pick a NEW literal scratch path under the session scratchpad, for example `<scratchpad>\lc-5b-1`. Never reuse or delete an old one to get a clean start; take the next number. Never name it `.marshell`.
2. Note `~/.claude/settings.json`'s modified time.
3. Start the app from the checkout under test:
   ```
   pwsh -NoProfile -File .claude/skills/marshell-live-check/live-check.ps1 start -Scratch <literal path>
   ```
   It creates `mh` (`MARSHELL_HOME`), `cc` (`CLAUDE_CONFIG_DIR`), `cx` (`CODEX_HOME`, so History's importer never reads the real `~/.codex`) and `proj/.claude/settings.local.json` seeded with `{}` (`MARSHELL_CLAUDE_SETTINGS`), starts `pnpm tauri dev` with CDP on port 9333, logs to `dev.out.log` and `dev.err.log`, and records the root PID in `pids.json`. It refuses a non-empty folder. `-Seed <folder>` first copies `<folder>\cc` and `<folder>\cx` into the new `cc` and `cx`, for hand-written transcripts and rollouts the app should find on its first run. `-CodexAuth` copies `~/.codex/auth.json`, and nothing else from the real Codex home, into `cx` so a real Codex can sign in; `stop` deletes that copy. `-CodexHome <short literal path>` (60 characters at most, for example `%TEMP%\lc-6a-4-cx`) puts `CODEX_HOME` there instead of `cx`, so a tab opened from the Codex profile can start Codex (see the Codex rule below). It refuses a non-empty folder, and that folder is deleted with the scratch folder at the end of the work. In that scratch `CODEX_HOME` the check may trust Marshell's own hook entries in Codex's `/hooks` (the user's permission, 2026-10-24); the product never does.
4. Wait until `http://127.0.0.1:9333/json/version` answers. The first Rust build can take minutes, so poll with a Monitor or an until-loop, not a fixed sleep.

## Drive it

The `webview` MCP server (`.mcp.json`) is Playwright attached to the dev webview over CDP. It connects on its first tool call, so start the app first.

- **Read state:** use `browser_evaluate` with a function that returns a small JSON value, for example `() => ({ title: document.title, focused: document.activeElement?.getAttribute('aria-label') })`. Prefer this to snapshots, and snapshots to screenshots. Take a screenshot only to judge the look.
- **Keys:** use `browser_press_key`. It sends CDP key events into the webview only.
- **Forced colors:** `browser_emulate_media` with `forcedColors: 'active'`. The emulation lasts only while that MCP connection is open. Record it as "emulated (not a real high-contrast theme)".
- If the tools don't appear, `/mcp` shows the server's state. The plugin `playwright` server launches its own browser, so it can't see the app.

## Rules the past checks taught

- **Claude Code sessions** ignore a PATH shim. Marshell's `pwsh` gets a fresh PATH and runs the real `claude.exe`, so the scratch `CLAUDE_CONFIG_DIR` set by `start` is what keeps Claude itself out of the real `~/.claude`. Marshell is kept out by `MARSHELL_CLAUDE_SETTINGS`: onboarding's "Set up" and the hooks consent install into that scratch file. Before pressing either, read the target path in the preview. Claude then stops at its login. If the check needs a real answer from Claude, ask the user first.
- **`/model` and `/effort`** typed into a Claude session write the real `~/.claude/settings.json`, even with a scratch home. Test model and effort with `claude --model <x> --effort <y>` at launch instead. If one slipped through, restore only the changed keys from `C:/dev/claude-setup/settings.json`.
- **Enter and input:** the agent sessions in a check are ones you start for it. Pressing Enter in one is part of the check, never part of the feature. Never send input to a session the user started.
- **Desktop input:** CDP input reaches only the webview. Anything outside it, such as the tray, a native menu, a notification or the window frame, needs real input. Ask before sending it. Before any `SetCursorPos`, `mouse_event` or `SendKeys`, check that the target rect is non-empty and belongs to the expected process, and abort otherwise. Prefer UIA Invoke. A stray click once landed in the user's Photoshop.
- **Codex sessions:** under a scratch `CODEX_HOME` this long, Codex 0.163 quits at start with "path must be shorter than SUN_LEN", because its app-server socket doesn't fit. Marshell starts Codex 0.163 and newer with `--no-daemon` (slice 6a decisions 12 and 14), but a Codex typed in a shell tab starts the shared daemon, so give `start` a `-CodexHome` of at most 60 characters, such as `%TEMP%\lc-<slice>-cx`. `-a untrusted` is gone; `-s read-only -a on-request` (Manual mode) makes shell writes ask, because the Windows sandbox can't start a process in the scratch home and every command escalates. Hooks run only after **Trust all and continue** at Codex's start, and trust is keyed by each entry's position in `hooks.json`, so moving a group asks again.
- **Rust edits during a check:** `tauri dev` rebuilds on any change under `crates/` or `src-tauri/` and restarts the app with a new PID and API port. The old tabs' shells and agent CLIs keep running outside the new app's tree, where `stop` won't find them. Read the API port from `endpoint.json` on every call, and after a restart list the agent CLIs outside the tree and stop the check's own by PID. Frontend edits arrive through Vite's hot reload without a restart.
- **Fake agents:** a test copy of fake-agent gets a name no real program has (`codex-marshell-test.exe`) and runs by full path. `cmd` here doesn't search the current folder, so a bare `codex.exe` runs the real Codex.

## Stop

1. Print the tree, and read it before stopping anything:
   ```
   pwsh -NoProfile -File .claude/skills/marshell-live-check/live-check.ps1 tree -Scratch <path>
   ```
2. If it holds an agent CLI (`claude.exe`, `codex.exe`, `node_repl.exe`), decide which ones this check started. The dev window is on the user's desktop, and they may have used it. Ask about any other.
3. Stop the tree:
   ```
   pwsh -NoProfile -File .claude/skills/marshell-live-check/live-check.ps1 stop -Scratch <path> [-AlsoStop <pid,...>]
   ```
   `stop` refuses (exit 2, nothing stopped) while the tree holds an agent CLI not named in `-AlsoStop`. It stops by PID, leaves first, and reports anything still running. It never deletes the folder.
4. Check that `~/.claude/settings.json`'s modified time is unchanged.
5. Scratch folders are deleted only at the end of the work, by literal absolute path, after showing the user the list and getting an OK.

## The research note

Write `docs/research/<date>-<topic>-live-check.md`:

```markdown
# <Topic> live check (slice <x>)

- Date: <date>, <start> to <end> local time.
- Build: `<branch>` under `pnpm tauri dev` on Windows 11 Pro (<build>).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR` and `MARSHELL_CLAUDE_SETTINGS`, with CDP on port 9333. <What onboarding was left with.>
- Data: <what was seeded and how, with the command>.
- How it was driven: <CDP evaluations, CDP keys, emulated forced colors, N screenshots; any desktop input and why>.

## Results

| Check | Result |
| --- | --- |
| <exit criterion, in the plan's words> | Pass. <What was seen, with exact copy in quotes.> |

## Findings

1. **<One-line finding.>** <What happened, why, and the fix commit with its failing-first test, or "Not fixed.">
```
