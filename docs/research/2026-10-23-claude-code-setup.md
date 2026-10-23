# Claude Code project setup: evidence and inventory

- **Date:** 2026-10-23.
- **Branch:** `claude-setup`.
- **Plan:** `docs/superpowers/plans/2026-10-23-claude-code-project-setup.md`.

This note records:
- the documented behavior the project setup rests on;
- the before and after inventory, measured from a session started in the `claude-setup` worktree;
- the check that nothing outside this project changed.

## Documented behavior this setup relies on

| Behavior | Source |
|---|---|
| **Precedence:** managed > `--settings` > `.claude/settings.local.json` > `.claude/settings.json` > `~/.claude/settings.json`. | [settings](https://code.claude.com/docs/en/settings.md) |
| **Merging:** lists such as `permissions.deny` and `deniedMcpServers` merge across files. `enabledPlugins` merges per plugin id, and the highest file naming an id decides. | [settings](https://code.claude.com/docs/en/settings.md) |
| **Synced plugins:** a project file can switch off a plugin enabled at user level, including claude.ai synced plugins written as `<name>@synced`. | [plugin loading](https://code.claude.com/docs/en/plugins/loading.md) |
| **Connectors:** `deniedMcpServers` blocks claude.ai connectors by display name, from any scope. | [mcp](https://code.claude.com/docs/en/mcp.md) |
| **`skillOverrides`:** works from the project file. Its values are `on`, `name-only`, `user-invocable-only` and `off`, and it matches user and bundled skills by name. Plugin skills ignore it. | [skills](https://code.claude.com/docs/en/skills.md) |
| **Hook deny:** a PreToolUse `permissionDecision: "deny"` blocks the call even in `bypassPermissions` mode. Exit 1 does not block. | [hooks](https://code.claude.com/docs/en/hooks.md) |
| **Hook path and shell:** `${CLAUDE_PROJECT_DIR}` is the folder the session started in. A worktree session started inside the worktree uses that worktree. Shell-form hooks run in Git Bash when it's installed. | [hooks](https://code.claude.com/docs/en/hooks.md) |
| **Name clashes:** a personal skill beats a project skill of the same name, hence the `marshell-` prefix. A project agent beats a user agent. | [skills](https://code.claude.com/docs/en/skills.md), [sub-agents](https://code.claude.com/docs/en/sub-agents.md) |
| **Path-scoped rules:** a `.claude/rules/*.md` file with `paths:` frontmatter loads only when Claude reads or edits a matching file. | [memory](https://code.claude.com/docs/en/memory.md) |
| **Worktrees:** a worktree loads the tracked `.claude/settings.json`, `.mcp.json`, skills, agents and rules of its own branch. On Windows, `.claude/settings.local.json` stays in each checkout, so per-person approvals don't carry over; `.worktreeinclude` copies it into worktrees that Claude Code creates. | [worktrees](https://code.claude.com/docs/en/worktrees.md), [settings](https://code.claude.com/docs/en/settings.md) |
| **Double `CLAUDE.md`:** `CLAUDE.md` loads from every folder above the start folder, so a session inside `.claude/worktrees/<x>` may also load the main checkout's copy. This is not documented either way. | [memory](https://code.claude.com/docs/en/memory.md) |
| **`-p` sessions:** these may skip project plugin and marketplace loading, so plugin state is checked with `claude plugin list` and not with `-p`. | [plugin install](https://code.claude.com/docs/en/plugins/install.md) |

## Before and after

Every measurement comes from a session started in the `claude-setup` worktree. The "before" ran with none of the project files.

| | Before | After |
|---|---|---|
| Tokens before the first reply (`input + cache_creation + cache_read` for "Reply with only: ok") | 39,901 | 33,259 (−6,642, −17%) |
| claude.ai connectors | Resend, Cloudflare Developer Platform, Supabase (all connected); Adobe already off for this project through `/mcp` | none connected; Adobe still off |
| Plugin MCP servers | context7, playwright, engineering:datadog | context7, playwright |
| Project MCP servers | none | `webview` (Playwright 0.0.83 over CDP at 127.0.0.1:9333) |
| Plugins | pyright-lsp, design@synced, engineering@synced enabled | those three disabled; good-css enabled at project scope |
| Skills removed | | underwater-photoshop, dataviz, 7 `design:*`, 10 `engineering:*` |
| Skills added | | good-css:good-css |

## Nothing outside this project changed

The user's files were hashed before and after (SHA-256):
- `~/.claude/settings.json`, without `advisorModel`, which the 5b run switches;
- `~/.claude/CLAUDE.md`;
- every file under `~/.claude/agents/` and `~/.claude/skills/`;
- `~/.claude.json`'s settings for other projects.

The only differences:
- **`skills/synced/<account>/.last-complete-round`:** Claude Code's own skill-sync bookkeeping, written by every session.
- **The good-css entry in `~/.claude/plugins/installed_plugins.json` and the plugin cache:** `claude plugin install --scope project` installs into the user's cache. Only this repo's settings enable the plugin.

The Adobe connector is off in this project through the per-project `/mcp` toggle (`~/.claude.json`), not through `deniedMcpServers`, so `/mcp` can switch it back on per project, as the global setup intends.
