---
paths:
  - "crates/core/src/config/**"
  - "crates/core/src/agents/**"
---

# Config write rules

- Configuration and instruction-file writes are safety-critical. Never silently edit a user's CLI configuration. (AGENTS.md)
- Every write goes preview, then the user sees a unified diff, then apply. (PLAN.md §6)
- Apply in order: take a per-path lock, check `expected_hash`, snapshot the original, write a temp file in the same directory and fsync, rename, record the new hash, update `managed.json`. (PLAN.md §6)
- Edit JSON only through CST edits with `jsonc-parser`. If a file doesn't parse, fail closed and don't edit it. (PLAN.md §6)
- Edit TOML with `toml_edit`, and Markdown only inside tagged `marshell:begin` / `marshell:end` blocks. (PLAN.md §6)
- Tag hook entries with the bridge path plus `--tag mshl1`. `managed.json` is the uninstall source of truth. (PLAN.md §6)
- `apply` hard-rejects managed and organization paths. Never run elevation commands. (PLAN.md §6)
- Every apply is one snapshot, and "Undo last fix" restores it only if the file still matches the post-write hash. (PLAN.md §6)
- Keep one CLI's conventions out of the others: Codex hook trust and `AGENTS.md` generation stay in Codex code, Claude Code's settings hooks stay in Claude Code code. (AGENTS.md)
- Before Codex adapter, hook or instruction-file work, read `docs/research/codex-cli.md`. (AGENTS.md)
- Codex hook installs end with a "trust these hooks in `/hooks`" step. (PLAN.md §6)
- Tests and live checks use `MARSHELL_HOME` and temp folders. Never touch `~/.claude/`, `~/.codex/` or `~/.marshell/`. (SCOPE.md)
