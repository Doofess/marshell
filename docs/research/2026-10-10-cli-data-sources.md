# What each CLI can tell Marshell (research, 2026-10-10)

Purpose: decide which items of the prompt bar's status strip (modelled on the user's `statusline.py`) work for every CLI, and what each vendor calls its extensions, for the per-vendor menus in `docs/IDEAS.md`. Checked against current vendor docs on 2026-10-10. "Unverified" means no primary source confirmed it; settle those with a real install before building (dump one status-line payload to a file; read one real Codex session file).

## Is a status line a "mod"? (Claude Code)

No. Claude Code's docs name five separate extension kinds: **mods, settings hooks, skills, status lines and MCP servers**. A **mod** is a plugin whose JavaScript or TypeScript runs *inside* Claude Code (`hooks/register.tsx`, `hooks/hooks.json` with `modules`, installed like a plugin, needs Claude Code 2.1.287 or newer, early access). A **status line** is the `statusLine` setting: an *external* command that gets session JSON on stdin and prints text (`statusline.py` is one). The docs list status lines with hooks, skills and MCP servers as things that "work from outside Claude Code", unlike mods.
The two can look alike: a mod can draw its own band above the prompt, and the user's `readable-session` is a mod that does. Disabling hooks (`disableAllHooks`) stops installed mods and a custom status line alike, but they are different things. Plugins can also ship a default `subagentStatusLine`.
Sources: [Mods overview](https://code.claude.com/docs/en/plugins/mods/overview), [Mods reference](https://code.claude.com/docs/en/plugins/mods/reference), [Status line](https://code.claude.com/docs/en/statusline).

## Where each strip item can come from

Source codes: a = Marshell computes it itself, b = pushed to a status-line command on stdin, c = hook event, d = on-disk transcript or log, e = a query (command, endpoint), f = parse the screen (fragile), g = not available.

**Universal (a), for every CLI, because Marshell owns the pty child:** project and branch with dirty count (`git status --porcelain=v2`), elapsed time (its own clock from spawn), listening ports of the process tree (OS socket table filtered to child PIDs; better than statusline.py's probe of common ports), env tag (`VIRTUAL_ENV`, `CONDA_DEFAULT_ENV`), CLI version (`<cli> --version`; "update available" needs one registry lookup per vendor).

| CLI | Push channel (b) | Model / effort | Context % and tokens | Plan meters | Cost | Session name | MCP | Permission mode |
|---|---|---|---|---|---|---|---|---|
| Claude Code | `statusLine` in settings.json | b | b | b `rate_limits` 5h and 7d with resets | b | b once named | `claude mcp list`; live state g | c (hooks only) |
| Codex CLI | none (built-in footer items only) | c model; effort unverified | d rollout `token_count` (unverified) | d rollout rate limits (unverified); e app-server | g | g | `codex mcp list --json` (config only) | c |
| Gemini CLI (dropped 2026-10-10: Antigravity is Google's one agent in Marshell; row kept for the record) | none (built-in footer toggles) | d telemetry model | d telemetry input tokens; window size needs a model table | f `/stats` | g | g | d telemetry (startup only) | d telemetry (startup only) |
| Antigravity CLI | `statusLine` in `~/.gemini/antigravity-cli/settings.json` | b | b | b one quota bucket, not 5h/7d | g | g | g | b `execution_mode` and `tool_confirmation_pending` (no permission-mode field) |
| Copilot CLI | `statusLine` in `~/.copilot/settings.json` (experimental) | b model | b | f `/usage` | premium requests only (unverified) | unverified | `copilot mcp` | g |
| Cursor Agent | `statusLine` in `cli-config.json` (undocumented) | b model | b | g | g | g | unverified | g |
| opencode | none external (in-process plugins) | e local HTTP | e / d | g | e (unverified) | e | e `GET /mcp` | c |
| Qwen Code | `ui.statusLine` command | b | b | g | g | g | g | g |
| Amp | none (plugin status items, experimental) | g | g | `amp usage` (unverified) | display only | g | g | g |
| Grok CLI | unverified | | | | | | | |

What `statusline.py` gets from Claude's stdin: model, effort, context window, `rate_limits`, cost, duration, `session_name`, `version`, `workspace`, `transcript_path`. What it digs for itself: git, the transcript title, plan meters via the undocumented OAuth usage endpoint, MCP health from `mcp-logs-*`, skills count, update check, a port probe, venv. Permission mode is not shown (hooks only).

Integration idea: install a Marshell relay as the `statusLine` command for Claude, Antigravity, Copilot, Cursor and Qwen. It forwards the stdin JSON to Marshell and prints the user's own line. Cursor warns that a running session can overwrite `cli-config.json`, so edit it only while no session is open.

## Verdict

*Accepted by the owner on 2026-10-10: the status strip shows only what a CLI can really supply, and per-vendor menus use each vendor's own terms.*

- **Universal today:** project, branch, dirty count, elapsed time, ports, env tag, CLI version.
- **Easy per-vendor adapter:** model and context % for Claude, Antigravity, Copilot, Cursor, Qwen (through the relay).
- **Medium:** Codex model, context and 5h/weekly meters (tail the rollout file, an unofficial format, or the app-server); opencode through its local HTTP server; permission mode for Claude and Codex through hooks.
- **Hard or fragile, leave out:** Gemini CLI altogether (dropped 2026-10-10: Google replaced it with Antigravity CLI for free and Google One users on 2026-06-18, and Marshell follows); MCP health for any vendor but opencode; plan meters outside Claude, Codex and Antigravity; cost outside Claude.
- **Recommended minimum common strip:** project · branch with dirty count · elapsed time · ports · env tag · CLI version. Add model and context % only where a push channel or structured source exists. This matches the design rule that unknown values are left out, never guessed.

## Antigravity CLI (`agy`), verified against the vendor docs on 2026-10-10

What the Phase 0 consent card and fixtures state, and where each fact comes from:

- **Binary and install.** `agy`. `irm https://antigravity.google/cli/install.ps1 | iex` on Windows puts it in `%LOCALAPPDATA%\agy\bin`; `curl -fsSL https://antigravity.google/cli/install.sh | bash` puts it in `~/.local/bin/agy` on macOS and Linux. No npm package is named. ([getting started, CLI tab](https://www.antigravity.google/docs/getting-started?tab=cli))
- **Status line.** `statusLine` in `~/.gemini/antigravity-cli/settings.json`: `{ "type": "command", "command": "~/.gemini/antigravity-cli/statusline.sh" }`, with optional `padding`, `enabled` (false suspends it) and `stack_with_default` (show it below the built-in line). The command gets JSON on stdin: `cwd`, `session_id`, `conversation_id`, `transcript_path`, `model` (`id`, `display_name`), `workspace`, `version`, `product`, `email`, `plan_tier`, `context_window` (token totals, `context_window_size`, `used_percentage`, `remaining_percentage`), `exceeds_200k_tokens`, `quota` (per model: `remaining_fraction`, `reset_time`, `reset_in_seconds`), `agent_state` (`idle`, `thinking`, `working`, `tool_use`, `initializing`), `vcs`, `sandbox`, `artifact_count`, `pending_input_count`, `tool_confirmation_pending`, `task_count`, `terminal_width`, `execution_mode` (`planning`, `fast`), `vim`. There is no permission-mode field. ([statusline](https://www.antigravity.google/docs/cli/statusline))
- **Plugins.** Components: skills, agents, rules, MCP, and `hooks.json` ("event handlers executing shell commands before or after tool calls"). Stored in `~/.gemini/antigravity-cli/plugins/<plugin_name>/`; `agy plugin list` (or `/plugin list` inside the TUI). ([plugins, CLI tab](https://www.antigravity.google/docs/plugins?tab=cli))
- **Unverified, to settle when the adapter is built:** hook event names; whether the CLI reads hooks from `settings.json` as well as from a plugin; whether any hook can grant or deny a tool confirmation; the CLI's version string; the instruction-file layout for rules. Until then the consent card offers only the status line, counts no hook entries, and the approve card focuses the tab.

## Each vendor's extension terms, and how to list them from outside

| CLI | Terms | Runs inside the CLI? | List from outside |
|---|---|---|---|
| Claude Code | plugins, marketplaces, skills, hooks, MCP servers, subagents, slash commands, output styles, statusLine, **mods** | mods only | `claude plugin list --json`, `claude mcp list`, `~/.claude/skills`, settings.json |
| Codex | plugins, marketplaces, skills, hooks, MCP, `notify`, subagents | no | `codex mcp list --json`, `config.toml`, `hooks.json` |
| Gemini CLI (dropped 2026-10-10) | extensions (bundle MCP, commands, context, skills, hooks, themes), custom commands, agents | no | `gemini extensions list`, `gemini mcp list`, `gemini skills list` |
| Antigravity | plugins (skills, agents, rules, MCP, hooks), statusLine | no | `~/.gemini/antigravity-cli/plugins/`; `agy plugin list` (community source) |
| Copilot CLI | plugins, marketplaces, skills, custom agents, hooks, MCP, statusLine | no | `copilot plugin`, `copilot skill`, `copilot mcp` |
| Cursor Agent | rules, skills, MCP, hooks (unverified in the CLI), statusLine | no | `cli-config.json`, `.cursor/mcp.json` |
| opencode | plugins, agents, commands, MCP | plugins (JS/TS) | `opencode mcp list`, `opencode agent list`, `opencode session list --format json`, local HTTP |
| Qwen Code | Gemini-style extensions, statusLine | no | `~/.qwen/settings.json` |
| Amp | plugins (TS/JS), skills, MCP, toolboxes | plugins | `amp plugins list`, `~/.config/amp/settings.json` |

So "mods" is Claude Code's word for in-process plugins; opencode and Amp have in-process plugins under other names. A per-vendor menu should use each vendor's own words and only show the kinds that vendor has.

## Sources

Claude: [statusline](https://code.claude.com/docs/en/statusline), [hooks](https://code.claude.com/docs/en/hooks), [plugin CLI](https://code.claude.com/docs/en/plugins/cli-reference), [mods](https://code.claude.com/docs/en/plugins/mods/overview). Codex: [config reference](https://learn.chatgpt.com/docs/config-file/config-reference), [hooks](https://learn.chatgpt.com/docs/hooks), [status-line request #17827](https://upd.dev/openai/codex/issues/17827), [app-server rate limits PR #5302](https://github.com/openai/codex/pull/5302). Gemini: [configuration](https://geminicli.com/docs/reference/configuration), [hooks](https://geminicli.com/docs/hooks/reference/), [telemetry](https://geminicli.com/docs/cli/telemetry/). Antigravity: [statusline](https://www.antigravity.google/docs/cli/statusline), [plugins](https://www.antigravity.google/docs/cli/plugins). Copilot: [CLI reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference), [hooks](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-hooks-reference). Cursor: [CLI config](https://cursor.com/docs/cli/reference/configuration). opencode: [server](https://opencode.ai/docs/server/), [plugins](https://opencode.ai/docs/plugins/). Qwen: [status line](https://qwenlm.github.io/qwen-code-docs/en/users/features/status-line/). Amp: [settings](https://ampcode.com/docs/cli/settings), [plugins](https://ampcode.com/docs/customize/plugins).
