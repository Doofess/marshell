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
    { id: "cli.copilot", agent: "copilot", group: "info", title: "Copilot CLI has no doctor command.", detail: "Marshell's own checks still cover its hooks and settings.", fixable: false, open_agents: false },
  ],
};

/** The two rows that carry a button: a Copilot hooks file that isn't Marshell's any more, and one the person kept. */
export const DOCTOR_KEEP: DoctorReport = {
  checked_at_ms: NOW - 12_000,
  duration_ms: 1_100,
  items: [
    { id: "hooksfile.copilot", agent: "copilot", group: "broken", title: "C:\\Users\\you\\.copilot\\hooks\\marshell.json isn't Marshell's file any more", detail: "Marshell set Copilot CLI's hooks up in it, and it now holds something Marshell didn't write. Keep it as it is, or pick the fix to replace it with Marshell's hooks.", fixable: true, unchecked: true, open_agents: false, action: "keep" },
    { id: "hookskept.copilot", agent: "copilot", group: "info", title: "Copilot CLI's hooks file has your changes", detail: "You kept C:\\Users\\you\\.copilot\\hooks\\marshell.json as it is. Marshell changes only its own hooks in it.", fixable: false, open_agents: false, action: "stopKeeping" },
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

/** One project's instruction line with the files two CLIs load, and a row with none, in the core's copy. */
export const DOCTOR_LOADS: DoctorReport = {
  checked_at_ms: NOW - 12_000,
  duration_ms: 1_200,
  items: [
    {
      id: "instr.3f2a9c1e",
      group: "info",
      title: "Airport: at start, Claude Code loads about 2.4k tokens of instructions, Codex 600.",
      detail: "Estimates: characters divided by 4. Files that load later aren't counted.",
      fixable: false,
      open_agents: false,
      loads: [
        {
          agent: "claude",
          heading: "Claude Code: about 2.4k tokens at start",
          files: [
            { path: "CLAUDE.md", when: "start", note: "about 1.8k tokens", depth: 0 },
            { path: "AGENTS.md", when: "start", note: "about 600 tokens, imported by CLAUDE.md", depth: 1 },
            { path: "docs/CLAUDE.md", when: "whenUsed", note: "about 300 tokens, loads when used", depth: 0 },
          ],
        },
        {
          agent: "codex",
          heading: "Codex: about 600 tokens at start",
          notes: ["Marshell stopped looking for files in subfolders after 10,000 entries."],
          files: [
            { path: "AGENTS.override.md", when: "start", note: "about 600 tokens", depth: 0 },
            { path: "AGENTS.md", when: "notLoaded", note: "not loaded: AGENTS.override.md is here", depth: 0 },
          ],
        },
      ],
    },
    { id: "cli.qwen", agent: "qwen", group: "info", title: "Qwen Code has no doctor command.", detail: "Marshell's own checks still cover its hooks and settings.", fixable: false, open_agents: false },
  ],
};
/** One finding of each kind the instruction checks make, in the core's copy: a row names the files it can open. */
export const DOCTOR_FINDINGS: DoctorReport = {
  checked_at_ms: NOW - 12_000,
  duration_ms: 1_400,
  items: [
    {
      id: "instrimport.3f2a9c1e.0a1b2c3d.4e5f6a7b.at",
      group: "broken",
      title: "CLAUDE.md in Airport imports a file that doesn't exist",
      detail: "Line 12: @docs/setup.md. Claude Code loads nothing for it. Copilot CLI leaves it in as text.",
      fixable: true,
      unchecked: true,
      open_agents: false,
      opens: [{ path: "C:\\dev\\Airport\\CLAUDE.md", line: 12, label: "Open CLAUDE.md" }],
    },
    {
      id: "instrcut.codex.3f2a9c1e.budget",
      agent: "codex",
      group: "broken",
      title: "Codex cuts its project instructions in Airport",
      detail: "It loads 32768 bytes of project instructions, and these are over: AGENTS.md. The fix sets project_doc_max_bytes to 40960, for every project. Codex then loads about 1.8k more tokens at each start here.",
      fixable: true,
      unchecked: true,
      open_agents: false,
    },
    {
      id: "refnotes.long.3f2a9c1e",
      group: "waste",
      title: "Reference notes in Airport average about 210 tokens",
      detail: "A note this long names several aliases at once.",
      fixable: false,
      cannot_fix: "Shorten or remove aliases in Settings, Projects.",
      open_agents: false,
    },
    {
      id: "instrstale.3f2a9c1e.9c8d7e6f",
      group: "info",
      title: "AGENTS.md in Airport names 2 paths that don't exist",
      detail: "Line 14: src/old/api.ts. Line 30: docs/setup.md.",
      fixable: false,
      open_agents: false,
      opens: [{ path: "C:\\dev\\Airport\\AGENTS.md", line: 14, label: "Open AGENTS.md" }],
    },
    {
      id: "instrtwice.copilot.3f2a9c1e.9c8d7e6f",
      agent: "copilot",
      group: "info",
      title: "Copilot CLI loads AGENTS.md twice in Airport",
      detail: "Once by its own name and once as an import of CLAUDE.md. About 250 tokens are sent twice.",
      fixable: false,
      open_agents: false,
      opens: [{ path: "C:\\dev\\Airport\\AGENTS.md", label: "Open AGENTS.md" }],
    },
    {
      id: "instrrepeat.3f2a9c1e.0a1b2c3d.9c8d7e6f",
      group: "info",
      title: "CLAUDE.md and AGENTS.md in Airport repeat 3 paragraphs",
      detail: "Claude Code and Copilot CLI load both. The first is at CLAUDE.md:12 and AGENTS.md:30: “Use pnpm for the frontend and Cargo for Rust; both versions are pinned.”. About 420 tokens are sent twice.",
      fixable: false,
      open_agents: false,
      opens: [
        { path: "C:\\dev\\Airport\\CLAUDE.md", line: 12, label: "Open CLAUDE.md" },
        { path: "C:\\dev\\Airport\\AGENTS.md", line: 30, label: "Open AGENTS.md" },
      ],
    },
    {
      id: "instrclash.3f2a9c1e.manager",
      group: "info",
      title: "Two package managers are named in Airport",
      detail: "~/.claude/CLAUDE.md:4 (user level) says npm: “Always run npm install before a commit.”. AGENTS.md:12 says pnpm: “Use pnpm for the frontend.”. Claude Code loads both.",
      fixable: false,
      open_agents: false,
      opens: [
        { path: "C:\\Users\\dev\\.claude\\CLAUDE.md", line: 4, label: "Open CLAUDE.md" },
        { path: "C:\\dev\\Airport\\AGENTS.md", line: 12, label: "Open AGENTS.md" },
      ],
    },
    {
      id: "refnotes.3f2a9c1e",
      group: "info",
      title: "Airport: 14 reference notes in the last 30 days, about 1.1k tokens",
      detail: "About 80 tokens a note. Marshell adds a note to a prompt that names an alias.",
      fixable: false,
      open_agents: false,
    },
  ],
};
export const DOCTOR_CLEAR: DoctorReport = { checked_at_ms: NOW - 4_000, duration_ms: 900, items: [] };

export const UNDO_FIXED: DoctorLastFix = { snapshot_id: "snap-1", fixed_at_ms: NOW, item_ids: ["hooks.claude", "hooks.codex@Ubuntu", "retention.claude"], count: 3, restart: ["tab-1", "tab-2"], files_match: true };
