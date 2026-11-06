import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { FilePreview } from "../../generated/FilePreview";
import { unifiedDiff } from "../../lib/diff";
import type { DoctorFixPlan, DoctorLastFix, DoctorReport } from "./types";

const HOME = "C:\\Users\\Home Office";
const CLAUDE = `${HOME}\\.claude\\settings.json`;
const CODEX_HOOKS = "/home/rotem/.codex/hooks.json";
const NOW = 1_800_000_000_000;

const BEFORE = `{
  "model": "opus",
  "cleanupPeriodDays": 30,
  "hooks": {}
}
`;
const AFTER = `{
  "model": "opus",
  "cleanupPeriodDays": 365,
  "hooks": {
    "PermissionRequest": [{ "hooks": [{ "type": "command", "command": "marshell-hook permission --agent claude --tag mshl1" }] }]
  }
}
`;
const CODEX_BEFORE = "{}\n";
const CODEX_AFTER = `{ "hooks": { "PermissionRequest": [{ "hooks": [{ "type": "command", "command": "marshell-hook permission --agent codex --tag mshl1" }] }] } }\n`;

export const filePreview = (path: string, diff: string, deleted = false): FilePreview => ({
  path,
  scope: "user",
  applies: "newSessions",
  before_hash: "aaaa",
  after_hash: deleted ? undefined : "bbbb",
  diff,
  status: { kind: "changes" },
});

export const DOCTOR_PREVIEW: ConfigPreview = {
  changeset_id: "cs-1",
  title: "Fix 3 issues",
  reason: "Marshell's hooks and the transcript retention.",
  files: [filePreview(CLAUDE, unifiedDiff(CLAUDE, BEFORE, AFTER, 1)), filePreview(CODEX_HOOKS, unifiedDiff(CODEX_HOOKS, CODEX_BEFORE, CODEX_AFTER, 1))],
  stale: false,
  can_apply: true,
  snapshots_dir: "~/.marshell/snapshots/",
};

export const DOCTOR_PLAN: DoctorFixPlan = { preview: DOCTOR_PREVIEW, item_ids: ["hooks.claude", "hooks.codex@Ubuntu", "retention.claude"], restart: ["tab-1", "tab-2"] };

export const DOCTOR_REPORT: DoctorReport = {
  checked_at_ms: NOW - 12_000,
  duration_ms: 1_840,
  items: [
    { id: "hooks.claude", agent: "claude", group: "broken", title: "Approve from anywhere is not installed", detail: "Claude Code's permission requests cannot reach Marshell's cards, so they only show in the terminal.", fixable: true, path: CLAUDE, open_agents: false },
    { id: "hooks.codex@Ubuntu", agent: "codex", distro: "Ubuntu", group: "broken", title: "Codex in Ubuntu has no Marshell hooks", detail: "Codex's requests in this distro only show in the terminal.", fixable: true, path: CODEX_HOOKS, open_agents: false },
    { id: "hooks.antigravity", agent: "antigravity", group: "broken", title: "Antigravity's settings file does not parse", detail: `${HOME}\\.gemini\\antigravity-cli\\settings.json`, fixable: false, cannot_fix: "The file does not parse (line 14). Marshell never edits a file it can't read.", open_agents: true },
    { id: "retention.claude", agent: "claude", group: "info", title: "Claude deletes transcripts after 30 days", detail: "Marshell keeps its own copy, but raising this to 365 days keeps Claude's own resume working.", fixable: true, path: CLAUDE, open_agents: false },
    { id: "managed.claude", agent: "claude", group: "info", title: "Managed settings apply on top of yours", detail: "Your organisation manages some Claude settings in C:\\Program Files\\ClaudeCode\\managed-settings.json. Marshell lists them and never changes them.", fixable: false, open_agents: false },
  ],
};

export const DOCTOR_PROBLEMS: DoctorReport = {
  checked_at_ms: NOW - 30_000,
  duration_ms: 2_100,
  items: [
    { id: "hooks.codex-untrusted", agent: "codex", group: "broken", title: "Codex has not trusted Marshell's hooks", detail: "Hooks added to Codex do not run until they are trusted by hash.", fixable: false, cannot_fix: "Marshell can't trust hooks for you. Open /hooks in Codex and trust them.", open_agents: false },
    { id: "hooks.antigravity", agent: "antigravity", group: "broken", title: "Antigravity's settings file does not parse", detail: `${HOME}\\.gemini\\antigravity-cli\\settings.json`, fixable: false, cannot_fix: "The file does not parse (line 14). Marshell never edits a file it can't read.", open_agents: true },
  ],
};

/** After "Run CLI doctors": a Codex fail, a Claude warning and a CLI with no doctor command, all report-only, in the core's copy. */
export const DOCTOR_CLI: DoctorReport = {
  checked_at_ms: NOW - 12_000,
  cli_checked_at_ms: NOW - 20_000,
  duration_ms: 1_840,
  items: [
    { id: "cli.codex.sandbox.helpers", agent: "codex", group: "broken", title: "Codex's doctor: Sandbox helpers are missing", detail: "Check sandbox.helpers, found by codex doctor. Marshell reports it and doesn't change it.", fixable: false, cannot_fix: "Reinstall Codex to restore the sandbox helpers.", open_agents: false },
    { id: "cli.claude.5f1c2a9e", agent: "claude", group: "info", title: "Claude Code's doctor: Multiple installations found", detail: "Found by claude doctor. Marshell reports it and doesn't change it.", fixable: false, cannot_fix: "Fix: Remove the npm installation with npm uninstall -g @anthropic-ai/claude-code", open_agents: false },
    { id: "cli.qwen", agent: "qwen", group: "info", title: "Qwen Code has no doctor command.", detail: "Marshell's own checks still cover its hooks and settings.", fixable: false, open_agents: false },
  ],
};

/** After "Measure MCP servers": a server's cost as an Info item and one that didn't start as an unchecked Broken item. */
export const DOCTOR_MEASURED: DoctorReport = {
  checked_at_ms: NOW - 12_000,
  measured_at_ms: NOW - 20_000,
  duration_ms: 1_840,
  items: [
    { id: "mcpfail.claude.docs", agent: "claude", group: "broken", title: "The MCP server docs didn't start", detail: "Its command wasn't found. Turn it off to stop Claude Code from trying.", fixable: true, unchecked: true, open_agents: false },
    { id: "mcpcost.claude", agent: "claude", group: "info", title: "Claude Code's MCP servers add about 12,000 tokens to every session", detail: "Measured by starting each local server for a moment.", fixable: false, open_agents: false },
  ],
};

export const DOCTOR_CLEAR: DoctorReport = { checked_at_ms: NOW - 4_000, duration_ms: 900, items: [] };

export const UNDO_FIXED: DoctorLastFix = { snapshot_id: "snap-1", fixed_at_ms: NOW, item_ids: ["hooks.claude", "hooks.codex@Ubuntu", "retention.claude"], count: 3, restart: ["tab-1", "tab-2"], files_match: true };
