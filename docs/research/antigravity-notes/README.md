# Antigravity CLI notes (partly verified)

Written on 2026-10-10 by another CLI at the owner's request, without sources, as input for the phase 6 adapter. Kept as research, not as a spec.

**Verified against vendor pages the same day** (`docs/research/2026-10-10-cli-data-sources.md`, section "Antigravity CLI, verified"): binary `agy`; settings at `~/.gemini/antigravity-cli/settings.json`; a `statusLine` command that receives model, context, quota and agent state on stdin; plugins can ship `hooks.json` that run "before or after tool calls".

**Checked against the installed agy 1.3.3 on 2026-10-15** (`docs/research/2026-10-15-local-install-check.md`, from `--help`, the bundled `agy-customizations/docs/hooks.md` and strings in the binary; no payload captured yet):
- **Confirmed:** `PreToolUse` grants permission (`decision: allow | deny | ask | force_ask`); payloads are camelCase (`conversationId`, `stepIdx`, `toolCall`); the `hooks.json` shape (named hooks with `enabled`, grouped `PreToolUse`/`PostToolUse`, flat `Stop`); the global file is `~/.gemini/config/hooks.json`; `agy -p` and `--dangerously-skip-permissions` exist.
- **Contradicted:** "no status line" (agy has `statusLine` with model, context, quota, cost and title); hooks in `settings.json` (they live only in `hooks.json`); the transcript path (it is `antigravity-cli/brain/<conversationId>/.system_generated/logs/transcript.jsonl`); a 600 s hook timeout (the default is 30 s, so Marshell sets 600 on `PreToolUse` itself).
- **Still unconfirmed:** the transcript schema, the headless recap flags beyond `-p`, and the brand colour in file 05 (the design already defines Antigravity's mark, colour and jellyfish motion).

The design now follows the confirmed parts (`docs/PLAN.md`, "Corrections to the brief"). Use these notes only where the check confirms them.
