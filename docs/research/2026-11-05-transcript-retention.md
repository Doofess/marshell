# Transcript retention in the four CLIs (slice 7a)

- Date: 2026-11-05. Checked from each CLI's docs and source. Nothing was run.
- Why: slice 7a decision 6 offers to raise a CLI's transcript retention while the person left it at the default. This note records which CLIs have such a setting, and what it deletes.

## Claude Code: `cleanupPeriodDays`

- **Where it's set:** user, project or local settings.
- **Values:** the default is 30 and the minimum is 1. A `0` fails validation; it doesn't mean "keep nothing" (https://code.claude.com/docs/en/claude-directory.md, "Cleaned up automatically").
- **What it deletes:** files older than the cutoff:
  - session transcripts, `projects/<project>/<session>.jsonl`, with their subagent and tool-result folders;
  - `file-history/`, `plans/`, `debug/`, `paste-cache/`, `image-cache/`, `tasks/` and more (same page).
- **What it keeps:**
  - `history.jsonl`, unless the HIPAA configuration applies;
  - auto memory;
  - transcripts of sessions started in Claude Desktop or Cowork, unless managed settings set the key.
- **When it runs:** at most once per session. It is skipped for `claude -p --bare` (https://code.claude.com/docs/en/monitoring-usage.md).
- **Doctor:** offered. The item is shown only while the key is unset, and its fix sets it to 365. Marshell's History and resume read these transcripts, so they lose sessions older than the cutoff.

## Qwen Code: `general.cleanupPeriodDays`

Source: `QwenLM/qwen-code` main at b9e2083, not the exact version installed here.

- **Values:** the default is 30 and the minimum is 0. A restart is needed (`packages/cli/src/config/settingsSchema.ts:614-627`).
- **What it deletes:** only three things (`packages/cli/src/utils/housekeeping/cleanup.ts`):
  - the `/rewind` backups under `~/.qwen/file-history/`;
  - background subagent transcripts under `<projectDir>/subagents/`;
  - session debug logs.
- **What it keeps:** chat recordings (`general.chatRecording`, default on).
- **Other retention keys:** OpenAI API logs have their own key, `model.openAILogRetentionDays`, default 7.
- **When it runs:** a background pass 10 minutes after launch, or after 1 minute when the last sweep is more than 7 days old. It then repeats about daily (`scheduler.ts`).
- **Doctor:** not offered, because the key doesn't put chats at risk. No other age-based deletion of chats was found. That isn't a full audit of Qwen's code.

## Codex CLI

Source: `openai/codex` main, `codex-rs/core/config.schema.json`, not pinned to the installed version.

- **`[history] persistence`:** `save-all` (the default) or `none`. It controls whether Codex writes `~/.codex/history.jsonl`.
- **`[history] max_bytes`:** no cap by default. When it is set, the oldest prompt-history entries are dropped.
- **No age limit:** no key deletes session rollouts by age. `memories.max_rollout_age_days` and `memories.max_unused_days` limit what feeds memory generation, and delete nothing.
- **Compression:** rollouts are compressed to `.zst` (`2026-10-24-codex-rollouts.md`).
- **Doctor:** not offered.

## Antigravity (`agy`)

- **What's documented:**
  - `agy --help` shows no retention flag.
  - https://antigravity.google/docs/settings lists `historySize` (default 2000), which caps prompt-history entries, not conversations.
  - https://antigravity.google/docs/cli/conversations says nothing about retention.
- **Doctor:** not offered. The copy says Antigravity documents no such setting, and doesn't claim it keeps conversations forever.

## The Doctor's copy

The Claude Code item ends with this sentence:

> Codex and Qwen Code have no setting that deletes chats by age, and Antigravity documents none.
