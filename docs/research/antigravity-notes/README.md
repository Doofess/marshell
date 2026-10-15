# Antigravity CLI notes (unverified)

Written on 2026-10-10 by another CLI at the owner's request, without sources, as input for the phase 6 adapter. Kept as research, not as a spec.

**Verified against vendor pages the same day** (`docs/research/2026-10-10-cli-data-sources.md`, section "Antigravity CLI, verified"): binary `agy`; settings at `~/.gemini/antigravity-cli/settings.json`; a `statusLine` command that receives model, context, quota and agent state on stdin; plugins can ship `hooks.json` that run "before or after tool calls".

**Conflicts to settle with a real `agy` install and one captured payload each:**
- These notes say Antigravity has no status line (`statusline` capability false); the vendor pages show one. The consent card in the design offers the status line.
- These notes say a `PreToolUse` hook can grant permission (`decision: allow | deny | ask | force_ask`) and give camelCase payload fields (`conversationId`, `stepIdx`, `toolCall`). The vendor pages name no hook events and no permission grant. Until confirmed, the design keeps the status and approve switches as "verified when the adapter is built".
- Hook file location (`~/.gemini/config/hooks.json` vs the CLI's own settings or plugin folders), transcript path and schema, the headless recap flags and the brand colour suggestion in file 05 (the design already defines Antigravity's mark, colour and jellyfish motion) are all unconfirmed.

Do not implement from these notes without that check.
