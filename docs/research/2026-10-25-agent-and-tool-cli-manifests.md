# Agent and tool CLIs: evidence for data manifests and tool profiles

- Date: 2026-10-25.
- Method: web docs, vendor repositories and registries, plus `--help` and `--version` of the copies installed on this PC. No CLI was signed in or changed. Nothing was installed.

## Purpose

This note is evidence for two kinds of data file. The first is a manifest for an agent CLI that has no Rust adapter: how to launch it, continue and resume a session, which flags set permissions, whether it has hooks, where it keeps sessions, and what it emits to the terminal. The second is a tool profile for a developer CLI that is not an agent (op, docker, gh, vite and so on): launch presets, text that means the program needs the user, lines that announce a local port, and commands that print secrets. Facts carry a source link. Anything not confirmed in a primary source is marked unverified.

Evidence tags for the tool profiles:

- `[src]` read in the vendor's open-source repository, with the file named.
- `[docs]` a vendor documentation page.
- `[local]` from `--help`, `--version` or installed files on this PC.
- `[unverified]` memory or community posts. A hint, not a pattern to ship.

For the agent CLIs, `[P]` means a primary source (vendor docs, repo, registry, install script) and `[T]` a third-party or forum source.

## Findings that change the design

1. **Grok loads Claude Code hook files by default.** Grok Build merges hooks from `~/.claude/settings.json` and the project's `.claude/settings.json` and `settings.local.json`. The switch is `[compat.claude] hooks = false` in its config, or the env var `GROK_CLAUDE_HOOKS_ENABLED`. Grok sets `GROK_HOOK_EVENT` on every hook process it starts. Verified 2026-10-25 in Grok 1.0.50's installed docs, `~/.grok/docs/user-guide/05-configuration.md` and `10-hooks.md`. The same files are in the repo: https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/10-hooks.md . So the hooks Marshell writes for Claude Code can fire inside a Grok session in the same folder. The hook handler must tell the two apart. The group B report names `GROK_SESSION_ID` and the `[compat.<vendor>] hooks = false` switch; the env var names `GROK_CLAUDE_HOOKS_ENABLED` and `GROK_HOOK_EVENT` come from the 2026-10-25 check of the installed docs.
2. **The `grok` exe name is not unique.** xAI's Grok Build installs `%USERPROFILE%\.grok\bin\grok.exe`. The community package `grok-dev` (https://github.com/superagent-ai/grok-cli, MIT, npm `grok-dev` 1.1.7 on 2026-05-15, "not affiliated with xAI") also installs a binary called `grok` (`bin: {grok: dist/index.js}`). A manifest must not assume `grok` on PATH is xAI's. Detect by `grok --version` printing `grok 1.0.x (hash) [stable]`, or by the path `%USERPROFILE%\.grok\bin\grok.exe`.
3. **Grok's install also ships `agent.exe`**, an identical copy of `grok.exe` for agent mode. Cursor's CLI executable is also `agent` (legacy name `cursor-agent`). Matching on the exe name `agent` is ambiguous. Match on the full path or the install folder: `%USERPROFILE%\.grok\bin\` against `%LOCALAPPDATA%\cursor-agent\`.
4. **Kimi has two CLIs.** `MoonshotAI/kimi-cli` (Python, `~/.kimi`) is archived ("no longer maintained", last tag 1.52.0 on 2026-09-22). The live product is Kimi Code CLI, https://github.com/MoonshotAI/kimi-code (TypeScript, MIT, binary `kimi`, data in `~/.kimi-code`, 2.3.1 on 2026-10-23). Flags and the session store differ. `kimi migrate` copies config, MCP and sessions from `~/.kimi/` to `~/.kimi-code/`; OAuth logins are not migrated. Anything written for the old `kimi` is stale.
5. **DeepSeek has no first-party terminal CLI.** DeepSeek Harness (`dsh`, https://github.com/deepseek-ai/deepseek-harness, MIT, developer preview) is web, headless, ACP and SDK. Its README's `--profile tui` is an example that needs third-party plugins. The documented route to a terminal agent on DeepSeek is Claude Code pointed at DeepSeek's Anthropic-compatible API: https://api-docs.deepseek.com/quick_start/agent_integrations/claude_code/ . The report's recommendation is to make no "DeepSeek" manifest. Offer it as a provider preset (env vars) on Claude Code, Crush, Qwen Code or opencode, and track `dsh` as a future ACP or headless target.
6. **Window-title and notification signals are patchy.** No CLI in either group documents the OSC 0 or 2 title as a status signal, except as listed here.
   - Documented or read in source: Grok (OSC 0, source), Qwen Code (OSC 2 and OSC 0, source), Kimi Code CLI (OSC 0, source), Copilot (`updateTerminalTitle` setting; sequence not stated), Cursor (`display.showStatusIndicators`; sequence not stated), opencode (`OPENCODE_DISABLE_TERMINAL_TITLE`; sequence not stated), Crush (Bubble Tea window title; OSC 0 or 2 unverified).
   - Not documented: Amp. Aider: none found.
   - Bell and OSC notifications differ by CLI and often depend on terminal focus (Grok, Crush, opencode). Hooks are the reliable "needs you" signal where they exist. Details are in the per-CLI notes.
7. **Copilot licence.** Proprietary "GitHub Copilot CLI License". It permits install and run, and bars modification. Redistribution is allowed only unmodified and only as part of an application that provides material functionality beyond the Software. So Marshell must not bundle `copilot`. The user installs it. Running the user's own copy in a tab is not restricted by anything found. Source: https://raw.githubusercontent.com/github/copilot-cli/main/LICENSE.md .
8. **A bare `copilot` on PATH on this PC is VS Code's `copilot-shim`**, which prints "not found". The manifest must not trust bare `copilot`.
9. **Hook files cross over between CLIs.** Copilot, Cursor, Crush and Grok read `CLAUDE.md` and/or Claude hook files; Qwen Code writes `CLAUDE_PROJECT_DIR` into its hook env. Marshell-generated instruction and hook files will be read by more than Claude Code. See each CLI's notes.
10. **Secrets also leak through argv.** Saved launch commands, command history and tab titles must never hold a token typed on a command line. See "Secret-printing commands".

## Agent CLIs

Versions are those seen on 2026-10-25. "Seen" dates are the release or publish date where the report gives one; "inferred" means taken from the version string.

| CLI | Exe (Windows) | Version, date seen | Launch with first prompt | Continue last | Resume by id | Permission / mode flags | Hooks, can deny | Session store | Title / notify signals | Licence |
|---|---|---|---|---|---|---|---|---|---|---|
| GitHub Copilot CLI | `copilot` (npm shims; winget `copilot.exe`, name unverified) | 1.0.103, 2026-10-22 | `copilot -i "prompt"` (`-p` is non-interactive) | `--continue` (falls back to the global latest session) | `--resume[=id\|prefix\|name]` or `-r`; `--session-id UUID` pre-assigns | `--allow-all-tools`, `--allow-all` / `--yolo`, `--allow-tool`, `--deny-tool`, `--mode interactive\|plan\|autopilot`, `--plan`, `--autopilot` | Yes, 14 events; `preToolUse` and `permissionRequest` can deny | `%USERPROFILE%\.copilot\session-state\<session-id>\` | `updateTerminalTitle`, OSC 9;4 progress, `beep`, `terminalNotifications` (OSC 777, default off) | Proprietary |
| Cursor CLI | `agent.exe` (legacy `cursor-agent.exe`) | 2026.10.22-27dcf5f (date inferred) | `agent "prompt"` (`-p` is non-interactive) | `agent --continue` or `agent resume` | `agent --resume <chatId>` | `-f`/`--force` (= `--yolo`), `--mode plan\|ask`, `--plan`, `--sandbox enabled\|disabled`, `--approve-mcps` | Partial; CLI coverage unverified; some events can deny | `~/.cursor/chats/<workspace-hash>/<chat-uuid>/` (forum; Windows unverified) | `notifications` setting, `display.showStatusIndicators`; sequences unverified | Proprietary |
| opencode | `opencode.exe` | V1 1.19.4 (2026-10-22); V2 2.1.4 (2026-10-23) | `opencode --prompt "text"` | `-c` / `--continue` | `-s <id>` / `--session <id>` | config `permission` allow/ask/deny; `--auto`; env `OPENCODE_PERMISSION` | No shell hooks; JS/TS plugins; `tool.execute.before` can block | `%USERPROFILE%\.local\share\opencode\opencode.db` (SQLite) | Title yes (OSC unverified); `tui.json` `attention` (default off) | MIT |
| Amp | `amp.exe` (npm bin; install.sh uses `~/.amp/bin`) | `0.0.1792872041-gb83e0b`, 2026-10-24 (inferred) | Not documented for interactive; `-x "prompt"` is non-interactive | `amp threads continue` with no id (inferred) | `amp threads continue <T-id>` | None by default (no prompts); `amp.tools.disable`, `amp.mcpPermissions`; `--dangerously-allow-all` unverified | Plugin API (TypeScript); `tool.call` can deny; no shell hooks | Server-synced threads; local path unverified | `amp.notifications.enabled` (sound; BEL only over SSH or `AMP_FORCE_BEL`); title undocumented | Proprietary |
| Grok Build | `grok.exe` (also `agent.exe`) | 1.0.50, 2026-10-06 | `grok "prompt"` | `-c` / `--continue` | `-r <id-or-title>`; `-s <uuid>` names a NEW session | `--permission-mode default\|acceptEdits\|auto\|dontAsk\|bypassPermissions\|plan`; `--always-approve` / `--yolo` | Yes, 15 events; `PreToolUse` can deny; fail-open | `%USERPROFILE%\.grok\sessions\<url-encoded-cwd>\<uuid>\` | OSC 0 title; notifications by terminal (OSC 9/99/777 or BEL), default only when unfocused; OSC 9;4 progress | Apache-2.0 |
| Qwen Code | `qwen.cmd` shim | 0.26.1, 2026-10-24 | `qwen -i "prompt"` / `--prompt-interactive` | `--continue` | `--resume [sessionId]` | `--approval-mode plan\|default\|auto-edit\|auto\|yolo`, `-y` / `--yolo` | Yes, 22 events; `permissionDecision` can deny | `%USERPROFILE%\.qwen\projects\<sanitized-cwd>\chats\<sessionId>.jsonl` | OSC 2 and OSC 0 title; bell (OSC 9 on iTerm, 99 on kitty, 777 on ghostty, else BEL) | Apache-2.0 |
| Kimi Code CLI | `kimi` (native binary; exact file name unverified) | 2.3.1, 2026-10-23 | None; positional args are rejected | `-c` / `--continue` | `-S` / `--session <id>` (hidden alias `-r` / `--resume`) | default Always Ask; `-y`/`--yolo` Ask When Needed; `--auto` Never Ask; `--plan` | Yes, 20 events; deny only on `PreToolUse`, `Stop`, `UserPromptSubmit`; global config only | `%USERPROFILE%\.kimi-code\sessions\<workDirKey>\<sessionId>\` | OSC 0 title; OSC 9;4 progress; notifications channel unverified | MIT |
| Aider | `aider.exe` | 0.86.2, 2026-02-12 | `-m "text"` sends one message and exits; no "then stay interactive" | `--restore-chat-history` only | None | `--yes-always`; `--chat-mode ask\|code\|architect`; no ask or plan modes | None | `<git-root>\.aider.chat.history.md`; no ids | No title; `--notifications` (Windows default is a PowerShell MessageBox) | Apache-2.0 |
| Crush | `crush.exe` | v0.98.2, 2026-10-23 | None for interactive; `crush run "prompt"` is non-interactive | `-C` / `--continue` | `-s` / `--session <id>` | ask by default; `-y` / `--yolo`; `permissions allow\|deny` in `crushrc` | `PreToolUse` only; can deny | `<project>\.crush\crush.db` (SQLite) | Title yes (OSC 0 or 2 unverified); `option notifications auto\|native\|osc\|bell\|disabled`, only when unfocused | FSL-1.1-MIT |
| DeepSeek | No first-party TUI. `dsh` (Harness) or Claude Code on DeepSeek's API | `dsh` 0.2.1-rc.3 (npm `next`, 2026-10-23) | n/a | n/a | n/a | n/a | n/a (Harness has ACP and SDK) | `$DSH_HOME`, layout unverified | n/a | MIT (Harness) |

Hook-capable and deny-capable here means a host can answer or observe a tool call. See the caveats below for what each CLI actually fires.

## Per-CLI caveats and unverified items

### GitHub Copilot CLI

- Command `copilot`, npm `@github/copilot`. Repo https://github.com/github/copilot-cli . Latest 1.0.103 published 2026-10-22T16:21:56Z: https://api.github.com/repos/github/copilot-cli/releases/latest . Prereleases v1.0.104-1 to -3 exist (2026-10-23 and 24). Changelog: https://raw.githubusercontent.com/github/copilot-cli/main/changelog.md .
- Install: `winget install GitHub.Copilot`, or `npm install -g @github/copilot`. The shell install script is macOS and Linux only. Needs PowerShell 7+ on Windows. Source: https://raw.githubusercontent.com/github/docs/main/content/copilot/how-tos/copilot-cli/set-up-copilot-cli/install-copilot-cli.md . Native on Windows (changelog 1.0.86 and 1.0.100). The exact winget file name is unverified.
- Flags source: https://raw.githubusercontent.com/github/docs/main/content/copilot/reference/copilot-cli-reference/cli-command-reference.md . `--continue` is rejected together with `--resume` (changelog 1.0.71). The resume hint printed on exit (changelog 1.0.72) has unverified text. Sandbox flag names (release 1.0.103) are unverified.
- Hooks: https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-hooks-reference and https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-hooks . Locations: `.github/hooks/*.json`, `%USERPROFILE%\.copilot\hooks\` (or `$COPILOT_HOME/hooks/`), policy `C:\ProgramData\GitHub\Copilot\policy.d\`. Windows hooks use the `powershell` field. Default timeout 30 s. `preToolUse` command errors fail closed and timeouts fail open; `ask` is denied in autopilot. The `notification` event has types `permission_prompt`, `agent_idle` and `elicitation_dialog`, which give needs-you signals. Repo hooks "require workspace admission"; the trust prompt flow is unverified. PascalCase event names switch the payload to snake_case, VS Code compatible.
- Sessions: https://raw.githubusercontent.com/github/docs/main/content/copilot/reference/copilot-cli-reference/cli-config-dir-reference.md . Files: `events.jsonl`, `workspace.yaml`, `plan.md`, `checkpoints/`, `files/`. The `events.jsonl` format is undocumented; a third-party crate checked 1.0.67 only: https://docs.rs/toolpath-copilot [T]. History also syncs to the account (`remoteExport`).
- Signals: `terminalProgress` (OSC 9;4), `beep` (documented default true), `terminalNotifications` (default false). A pre-release note says the bell became opt-in on turn completion (https://newreleases.io/project/github/github/copilot-cli/release/v1.0.55-1 [T]). That conflicts with the documented `beep` default of true. Unverified which holds.
- Reads `CLAUDE.md`, `GEMINI.md`, `AGENTS.md`, `.github/copilot-instructions.md` and `.claude/rules/**`; `--no-custom-instructions` turns it off.
- Auth: `copilot login`; token env order `COPILOT_GITHUB_TOKEN`, `GH_TOKEN`, `GITHUB_TOKEN`. No non-interactive status command found.
- ACP: `copilot --acp` (public preview): https://docs.github.com/en/copilot/reference/copilot-cli-reference/acp-server .
- Terms: the wider Copilot terms (Acceptable Use, Additional Product Terms) were not read. A Copilot subscription is needed and org policy can block use.

### Cursor CLI

- Docs https://cursor.com/docs/cli/overview ; install script https://cursor.com/install . Windows: `irm 'https://cursor.com/install?win32=true' | iex`, installs to `%LOCALAPPDATA%\cursor-agent`, adds `agent.exe` and `cursor-agent.*` copies, warns that Git for Windows is required. The release date 2026-10-22 is inferred from the version string.
- Flags: https://cursor.com/docs/cli/reference/parameters . `--continue` is an alias of `--resume=-1`. The chat id comes from `agent ls` and from hooks (`conversation_id`). Printing it on exit is unverified.
- Permission config: https://cursor.com/docs/cli/reference/configuration (`permissions.allow` / `deny` like `Shell(ls)`).
- Hooks: https://cursor.com/docs/hooks . The official page does not say which events fire in the CLI. Forum posts disagree and age quickly: https://forum.cursor.com/t/cursor-cli-hooks/148511 and https://forum.cursor.com/t/cursor-cli-doesnt-send-all-events-defined-in-hooks/148316 [T]. Treat CLI hook coverage as unverified and version-dependent. Test on the installed version.
- Chats path comes from forum replies: https://forum.cursor.com/t/where-are-cursor-chats-stored/77295 [T]. The Windows path and the id-as-directory-name rule are unverified. The IDE's `state.vscdb` is a different store.
- Signals: `notifications` ("when the agent finishes or needs input") and `display.showStatusIndicators` ("terminal title status indicators"). Escape sequences and defaults are unverified.
- ACP: `agent acp` (https://cursor.com/docs/cli/acp, from a search summary; page not fetched).
- Status: `agent status` and `whoami` with `--format text|json` (the authentication page does not mention `whoami`; the parameters page does).
- Terms: no wrapping clause found. A forum post cites an Acceptable Use Policy updated 2026-08-11 about "automated or non-human means" (https://forum.cursor.com/t/is-this-use-of-cursor-cloud-agents-permitted-under-cursor-s-terms/168633 [T]). The AUP text was not read: unverified.

### opencode

- Repo https://github.com/anomalyco/opencode (moved from sst/opencode), MIT, docs https://opencode.ai/docs/ . **Two lines exist.** V1 `opencode-ai` latest 1.19.4: https://api.github.com/repos/anomalyco/opencode/releases/latest . V2 latest 2.1.4 per https://opencode.ai/changelog (2.0 around 2026-09-11 per a third party, https://capitalandcompute.net/blog/opencode-2-0-what-changed/ [T]). Both use the command `opencode`. Which is "current" is a judgment call; the docs say `brew install opencode` now installs V2.
- Windows install disagrees between pages. The V2 docs (https://opencode.ai/v2/docs) say "Windows package managers are not supported". The V2 changelog (v2.0.28) says "official WinGet packages". Unverified which is current. V1 recommends WSL but runs natively: https://opencode.ai/docs/windows-wsl/ . Needs Git for Windows (`OPENCODE_GIT_BASH_PATH`).
- Flags (V1): https://opencode.ai/docs/cli/ . V2's CLI page documents only `--standalone` and `--server`: https://opencode.ai/v2/docs/cli . So `-c`, `-s` and `--model` for V2 are unverified. `--auto` is in https://opencode.ai/docs/permissions/ but not on the CLI or TUI pages, so unverified in `--help`. Default permissions allow most tools without asking.
- Data dir `%USERPROFILE%\.local\share\opencode` (not AppData): https://opencode.ai/docs/troubleshooting/ . `opencode db path` prints the db path. Whether the id is printed on exit is unverified.
- Plugins: https://opencode.ai/docs/plugins/ . `tool.execute.before` can block. A `permission.ask` hook is not documented; only the events `permission.asked` and `permission.replied`, so answering a pending prompt from a plugin is unverified. The global plugin path on Windows is unverified. V2 changed the plugin contracts (v2.1.0).
- Signals: `OPENCODE_DISABLE_TERMINAL_TITLE`; `tui.json` `attention.enabled` (default off): https://opencode.ai/docs/tui/ and https://opencode.ai/docs/config/ . Whether it uses OSC 9, 777 or BEL is unverified.
- Instructions: `AGENTS.md`, falling back to `CLAUDE.md` only when `AGENTS.md` is absent: https://opencode.ai/docs/rules/ .
- Terms: MIT. The OpenCode Zen and Go hosted terms were not read.

### Amp

- Docs https://ampcode.com/docs (index https://ampcode.com/llms.txt ). npm `@sourcegraph/amp`: https://registry.npmjs.org/@sourcegraph/amp/latest . Proprietary; terms https://ampcode.com/terms . "Research preview". The version date is computed from the version string, not read from the registry.
- **Native Windows is not officially supported.** The docs say macOS, Linux and "Windows through WSL": https://ampcode.com/docs/cli . `install.sh` maps Git Bash to `amp-windows-x64.exe` and the npm bin is an `amp.exe`, so a native binary exists. Treat native support as unofficial.
- Interactive launch with a first prompt is undocumented; only piped stdin is. `amp threads continue` with no id picking the latest thread is inferred from SDK `continue: true`: https://ampcode.com/docs/markdown/threads and https://ampcode.com/docs/markdown/sdk .
- `--mode` is reported by third parties (https://e2b.dev/docs/agents/amp [T]) and not confirmed officially. `--dangerously-allow-all` is reported by e2b only. Amp does not ask for approval by default: https://ampcode.com/docs/legacy-permissions-rules.txt . No plan or read-only flag found.
- Local session storage is not documented. An unofficial claim names `~/.local/share/amp/threads/` (https://traces.com/s/jn7c679tvtyemcetnqeq9tx34d81k96z [T], weak). Stable interfaces are `amp threads export` and `session_id` in `--stream-json`: https://ampcode.com/docs/markdown/cli/streaming-json (execute mode only).
- Plugin API: https://ampcode.com/docs/markdown/plugin-api . `tool.call` can allow, reject, modify, synthesize or error. A third-party mirror says plugins need the binary install and `PLUGINS=all` (https://milad-local.context.ai/manual/plugin-api [T, mirror, unreliable]); the official docs mention neither. Unverified.
- Signals: https://ampcode.com/docs/markdown/cli/settings . Sound by default; BEL only over SSH or with `AMP_FORCE_BEL`. OSC title: undocumented.
- Instructions: `AGENTS.md`, falling back to `AGENT.md` or `CLAUDE.md`: https://ampcode.com/docs/markdown/customize/agents-md .
- Terms: the Acceptable Use Policy was not read. The ToS bar reselling quotas or credits.

### Grok Build

- Docs https://docs.x.ai/build/overview ; announcement https://x.ai/news/grok-build-cli ; source https://github.com/xai-org/grok-build (Apache-2.0, no GitHub releases). Installed here: 1.0.50 (c58f321264ba) [stable], dated 2026-10-06 in the shipped `~/.grok/CHANGELOG.md`. Whether a newer stable exists is unverified (`grok update --check` not run; https://x.ai/build/changelog returned HTTP 403).
- Install: `irm https://x.ai/cli/install.ps1 | iex` or `winget` id `xAI.GrokBuild`: https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/01-getting-started.md . Installs to `%USERPROFILE%\.grok\bin`: `grok.exe`, `agent.exe`, and the Grove helpers used only by `grok clone`.
- `-s` / `--session-id <UUID>` lets the host choose the id of a new session. With `--resume` or `--continue` it works only with `--fork-session`. This lets Marshell pre-assign ids.
- On this PC `~/.grok/config.toml` already sets `permission_mode = "always-approve"`, so a launch with no flags runs in always-approve here. Permissions doc: https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/22-permissions-and-safety.md .
- Sessions: https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/17-sessions.md , confirmed by listing the real tree. The id is the directory name. `GROK_HOME` relocates it.
- Hooks: all failures and timeouts fail open; `allow` does not auto-approve a call that would otherwise prompt. Project hooks under `.grok/hooks/` need folder trust. Global `~/.grok/hooks/*.json` are always trusted. Hook input is camelCase on stdin. There are separate `[[ui.notifications.hooks]]` in `config.toml`.
- Signals: title is OSC 0 (source https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/src/notifications/title.rs ). Notification matrix: https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/05-configuration.md . Default `condition = "unfocused"`. Windows Terminal is detected (https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/21-terminal-support.md) but is absent from the notification matrix. An xterm.js host is an unknown terminal (BEL, no focus tracking). Whether notifications fire without focus tracking under `unfocused` is unverified. Set `condition = "always"` and `method = "bel"`, or hook on `Notification`.
- Instructions: reads `AGENTS.md`, `CLAUDE.md`, `CLAUDE.local.md` and, with compat on, `.claude/rules/` and `.cursor/rules/`: https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/12-project-rules.md .
- Auth: `grok login`, credentials in `~/.grok/auth.json` (never read). No `status` or `whoami`; closest are `grok doctor --json` and `grok inspect --json`.
- Terms: no wrapping prohibition found. xAI publishes a Claude Code plugin that drives Grok Build (https://github.com/xai-org/grok-build-plugin-cc, Apache-2.0). Consumer terms for third-party hosts are unverified (https://x.ai/legal/terms-of-service). A third-party claim that the CLI uploads the whole repo and git history (https://github.com/cereblab/grok-build-exfil-repro) is unverified; check before shipping Marshell defaults.

### Qwen Code

- Repo https://github.com/QwenLM/qwen-code (Apache-2.0, a Gemini CLI fork); docs https://qwenlm.github.io/qwen-code-docs/en/users/overview . v0.26.1, 2026-10-24. npm `@qwen-code/qwen-code` (Node 22+).
- Windows: the standalone script installs `%LOCALAPPDATA%\qwen-code\bin\qwen.cmd` (script read, not run). The real exe behind the shim is unverified.
- `--continue` and `--resume` are documented in the headless table but defined as global options in https://github.com/QwenLM/qwen-code/blob/main/packages/cli/src/config/config.ts , so they apply to interactive launches. `--session-id` and `--fork-session` exist there; semantics not read, unverified. Default approval mode is `auto` (LLM classifier): `docs/users/features/approval-mode.md`.
- Session path `projects\<sanitized-cwd>\chats` is from the docs; an older code comment says `tmp\<project_id>\chats`. Confirm on a real install. `/status paths` prints the real paths.
- Hooks: `docs/users/features/hooks.md` in the repo. `settings.json` `hooks` key; command hooks can set `shell: powershell`. `PermissionRequest` fires when the dialog shows. Server modes: `--acp`, `qwen serve` ("Stage 1 experimental").
- Signals: source `packages/cli/src/ui/utils/windowTitle.ts` (OSC 2 and OSC 0) and `notificationService.ts` (OSC 9 for iTerm, 99 for kitty, 777 for ghostty, else BEL). Disable with `ui.hideWindowTitle` or `general.terminalBell=false`.
- Default context files are `QWEN.md` and `AGENTS.md` (`memory-constants.ts`); the docs name only `QWEN.md`.
- Auth: the standalone `qwen auth` was removed; use `/auth`. Qwen OAuth free tier discontinued 2026-04-15 (`docs/users/configuration/auth.md`).
- Terms: `docs/users/support/tos-privacy.md`. Whether Alibaba's Coding Plan terms limit use to approved tools is unverified.

### Kimi Code CLI

- Repo https://github.com/MoonshotAI/kimi-code (MIT); docs https://moonshotai.github.io/kimi-code/en/ . 2.3.1, 2026-10-23.
- Windows: `irm https://code.kimi.com/kimi-code/install.ps1 | iex`. **Needs Git for Windows** (Git Bash; `KIMI_SHELL_PATH` overrides). `KIMI_INSTALL_DIR` defaults to `%USERPROFILE%\.kimi-code`. The exact exe name and `bin` subfolder are unverified.
- **No positional initial prompt.** Extra positional arguments are rejected. A host would have to type the prompt into the PTY. Source `docs/en/reference/kimi-command.md`, `apps/kimi-code/src/cli/commands.ts`.
- `--yolo` and `--auto` are mutually exclusive; `--continue` and `--session` are mutually exclusive.
- Hooks: `docs/en/customization/hooks.md`. `[[hooks]]` in `~/.kimi-code/config.toml` only; extra fields make the config fail to load. A project-level hook location is not documented, unverified. `PermissionRequest` and `PermissionResult` let a host observe approvals but not answer them. `SessionHeartbeat` fires every 60 s only if configured.
- Sessions: `docs/en/configuration/data-locations.md` and `docs/en/guides/sessions.md`; index `~/.kimi-code/session_index.jsonl`; `KIMI_CODE_HOME` relocates. The id is readable.
- Signals: title is OSC 0 (`packages/pi-tui/src/terminal.ts`); OSC 9;4 progress. `tui.toml` `[notifications]` are called "desktop notifications"; the channel (OSC 9 or 777, bell, native toast) is unverified. No such string was found in the pi-tui terminal code. Use `Notification` and `Stop` hooks instead.
- Terms: the plan's terms restrict use of its key to Kimi CLI and permitted agents, and call tampering with the User-Agent a violation: https://www.kimi.com/code/docs/en/benefits.html ; third-party agents list: https://www.kimi.ai/help/kimi-code/third-party-agents . Running the real `kimi` in Marshell is the supported path. Do not rewrite its User-Agent or reuse its key elsewhere.

### Aider

- Repo https://github.com/Aider-AI/aider (Apache-2.0); PyPI https://pypi.org/project/aider-chat/ . Latest on PyPI 0.86.2 (2026-02-12). Last GitHub release v0.86.0 (2025-08-09). Last commit 2026-05-22. Maintenance only.
- Install: https://aider.chat/docs/install.html .
- `-m` is `--message`, not the model; the model flag is `--model`. `--message` sends one message and exits. `--load <file>` runs `/commands` at launch; whether plain text lines go to the model is unverified. Options: https://github.com/Aider-AI/aider/blob/main/aider/website/docs/config/options.md . Modes: https://github.com/Aider-AI/aider/blob/main/aider/website/docs/usage/modes.md .
- No ids, no hooks, no ACP, no server. A host can only watch the PTY and the history file.
- Notifications: `--notifications` is opt-in. In `aider/io.py` the Windows default is a PowerShell `MessageBox.Show('Aider is waiting for your input','Aider')`, a modal dialog that blocks until dismissed. BEL only when `notifications_command` is falsy. The docs call it "terminal bell notifications", which does not match the code on Windows. No OSC title found; absence not exhaustively verified.
- No automatic `AGENTS.md` or `CONVENTIONS.md` loading was found. Pass instruction files with `--read` or a `read:` list in `.aider.conf.yml`.

### Crush

- Repo https://github.com/charmbracelet/crush , Go. Licence FSL-1.1-MIT: https://github.com/charmbracelet/crush/blob/main/LICENSE.md . Launching it as a user tool is not redistribution. Bundling it would need the licence notice. Whether a multi-agent session manager counts as "competing use" is a legal question, unverified. v0.98.2, 2026-10-23.
- Install: `winget install charmbracelet.crush`, scoop, or `npm install -g @charmland/crush`. Embedded Bash interpreter, so no Git Bash needed.
- The model flag exists only on `crush run`. No interactive initial prompt (root command takes no positional prompt; whether cobra rejects stray args is unverified).
- Config is now a Bash-syntax `crushrc`: https://github.com/charmbracelet/crush/blob/main/docs/config/README.md .
- Session ids come from `crush session list --json` or the DB, not file names. Data dir `.crush` in the project; override with `--data-dir`.
- Hooks: only `PreToolUse`: https://github.com/charmbracelet/crush/blob/main/docs/hooks/README.md . No Stop, Notification or SessionStart, so a host cannot get turn-finished from hooks.
- Signals: window title set through Bubble Tea (`internal/ui/model/ui.go`); OSC 0 versus 2 unverified. Notifications fire only when the terminal is unfocused and reports focus (`internal/ui/notification/bell.go` writes `\x07`). For an xterm.js host, `option notifications bell` plus focus reporting (DECSET 1004) is the likely combination; unverified. `crush server` exists; ACP not found.
- Terms: metrics are on by default (`CRUSH_DISABLE_METRICS=1` or `DO_NOT_TRACK=1`). Hyper's subscription terms are unverified.

### DeepSeek

- DeepSeek Harness: https://github.com/deepseek-ai/deepseek-harness (default branch `master`); docs https://deepseek-harness.github.io/deepseek-harness/ . npm `@deepseek-ai/dsh`: `latest` 0.2.1-rc.2 (2026-10-17), `next` 0.2.1-rc.3 (2026-10-23). The README warns of compatibility-breaking changes; `SAFETY.md` says it is not security-audited. Entry modes (`apps/cli/README.md`): `dsh web`, `dsh --profile headless`, `--profile acp`, `--profile sdk`.
- The base bundle includes "default-on DeepSeek session-log upload" (`apps/cli/reference/README.md`). Check before any use.
- Session store, resume flags, instruction file names and tool approve or deny hooks for Harness are unverified. The relation of third-party plugin orgs (for example `deepseek-harness/turtle-ui`) to DeepSeek is unverified.
- Claude Code route env vars (names only): `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_MODEL`, `ANTHROPIC_DEFAULT_OPUS_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL`, `ANTHROPIC_DEFAULT_HAIKU_MODEL`, `CLAUDE_CODE_SUBAGENT_MODEL`, `CLAUDE_CODE_EFFORT_LEVEL`, `CLAUDE_CODE_AUTO_COMPACT_WINDOW`. Base URL and model mapping: https://api-docs.deepseek.com/quick_start/agent_integrations/claude_code/ . The auth variable holds a DeepSeek key and must never be saved in a manifest. Anthropic's position on third-party back ends is unverified from Anthropic.
- Other options listed in https://github.com/deepseek-ai/awesome-deepseek-agent : Deep Code (https://github.com/lessweb/deepcode-cli, community, MIT), DeepSeek-TUI (https://github.com/Hmbown/DeepSeek-TUI, community, MIT), Reasonix (https://github.com/esengine/reasonix, community). None is made by DeepSeek.

### Cross-CLI points

- Resume syntax differs in kind. Copilot, Cursor, opencode, Grok, Qwen, Kimi and Crush use flags. Amp uses the subcommand `amp threads continue`. Cursor also has `agent resume`. Aider has none.
- Only some CLIs let a host answer a tool call: Copilot, Cursor (partly), Grok, Qwen, Kimi (deny only), Crush (`PreToolUse`), and Amp and opencode through plugins.
- All of Copilot, Cursor, opencode, Amp, Grok, Crush read `AGENTS.md`; most also read `CLAUDE.md` (opencode and Amp only as a fallback). Aider reads neither automatically. Qwen reads `AGENTS.md` and `QWEN.md`; Kimi reads `AGENTS.md`.

## Tool profiles

### Rules that apply to every profile

1. Strip ANSI (SGR) and OSC 8 hyperlinks before matching. Vite wraps the port digits in bold: `http://localhost:` then `ESC[1m` `5173` `ESC[22m` then `/` ([src] `packages/vite/src/node/logger.ts`). Vercel and Firebase can emit OSC 8. Supabase colours its values.
2. Never trust a default port. Always parse the announced URL. Vite prints `Port 5173 is in use, trying another one...` ([src] `http.ts`). Next.js prints `Port 3000 is in use by process <pid>, using available port 3001 instead.` ([src] `start-server.ts`). gcloud emulators pick a random port in 8000-8999 unless `--host-port` is set ([local] `command_lib/emulators/util.py:212`).
3. Ports and secrets share output. `supabase start` prints a status table with keys and a DB URL with a password. `stripe listen`'s ready line carries the webhook signing secret. A port matcher keeps only `scheme://host:port` and never persists or logs the matched line or its neighbours.
4. Auth callback ports are transient and get no chip: gcloud 8085-8185 ([local] `core/credentials/flow.py`), wrangler `localhost:8976` ([docs]), AWS CLI loopback `127.0.0.1:<random>/oauth/callback` ([src] `awscli/customizations/sso/utils.py`), `az login`, Terraform login.
5. Needs-you comes in three kinds: `text` (a prompt on the TTY), `browser` (a URL or code printed, the user acts outside the terminal), `os-dialog` (nothing useful printed).
6. Device-code sentences can be server-supplied or localized. The `az` sentence is MSAL's `message` field, so match loosely on `microsoft.com/devicelogin` or `aka.ms/devicelogin`.
7. npm-installed tools are `.cmd` shims on Windows (`firebase.cmd`, `vercel.cmd`, `netlify.cmd`, `wrangler.cmd`, `npm.cmd`, `npx.cmd`, `pnpm.cmd`); `az.cmd` and `gcloud.cmd` wrap Python. The real child is `node.exe` or `python.exe`, so process-name matching on the child will not identify the tool. People also launch through `npx`, `pnpm dlx`, `pnpm exec`, `npm exec` or `bunx`. Key the profile on the tool token after the launcher.
8. `npx` itself prompts: `Need to install the following packages:` then `<pkg>` then `Ok to proceed? ` ([src] `workspaces/libnpmexec/lib/index.js:304`; npm 11.19 then shows its default, `Ok to proceed? (y) `, [local] seen with `npx vite@8.3.5` in the slice 6c live check). A generic needs-you pattern for any `npx <tool>` launch.
9. Launch presets are inert templates with no secret in the argument list. A target such as `kubectl port-forward <target>` is a fill-in field, never a saved literal.

### Tool executables and installed versions

| Tool | Windows exe | Seen here |
|---|---|---|
| op | `op.exe` | 2.41.0 [local] |
| docker | `docker.exe` (compose is a plugin) | 29.9.2 [local] |
| gh | `gh.exe` | 2.101.0 [local] |
| kubectl | `kubectl.exe` | client v1.36.1 [local] |
| gcloud | `gcloud.cmd` (child `python.exe`) | SDK 588.0.0 [local] |
| supabase | `supabase.exe` or `supabase.cmd` | v2.122.0 (2026-10-23) per GitHub releases; not installed here |
| vercel, wrangler, firebase, netlify | `.cmd` shims (child `node.exe`) | not installed here |
| aws, az, terraform, ngrok, stripe | `aws.exe`, `az.cmd`, `terraform.exe`, `ngrok.exe`, `stripe.exe` | not installed here |
| fly | `fly.exe` and `flyctl.exe` in `%USERPROFILE%\.fly\bin\` ([src] https://fly.io/install.ps1) | not installed here |

### Port announcement lines (parse the output)

| Tool | Command | Exact line | Regex-relevant shape | Evidence |
|---|---|---|---|---|
| Vite | `vite`, `vite preview` | `➜  Local:   http://localhost:5173/` | `➜\s+Local:\s+(https?://\S+)` after stripping ANSI. Preview default 4173. | [src] `cli.ts`, `logger.ts`, `constants.ts`; [local] vite 8.3.5 in the slice 6c live check |
| Next.js | `next dev`, `next start` | `- Local:        http://localhost:3000`, then `✓ Ready in 1.2s` | `Local:\s+(https?://\S+)`. Padding varies by version. `Ready in` is the stable "up" signal. | [src] `app-info-log.ts`, `start-server.ts` |
| Vercel | `vercel dev` | `> Ready! Available at http://localhost:3000` | `Ready! Available at (https?://\S+)`. Strip ANSI and OSC 8. | [src] `util/dev/server.ts:1570`, `create-output.ts:157` |
| Wrangler | `wrangler dev` | `[wrangler:info] Ready on http://localhost:8787` | `Ready on (https?://\S+)`. The log-level tag varies (`[wrangler:inf]` in older versions). Older versions print `127.0.0.1`. | [src] `e2e/helpers/normalize.ts:192` |
| Netlify | `netlify dev` | box: `Local dev server ready: http://localhost:8888` | `Local dev server ready:\s+(https?://\S+)` after stripping box characters. Older: `◈ Server now ready on http://localhost:8888` [unverified]. | [src] `src/utils/dev-server-banner.ts` |
| Firebase | `emulators:start` | table rows `Authentication \| 127.0.0.1:9099 \| http://127.0.0.1:4000/auth`, and `i  View Emulator UI at http://127.0.0.1:4000/` | After stripping box characters: `^\s*│?\s*(\w[\w ]+?)\s*│\s*([\d.]+\|\[::1\]\|localhost):(\d+)`. Only the UI row has a scheme. | [src] `src/commands/emulators-start.ts:70-140` |
| Supabase | `start`, `status` | rounded tables with `Studio`, `Mailpit`, `Project URL`, `REST`, `GraphQL`, `Edge Functions` | `http://(127\.0\.0\.1\|localhost\|0\.0\.0\.0\|\[::1\]):\d+` only in the `Development Tools` and `APIs` groups. Ignore the `Database` row (password) and everything after `Authentication Keys`. | [src] `apps/cli-go/internal/status/status.go` at v2.122.0 |
| kubectl | `port-forward` | `Forwarding from 127.0.0.1:8080 -> 80` and `Forwarding from [::1]:8080 -> 80` | Local port appears here even with `:5000` (random local port). | [src] `client-go/tools/portforward/portforward.go`; [local] help |
| kubectl | `proxy` | `Starting to serve on 127.0.0.1:8001` | No scheme. `--port=0` prints the chosen port. | [src] `kubectl/pkg/cmd/proxy/proxy.go:231`; [local] help |
| gh | `codespace ports forward` | `Forwarding ports: remote 80 <=> local 127.0.0.1:8080` | `<addr>` may be `127.0.0.1:N` or `[::]:N`. | [src] `pkg/cmd/codespace/ports.go:351` |
| gcloud | `compute start-iap-tunnel` | `Listening on port [2222].` | `Listening on port \[(\d+)\]`. Same text for `workstations start-tcp-tunnel`. | [local] `command_lib/compute/iap_tunnel.py:872-892`, `api_lib/workstations/workstations.py:318` |
| AWS | `ssm start-session` (port forward) | `Port 8080 opened for sessionId <id>.` | `Port (\d+) opened for sessionId`. Medium confidence. | [docs] example https://docs.aws.amazon.com/systems-manager/latest/userguide/example_ssm_StartSession_section.md |

Other sources behind the table: Supabase config defaults https://supabase.com/docs/guides/local-development/cli/config ; Vercel dev https://vercel.com/docs/cli/dev ; Wrangler https://developers.cloudflare.com/workers/wrangler/commands/workers/ ; Firebase ports https://firebase.google.com/docs/emulator-suite/install_and_configure .

Default ports, as hints only: Vite 5173 and 4173; Next 3000; `vercel dev` 3000; wrangler 8787; netlify dev 8888 and functions 9999; Supabase API 54321, Postgres 54322, Studio 54323, Mailpit 54324; Firebase UI 4000, Hosting 5000, Functions 5001, Firestore 8080, Pub/Sub 8085, Database 9000, Auth 9099, Storage 9199 (all [src]/[docs] as above).

The Supabase old format `API URL: http://127.0.0.1:54321` and `Studio URL: ...` is obsolete in current source. Tolerating it for older CLIs is [unverified].

### Tools with no reliable announcement, and the fallback

| Tool | What happens | Fallback |
|---|---|---|
| Docker and Compose | `up` and `run -p` print no URL or port ([docs] https://docs.docker.com/reference/cli/docker/compose/up/). Container logs have a `service-1  \| ` prefix. | Poll `docker compose ps`, `docker port <c>`, `docker compose port <service> <port>`. The PORTS column format is [unverified]. |
| ngrok | Redrawn TUI. Logfmt only with `--log=stdout`; its shape is [unverified]. | `GET http://127.0.0.1:4040/api/tunnels` ([docs] https://ngrok.com/docs/agent/api/; marked deprecated but still answers locally, verify live). `web_addr` default `127.0.0.1:4040` ([docs] https://ngrok.com/docs/agent/config/v3/). The public URL is bearer-like: show it separately and never log it. |
| Fly `proxy` | No printed line found in source or docs. | Take the local port from the launch command (`<local>:<remote>`, bind address default `127.0.0.1`) ([docs] https://docs.fly.io/flyctl/proxy). |
| Stripe `listen` | No own port; the ready line carries a secret. | Take the `--forward-to` port from the command. Use the Ready line only as a liveness hint. |
| Terraform, op, az login, aws login, gh auth | No ports. | None. Callback ports are transient. |
| gcloud emulators | Random 8000-8999 unless set. The Java emulator start lines are [unverified]. | Pass `--host-port` and read it from the command. Spanner emulator uses 9010 and 9020 ([local] `spanner_util.py:34-35`). |
| Supabase `functions serve`, `az aks browse`, `az network bastion tunnel` | Not verified. | Live-check before shipping a pattern. |

Do not match `fly redis proxy`: its line `Proxying redis to port "<p>" with password "<pw>"` carries a password ([src] `internal/command/redis/proxy.go:44`).

### Needs-you prompt texts

Kinds are in brackets. Evidence tags are from the tool report.

- **terraform** (`text`) [src] `internal/backend/local/backend_apply.go:140-170`, `internal/command/ui/ui_input.go:141-155`. Match `Enter a value:` at end of output plus `Only 'yes' will be accepted`. Lines: `Do you want to perform these actions?`, `Terraform will perform the actions described above.`, `Only 'yes' will be accepted to approve.`. Variants: `...in workspace "<ws>"?`, `Do you really want to destroy all resources?`, `Would you like to update the Terraform state to reflect these detected changes?`. The same `Enter a value:` also appears for missing input variables. `terraform init` backend prompts: [src] `command/meta_backend.go:3222-3236`. `terraform login` [src] `internal/command/login.go`: `Do you want to proceed?`, `Open the following URL to access the login page for <host>:`, `Terraform will now wait for the host to signal that login was successful.`, `Generate a token using your browser, and copy-paste it into this prompt.`, `Token for <host>:` (input hidden). The OAuth callback ports ~10000-10010 are [unverified].
- **gh auth login** (`text`, then `browser`) [src] `login_flow.go`, `internal/authflow/flow.go`. `First copy your one-time code: XXXX-XXXX` (match `First copy your one-time code: ([A-Z0-9]{4}-[A-Z0-9]{4})`), then `Press Enter to open <authURL> in your browser... `. Questions: `? Where do you use GitHub?`, `? How would you like to authenticate GitHub CLI?`. The `? ` glyph depends on the prompter. `gh repo delete` and `gh auth logout` confirmations: [unverified] strings.
- **docker login** (`browser`) [src] `internal/oauth/manager/manager.go`. Match `one-time device confirmation code` and `login.docker.com/activate`. Lines: `Your one-time device confirmation code is: <XXXX-XXXX>`, `Open this URL to authenticate: <url>`, `Press ENTER to open the URL in your browser, or Ctrl+C to cancel.`. Older phrasing is [unverified]. Fallback `Username: ` / `Password: ` [src] `cli/command/registry.go`. Prune confirms: `Are you sure you want to continue? [y/N] ` [src] `cli/command/system/prune.go`, `internal/prompt/prompt.go`.
- **kubectl** (`text`) [src] `staging/src/k8s.io/kubectl/pkg/cmd/delete/delete.go`: only with `delete --interactive`: `You are about to delete the following N resource(s):` and `Do you want to continue? (y/N): `. Credential-plugin auth prompts belong to the plugin; the symptom wording is [unverified].
- **gcloud** (`text`, `browser`) [local] `core/console/console_io.py:305-325`: `Do you want to continue (Y/n)?` or `(y/N)?`. Match `\((Y/n|y/N)\)\?\s*$`. Login [local] `core/credentials/flow.py`: `Your browser has been opened to visit:`, `Go to the following link in your browser, and complete the sign-in prompts:`, `Once finished, enter the verification code provided in your browser: `, `Enter the output of the above command: ` (remote bootstrap). The enable-API prompt wording is [unverified].
- **aws** (`browser`, `text`) [src] `awscli/customizations/sso/utils.py`, [docs] https://docs.aws.amazon.com/cli/latest/userguide/sso-configure-profile-token.html . Match `Then enter the code:` plus the next non-empty line `^[A-Z0-9]{4}-[A-Z0-9]{4}$`, and URLs on `device.sso.` or `oidc.` hosts. Also `Attempting to open your default browser.` `aws configure sso` prompts: `SSO session name (Recommended): `, `SSO start URL [None]: `, `SSO region [None]: `, `Default client Region [None]: `, `Profile name [...]: `. `aws login` overwrite question `(y/n): ` [src] `login/login.py:186-217`. `aws configure` static-key prompts are [unverified].
- **az** (`browser`, `text`) [src] `azure-cli-core/azure/cli/core/auth/identity.py`, `command_modules/profile/_subscription_selector.py`; [docs] https://learn.microsoft.com/en-us/cli/azure/authenticate-azure-cli-interactively . Device code: match `(microsoft\.com/devicelogin|aka\.ms/devicelogin)` and the 9-char code `[A-Z0-9]{9}` loosely. Browser flow: `A web browser has been opened at https://login.microsoftonline.com/organizations/oauth2/v2.0/authorize. Please continue the login in the web browser.` Subscription selector: `Select a subscription and tenant (Type a number or Enter for no changes): `. Extension-install and delete prompts are [unverified].
- **firebase** (`browser`, `text`) [src] `src/auth.ts`. `Visit this URL on this device to log in:` then `Waiting for authentication...`. With `--no-localhost`: `Paste or enter the authorization code below once you have it:` and `Enter authorization code:`. Confirm prompts: `Do you want to continue?` (`emulator/commandUtils.ts:135`). The `? ... (y/N)` rendering is [unverified].
- **supabase** (`browser`, `text`) [src] `apps/cli/src/command-internal/ensure-login.ts`. `Hello from Supabase! Press Enter to open browser and login automatically.`, `Here is your login link in case browser did not open <url>`, `Enter your verification code: `. Older [docs] flow: `Enter your access token:` (https://supabase.com/docs/reference/cli/supabase-login). `supabase link` database-password prompt is [unverified].
- **stripe** (`browser`, `text`) [src] `pkg/login/client_login.go`, `pkg/login/oauth_device.go`. `Your pairing code is: <word-word-word>`, `Press Enter to open the browser or visit <url> (^C to quit)` (no trailing newline), `To authenticate with Stripe, please go to: <url>`, `Waiting for confirmation...`. Device flow: `When prompted, confirm your verification code:`. `stripe login --interactive` asks `Enter your API key: ` (echo off).
- **fly** (`browser`, `text`) [src] `internal/command/auth/webauth/webauth.go`. `Opening <url> ...`, `Waiting for session...`, headless `Open this URL in a browser and approve the fly auth login:`. Confirmations use survey: `Destroy app <name>?`; the `(y/N)` suffix is [unverified]. `fly deploy` and `fly launch` prompts are [unverified].
- **netlify** (`browser`) [src] `src/commands/base-command.ts`, `src/utils/command-helpers.ts`. `Opening <url>` and `Waiting for authorization...`. Timeout and denial strings are in the tool report. Link and editor-helper prompts are [unverified].
- **vercel** (`browser`) [src] `packages/cli/src/commands/login/future.ts:113-158`. `Visit vercel.com/<device path>` plus ` and enter <USER_CODE>`, or the full URL; spinner `Waiting for authentication...`. Match `Visit ` plus `vercel.com` only, not a fixed path. Setup prompts (`Set up and develop ...?`, `Link to existing project?`) are [unverified].
- **wrangler** (`browser`) [docs] https://developers.cloudflare.com/workers/wrangler/commands/general/ . `Attempting to login via OAuth...` and `Opening a link in your default browser: <url>`. `wrangler secret put` prompts `? Enter a secret value:` [docs]. `wrangler delete` confirmation is [unverified]. Dev hotkeys (`[b] open a browser`, `[d] open devtools`, `[x] to exit`) are not prompts [src] `packages/wrangler/src/dev/hotkeys.ts`.
- **op** (`text`, manual sign-in only). Verbatim strings are [unverified] (closed source): `Enter the password for <email> at <address>:`, `Enter your six-digit authentication code:`, `Enter your Secret Key`. Sources: https://www.1password.dev/cli/sign-in-manually/ and https://1password.community/discussion/comment/582262/ [T]. `op read -o` and `op inject -o` confirm before overwriting [local]; the text is not verified.
- **npx / pnpm dlx** (`text`): `Ok to proceed? ` or `Ok to proceed? (y) ` (see rule 8).
- **ngrok**: no prompts in a normal `ngrok http` session. A missing authtoken exits with an `ERR_NGROK_4018` style message [unverified]; not a prompt.

### Prompts that text cannot detect

- **op with desktop-app integration** (`os-dialog`). Windows Hello or the 1Password app dialog; "the page doesn't say whether the terminal shows anything else": https://www.1password.dev/cli/app-integration/ . [local] `op signin --help` agrees. Failure names: `LostConnectionToApp`, `connectionreset`, `No accounts configured for use with 1Password CLI` [docs].
- **az login on Windows** (`os-dialog`). WAM is the default from Azure CLI 2.61.0 [docs]. The only text is a stderr warning `Select the account you want to log in with. For more information on login with Azure CLI, see https://go.microsoft.com/fwlink/?linkid=2271136` [src] `identity.py:36-37`. A weak hint that an OS dialog is up.
- **kubectl credential plugins.** The text belongs to the plugin.
- **Browser consent pages** for every device or PKCE flow. The terminal shows the URL or code; completion is silent until the spinner ends.
- For these, at most use a "process is alive, no output for N seconds" heuristic.

## Secret-printing commands

The full list from the tool report. A name in this list with no tag in its section of the report is `[unverified]`. Tags below are kept as the report gives them. "argv" means the secret is typed on the command line.

- **op**: `op read` [local]; `op item get --reveal` [local]; `op item get --fields` [local]; `op item get --otp` [local]; `op item get --share-link` [docs]; `op item get --format json` [docs] (treat as secret); `op document get` [local]; `op inject` without `-o` [local]; `op run --no-masking` [local]; `op signin --raw` [local]; `op account add --raw` [local]; `op signin` manual-mode session export [local]; `op service-account create` [local]; `op connect token create` [local]. `op item create` and `op item edit` with assignment arguments put secrets in argv [docs] https://www.1password.dev/cli/reference/management-commands/item/ .
- **docker**: `docker login -p` / `--password` [src] `login.go:132` (argv; the CLI warns it is insecure); `docker compose config` [local] (interpolates `.env`); `docker inspect` [unverified]; `docker exec <c> env` and `docker compose run/exec ... env` [unverified]; `docker logs` of apps that log secrets [unverified]; `docker context inspect` [unverified]; `docker buildx build --secret` and build args, visible in `docker history` [unverified].
- **supabase**: `supabase start` [src] (prints the status tables with keys, S3 keys and a DB URL with password); `supabase status` in every `-o` format [src]; `supabase login --token` [docs] (argv); `supabase projects api-keys` [unverified]; `supabase secrets list` (digests only) [unverified]; `supabase db dump` [unverified].
- **gh**: `gh auth token` [local]; `gh auth status --show-token` [local]; `gh secret set -b` (argv) [unverified]; `gh auth login --with-token` is safe unless piped from a typed `echo $TOKEN |` [local]; `gh config get` may show `oauth_token` when stored in plain text [local]; `gh api` with `Authorization` headers or `--include` [unverified].
- **kubectl**: `kubectl get secret -o yaml|json|jsonpath|go-template` [local]; `kubectl config view --raw` [local]; `kubectl create token` [local]; `kubectl create secret --from-literal` and `create secret docker-registry` (argv) [local]; `kubectl get pod -o yaml`, `kubectl exec ... env|printenv`, `kubectl logs` [local]; `--token` (argv).
- **vercel**: `vercel env pull` (writes `.env.local` to disk), `vercel env run`, `vercel env add`, `vercel api` [src]/[docs]; `vercel --token` (argv) [docs]; `vercel env ls` [unverified].
- **wrangler**: `wrangler auth token` [docs]; `wrangler secret put` and `secret bulk` [docs]; `wrangler whoami` [unverified]; `wrangler d1 execute --remote` and `kv key get` print stored data [unverified].
- **firebase**: `firebase login:ci` [src] `src/commands/login-ci.ts`; `firebase functions:secrets:access` [unverified]; `firebase --token` (argv); `firebase apps:sdkconfig` and `auth:export` [unverified].
- **aws**: `aws configure export-credentials` [unverified]; `aws configure get aws_secret_access_key` [unverified]; `aws configure list` (tail visible) [unverified]; `aws sts assume-role`, `get-session-token`, `get-federation-token` [unverified]; `aws sso get-role-credentials` [unverified]; `aws iam create-access-key` [unverified]; `aws eks get-token` [unverified]; `aws ecr get-login-password` [unverified]; `aws secretsmanager get-secret-value` [unverified]; `aws ssm get-parameter --with-decryption` [unverified]. Also in the report's list: `aws codeartifact get-authorization-token`, `aws kms decrypt`, `aws lambda get-function-configuration`, `aws apigateway get-api-keys --include-values`, `aws rds generate-db-auth-token`, all [unverified]. `aws configure set aws_secret_access_key <v>` (argv).
- **gcloud**: `gcloud auth print-access-token` [local]; `gcloud auth print-identity-token` [local]; `gcloud auth application-default print-access-token` [local]; `gcloud config config-helper` [local]; `gcloud secrets versions access` [local]; `gcloud auth activate-service-account --key-file` [local] (a path; fine); `gcloud iam service-accounts keys create` [unverified]; `gcloud kms decrypt` [unverified]; `gcloud sql users set-password --password` [unverified] (argv); `gcloud compute instances get-serial-port-output` [unverified]; `gcloud run services describe` and `gcloud functions describe` (env vars) [unverified].
- **az**: `az account get-access-token` [docs]; `az login -p` and `--service-principal -u <appId> -p <secret>` [docs] (argv); `az ad sp create-for-rbac` [unverified]; `az keyvault secret show` and `secret download` [unverified]; `az storage account keys list` [unverified]; `az acr credential show` [unverified]; `az webapp config appsettings list` [unverified]. Also [unverified]: `az ad app credential reset`, `az storage account show-connection-string`, `az cosmosdb keys list`, `az servicebus ... authorization-rule keys list`, `az functionapp config appsettings list`, `az acr login --expose-token`, `az sql ... connection-string`.
- **terraform**: `terraform output -json` and `-raw` [unverified]; `terraform show -json` [unverified]; `terraform state show` and `state pull` [unverified]; `terraform -var` (argv) [unverified]; `TF_LOG=DEBUG` prints request bodies [unverified]; `terraform login` token prompt and `%APPDATA%\terraform.d\credentials.tfrc.json` [src] `login.go:680-682`.
- **fly**: `fly auth token` [src] (file exists); `fly tokens create` [unverified]; `fly ssh issue` [unverified]; `fly redis proxy` [src] `redis/proxy.go:44`; `fly -t` / `--access-token` (argv); `fly secrets set KEY=VALUE` (argv); `fly postgres connect` and `fly mpg ...` [unverified].
- **netlify**: `netlify env:list` [src] (names and values, `--plain`); `netlify env:get` and `env:export` [src]/[unverified]; `netlify --auth` (argv); `netlify api` responses [unverified].
- **ngrok**: `ngrok config add-authtoken` [docs] (argv); `NGROK_AUTHTOKEN`; the inspector at `127.0.0.1:4040` shows request and response bodies including `Authorization` headers; `--basic-auth user:pass` (argv).
- **stripe**: `stripe listen` ready line with `whsec_...` and `--print-secret` [src] `pkg/cmd/listen.go:338`, `listen.go:135`; `stripe config --list` [unverified]; `stripe login --interactive` [src]; `stripe --api-key` (argv); `stripe get /v1/...` responses (client secrets) [unverified].
- **Files never to read**: `~/.docker/config.json`, `~/.supabase/access-token`, `%APPDATA%\com.vercel.cli\Data\auth.json`, `.dev.vars` and `.env*`, `~/.aws/credentials`, `~/.aws/sso/cache/*.json`, `%USERPROFILE%\.azure\msal_token_cache.*`, `terraform.tfstate*`, `*.tfvars`, `%APPDATA%\netlify\Config\config.json`, `%LOCALAPPDATA%\ngrok\ngrok.yml`, `%USERPROFILE%\.config\stripe\config.toml`, `~/.grok/auth.json`, `~/.kimi-code/credentials/*.json`.
- **Argv rule.** The report's list of flags that put a secret on the command line (see the items above) means saved launch commands, history and tab titles must be scrubbed, not only output.

## Live check

The slice 6b live check ran opencode 1.19.4, Qwen Code 0.26.1, Crush v0.98.2, Kimi Code CLI 2.3.1 and Grok 1.0.50 to 1.0.70 inside Marshell. The results are in [2026-10-25-agent-manifests-live-check.md](2026-10-25-agent-manifests-live-check.md): the flags, the argv per mode, the terminal signals, Grok's hook sources and process chain, and the findings.

## Open verification items

Agent CLIs.

- [ ] Copilot: exit resume hint text (changelog 1.0.72); exact winget `copilot.exe` name; sandbox flag names; which of `beep` default true and the 1.0.55-1 opt-in note holds; repo-hook trust prompt flow; a non-interactive status command; the wider Copilot terms.
- [ ] Cursor: which hook events fire in the CLI on the installed version; Windows chats path and whether the id is the directory name; chat id printed on exit; escape sequences for `notifications` and the title; Acceptable Use Policy text (2026-08-11); ACP page not fetched directly; nested and global instruction files.
- [ ] opencode: V1 against V2 flag set (`-c`, `-s`, `--model` in V2); Windows install route in V2 (docs against changelog); `--auto` in `--help`; global plugin path on Windows; a plugin way to answer a pending permission; OSC used for the title and for `attention` notifications; Zen and Go terms; provider terms for subscription logins; whether it prints the id on exit.
- [ ] Amp: native Windows support; local session path; interactive launch with a first prompt; `amp threads continue` with no id; `--mode` and `--dangerously-allow-all` officially; plugin prerequisites (`PLUGINS=all`, binary install); OSC title; Acceptable Use Policy text.
- [ ] Grok: version newer than 1.0.50; consumer terms for third-party hosts; notifications for an unknown terminal without focus tracking; the third-party repo-upload claim; `GROK_CLAUDE_HOOKS_ENABLED` and `GROK_HOOK_EVENT` are from the 2026-10-25 docs check and are not in the group B report.
- [ ] Qwen Code: the real exe behind `qwen.cmd`; `projects\...\chats` against older `tmp\<id>\chats` on a real install; `--session-id` and `--fork-session` semantics; Alibaba Coding Plan terms.
- [ ] Kimi Code CLI: the notifications channel; per-project hook location; exact exe filename and `bin` subfolder.
- [ ] Aider: `--load` with plain text lines; absence of an OSC title.
- [ ] Crush: interactive initial prompt; OSC 0 or 2 for the title; the OSC type for `osc` notifications; bell plus focus reporting in an xterm.js host; ACP; FSL "competing use" if Marshell bundles it; Hyper terms.
- [ ] DeepSeek Harness: default `$DSH_HOME`, session store, resume flags, instruction files, hook semantics; relation of `deepseek-harness/turtle-ui` to DeepSeek; Anthropic's position on third-party back ends for Claude Code.

Tool profiles.

- [ ] op: verbatim manual sign-in prompt strings; the `op read -o` / `op inject -o` overwrite prompt text.
- [ ] docker: older `docker login` phrasing; PORTS column format; the secret-leaking commands tagged unverified above; installed 29.9.2 was not run.
- [ ] supabase: obsolete status format for older CLIs; `functions serve` output; `projects api-keys`, `secrets list`, `link` prompt.
- [ ] gh: `repo delete` and `auth logout` prompt strings; `gh secret set` prompt; `gh copilot` prompts; `gh api` secret endpoints.
- [ ] kubectl: `get pods -w`; credential-plugin error wording.
- [ ] vercel: success text after login; setup prompt wording; `env ls` and `env add` prompts.
- [ ] wrangler: `pages dev` output; `wrangler delete` prompt; `dev` login prompt with `--remote`; `whoami` contents.
- [ ] firebase: standalone `firebase-tools-win.exe`; confirm-prompt rendering; `functions:secrets:*`.
- [ ] aws: which CLI version introduced `aws login`; `ssm start-session` args and the missing-plugin error; `aws configure` prompts; `configure list` masking; the STS and other credential-printing commands listed as unverified.
- [ ] gcloud: Java emulator start lines and the `env-init` output; Pub/Sub and Firestore default ports; `start-iap-tunnel` argument shape; the enable-API prompt wording; `gcloud init` and `gcloud copilot`-style prompts not researched.
- [ ] az: extension-install and delete prompt wording; `aks browse` and `bastion tunnel` announcements; the credential-printing commands tagged unverified.
- [ ] terraform: OAuth callback ports for `login`; the output and state commands tagged unverified.
- [ ] fly: `logs` default follow; `deploy` and `launch` prompts; survey `(y/N)` suffix; `postgres connect`, `mpg`, `secrets list`.
- [ ] netlify: link and editor-helper prompts; older `Server now ready on` banner.
- [ ] ngrok: `ERR_NGROK_4018` text; port fallback when 4040 is busy; the logfmt shape; the TUI text; `tcp` and `start --all` output; whether `/api/tunnels` still answers.
- [ ] stripe: event-line shape after `Ready!`; `logs tail`; `config --list` masking.
- [ ] npx: the trailing `(y)` marker in the install prompt.
