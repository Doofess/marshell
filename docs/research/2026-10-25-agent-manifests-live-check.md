# Agent manifests live check (slice 6b)

- Date: 2026-10-25, 03:26 to 10:36 local time. Most of that was a wait for the Grok sign-in. The CLIs were installed and their flags checked from 03:05.
- Build: `phase-6b` at 0ef4595 under `pnpm tauri dev` on Windows 11 Pro (26300).
- CLIs:
  - opencode 1.19.4, Qwen Code 0.26.1 and Crush v0.98.2, from npm into a scratch prefix.
  - Kimi Code CLI 2.3.1, from its install script into a scratch folder.
  - Grok 1.0.50 at the start of the check. It was 1.0.70 after the person signed in.
  - Claude Code 2.1.310, Codex 0.164.0 and Antigravity 1.4.0, as installed.
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR` and `MARSHELL_CLAUDE_SETTINGS`, and a scratch `CODEX_HOME`, with CDP on port 9333. Onboarding wrote the Claude Code hooks (11 entries and a status line) into the scratch settings file. The Codex hooks were skipped.
- Installs (6b decision 8): none of these CLIs was signed in.
  - npm: `npm install -g --prefix <scratch>\npm --cache <scratch>\npm-cache opencode-ai @qwen-code/qwen-code @charmland/crush`. It was run again with `--allow-scripts=@charmland/crush,opencode-ai`; `@qwen-code/audio-capture` stayed blocked.
  - Kimi: `code.kimi.com/kimi-code/install.ps1`, read first, then run with `KIMI_INSTALL_DIR`, `KIMI_CODE_HOME` and `KIMI_NO_MODIFY_PATH=1`. The User PATH was unchanged afterwards.
- Grok: run in a scratch folder on the person's account. The person signed it in from their own terminal and allowed the check to trust `work\grok` in Grok's folder prompt.
- How it was driven:
  - The core API through a page helper that keeps the token inside the webview: create, restart, close, input, peek, sessions, recordings, hook events, profiles, `agents/own` and `agents/catalog`.
  - CDP evaluations returning small JSON, and CDP keys: Enter in the check's own tabs, Escape, letters in the launcher.
  - No screenshots and no desktop input.
  - Terminal signals were counted from each tab's asciicast recording with a scratch script.

## Results

| Check | Result |
| --- | --- |
| Flags match Task 1's table | Pass. `--help` for opencode, Qwen, Crush and Kimi matches the prompt style, continue and every mapped mode flag. No `manifests.json` edit. |
| The catalog says found | Pass. Found: claude 2.1.310, codex, antigravity, opencode (no modes), grok 1.0.50 (all five), qwen, kimi (manual, plan, full-auto, bypass), crush. Not found: copilot (a VS Code shim, rejected), cursor (Grok's `agent.exe`, rejected), amp, aider. |
| Each starts in Manual with the mapped argv | Pass, as the tab's pwsh ran it: `qwen --approval-mode default`, bare `kimi`, `crush` and `opencode` (opencode maps no modes, so it starts with none), and `grok --permission-mode default`. Each stopped at its own sign-in or folder-trust screen. |
| One other mode | Pass. Restarting in another mode gave `qwen --continue --approval-mode plan`, `kimi --continue --plan` and `crush --continue --yolo`. A mode that isn't mapped is refused before anything starts: create and restart both answered 400 "Crush has no Plan mode." |
| The row says "Status limited" | Pass. "Idle · Status limited", and "Working · Status limited" while output streamed. `live` stayed null, and no row showed hook-shaped status. |
| Resume restarts with the continue flag | Pass. opencode was quit (Ctrl+C) and its shell was ended with `exit`. The row read "Exited with code 0", and Resume ran `opencode --continue`. |
| Terminal signals inside Marshell | Recorded below. |
| Grok asks before a tool (Manual) | Not shown. In Manual, Grok ran `dir` and then wrote `probe.txt` without asking. The argv is right: Grok's `22-permissions-and-safety.md` says `default` lets only read-only tools and commands run without asking. `grok inspect` reports one permission rule loaded from the person's `~/.claude/settings.json`. Grok merges Claude Code's `permissions.allow`, which would explain the write. The rule's text wasn't read (the check stays out of `~/.claude`). See finding 4. |
| No Claude-shaped status or approve card for Grok | Pass. The row read "Grok, Working" and then "Grok, Idle", with `live` null and no card. `/v1/hook/events` held no event for the Grok tab, and none untabbed. |
| Grok's hooks and their sources | `grok inspect` in the untrusted scratch project: one hook, the superpowers plugin's, from `~/.claude/plugins`. Permissions came from `~/.claude/settings.json`. `externalCompat` has Claude hooks, rules, skills, agents and MCPs on by default. The project's `.claude/settings.local.json` (Marshell's scratch hooks) didn't load, because Grok hadn't trusted the folder. The hook filter is covered by its unit tests only. |
| Grok's process chain | `pwsh.exe` → `grok.exe --permission-mode default`. No `agent.exe` child and no other Grok process. |
| An own agent by full path | Pass. "Scratch Kimi" was added in the Agents dialog, with the scratch `kimi.exe` path, Continue `--continue`, Manual with no flags and Plan `--plan`. The announcement was "Scratch Kimi saved." The hint showed "Id: scratch-kimi". The catalog listed it found, own, `[manual, plan]`, `can_continue`. It started as `& '<scratch>\kimi\bin\kimi.exe' --plan`. |
| A hand edit reloads | Pass. Adding `"bypass": ["--auto"]` by hand put Bypass in the catalog with no restart. A second entry with the id "Bad Id" was flagged by itself ("An id uses lowercase letters, digits and dashes, up to 32."), and Scratch Kimi stayed. Saving from the stale dialog showed "agents.json changed since you opened it. Reload to see the change." with Reload. After the reload, the list showed the bad entry with its reason, and Remove dropped it ("Bad removed."). The hand-added Bypass survived the dialog's save. |
| DeepSeek without a key | Pass. The catalog's preset has the problem "Set DEEPSEEK_API_KEY, then restart Marshell to start DeepSeek." The launcher lists DeepSeek. Start shows that text (`role="alert"`) and starts nothing (5 sessions before and after). A direct POST answers 400 with the same text. A start with a key wasn't run: no key was supplied. |
| `~/.claude/settings.json` untouched | Pass. Its modified time is still `2026-10-25T01:18:38.2971498+03:00`. |

### Terminal signals

From each tab's plain recording, after the mode restart.

| CLI | OSC 9;4 | OSC 9 | OSC 99 | OSC 777 | Bell |
| --- | --- | --- | --- | --- | --- |
| Crush | `9;4;3` then `9;4;0` | none | `99;i=crush-osc99-query:p=?` (a query) | none | none |
| Qwen Code | `9;4;0;` once | none | none | none | none |
| opencode | none | none | `99;i=opentui-notifications:p=?` (a query) | none | none |
| Kimi | none | none | none | none | none |

Kimi sent only OSC 8 links. The scanner treats the two `p=?` queries as not text (`osc133.rs:569`), and no row went to Needs you. With no sign-in, no CLI got as far as a notification. Grok's tab wasn't scanned.

## Findings

1. **A tab can't run a CLI the catalog says is found, when it's on a PATH only Marshell's own process has.** portable-pty 0.9 rebuilds a child's PATH from the registry (`get_base_env` in `cmdbuilder.rs`), while the catalog searches the core's own PATH. The first Qwen tab printed "The term 'qwen' is not recognized". It only bites a PATH set in the shell that started Marshell, such as this check's scratch prefix. The other way round, a CLI installed while Marshell runs is found only after a restart. For the check, the scratch profile got a `PATH` env entry. Fixed in 53bc927: commands are looked up on the PATH portable-pty builds for the tab (the profile's own PATH wins), with `tab_path::an_agent_is_looked_up_on_the_tab_path_not_on_marshells_own` failing first.
2. **A manifest agent with the generic mark is unnamed in its row.** Crush (and Aider), and every own agent, read "Custom agent" in the row's mark and accessible name. The row's middle word is the folder, so nothing in the row says which agent runs. `toRow` maps an id outside `AGENT_IDS` to `generic`, and `AgentMark` labels that "Custom agent". Fixed in b4370ec: the row and its mark take the catalog's name, else the program's, with the `toRow` and `AgentMark` tests failing first.
3. **A shape error in `agents.json` hides all of a person's own agents.** A missing `cli` field gave a single problem ("agents.json can't be read: it has an unexpected shape (missing field `cli`).") and an empty list, so Scratch Kimi dropped out of the catalog until the file was fixed. A well-shaped bad entry is flagged by itself. Fixed in 839029f and 2970d44: each entry is read on its own, a broken one is its own problem and the others load, and a save writes a broken entry back exactly as it was unless the person edited or removed it (both `catalog_api` tests failing first).
4. **Grok's Manual is only as strict as the person's Claude Code allow rules.** Grok merges `permissions.allow` from `~/.claude/settings.json` into its own rules, so with `--permission-mode default` a write ran without a prompt. Marshell's flag is right. Grok has no flag that ignores Claude's settings for one start (`externalCompat` is config), so the mode chip's "ask before each change" can be wrong for Grok on a machine with broad Claude allow rules. Not fixed. It's a known limit for the PR.
5. **The palette lists the five Mode commands for every tab.** For an agent that doesn't map a mode, or for opencode, which maps none, picking one does nothing and says nothing (`askRestart` refuses it). The row menu already offers only the mapped modes (90ac726). This existed for plain shells before 6b. Fixed in f6459be: the palette lists only the modes the focused agent offers, as the row menu does.
6. **Cancel in the own-agent form puts focus on the dialog title.** Focus doesn't go back to the Edit or "Add an agent" button that opened the form, and opening the form leaves focus on that button. Fixed in 411b85d: opening the form focuses Name, and Cancel, Save and Remove return focus to an Edit button or "Add an agent". The focus decisions are unit-tested; they weren't rechecked in a real window.
7. **Grok updated itself from 1.0.50 to 1.0.70 during the sign-in.** `--permission-mode` takes the same values in 1.0.70: `default, acceptEdits, auto, dontAsk, bypassPermissions, plan`, and `-c/--continue` is unchanged.

## Left in the person's home

The CLIs wrote these:
- `~\.qwen`
- opencode's `~\.local\share\opencode`, `~\.local\state\opencode`, `~\.config\opencode` and `~\.cache\opencode`. They date from the `--version` and `--help` runs.
- `~\AppData\Local\crush`
- Grok's session folder for `work\grok` under `~\.grok\sessions`, and that folder's line in `~\.grok\trusted_folders.toml`

Each is removed only with the person's OK, by literal path.
