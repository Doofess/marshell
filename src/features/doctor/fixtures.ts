import { unifiedDiff } from "../../lib/diff";
import type { DoctorItem, DoctorReport } from "./types";
import type { UndoState } from "./undo";

const HOME = "C:\\Users\\Home Office";
const CLAUDE = `${HOME}\\.claude\\settings.json`;
const NOW = 1_800_000_000_000;

// One settings file; each fix is a change to it, diffed against the file as it is now (the sheet merges them into one diff).
const FILE = `{
  "model": "opus",
  "statusLine": { "type": "command", "command": "python ~/.claude/statusline.py" },
  "env": {
    "EDITOR": "code"
  },
  "permissions": {
    "allow": ["Bash(npm test:*)"]
  },
  "cleanupPeriodDays": 30,
  "includeCoAuthoredBy": true,
  "theme": "dark",
  "notifications": { "desktop": true },
  "verbose": false,
  "autoUpdates": true,
  "editorMode": "normal",
  "hooks": {}
}
`;
// Inside the JSON file the command's own quotes are written \"...\" (a quoted path, because the home folder has a space).
const HOOK = String.raw`\"C:/Users/Home Office/.marshell/bin/marshell-hook.exe\"`;
// The user's own status line command is kept verbatim in a new file next to Marshell's other data, so the fix shows it is not lost.
const CHAIN_FILE = `${HOME}\\.marshell\\agents\\claude\\statusline-chain.json`;
const CHAIN_BODY = '{ "type": "command", "command": "python ~/.claude/statusline.py" }\n';
const chainKept = { file: CHAIN_FILE, diff: unifiedDiff(CHAIN_FILE, "", CHAIN_BODY).replace(/^--- a\/.*$/m, "--- /dev/null") };
const hooksAfter = FILE.replace('"hooks": {}', `"hooks": {
    "PermissionRequest": [{ "hooks": [{ "type": "command", "command": "${HOOK} permission --agent claude --tag mshl1" }] }]
  }`);
const chainAfter = FILE.replace('"python ~/.claude/statusline.py"', `"${HOOK} statusline --agent claude --tag mshl1"`);
const cleanupAfter = FILE.replace('"cleanupPeriodDays": 30', '"cleanupPeriodDays": 365');
// Illustrative until phase 7 verifies the real keys against each CLI's docs.
const dupAfter = FILE.replace('"desktop": true', '"desktop": false');

type FixableInput = Omit<DoctorItem, "fixable" | "file" | "diff" | "severity"> & { after: string; severity?: DoctorItem["severity"] };
const fixable = (i: FixableInput): DoctorItem => {
  const { after, severity = "warning", ...rest } = i;
  return { ...rest, severity, fixable: true, file: CLAUDE, diff: unifiedDiff(CLAUDE, FILE, after, 1) };
};

export const DOCTOR_REPORT: DoctorReport = {
  checkedAtMs: NOW - 12_000,
  durationMs: 1_840,
  items: [
    fixable({ id: "claude-permission-hook", cli: "claude", title: "Approve from anywhere is not installed", detail: "Claude Code's permission requests cannot reach Marshell's cards, so they only show in the terminal.", severity: "error", restartSessions: [3], after: hooksAfter }),
    fixable({ id: "claude-statusline-chain", cli: "claude", title: "Your status line is no longer chained", detail: "Something replaced the status line command. Marshell puts itself in front and keeps yours running.", restartSessions: [3, 5], after: chainAfter, alsoWrites: [chainKept] }),
    fixable({ id: "claude-cleanup-period", cli: "claude", title: "Claude deletes transcripts after 30 days", detail: "Marshell keeps its own copy, but raising this to 365 days keeps Claude's own resume working.", after: cleanupAfter }),
    fixable({ id: "duplicate-notifications", cli: "marshell", title: "Two notifications for one request", detail: "Claude Code and Marshell both show a banner when a request arrives.", after: dupAfter }),
    {
      id: "managed-present",
      cli: "claude",
      title: "Managed settings apply on top of yours",
      detail: "Your organisation manages some Claude settings in C:\\Program Files\\ClaudeCode\\managed-settings.json. Marshell lists them and never changes them.",
      severity: "info",
      fixable: false,
    },
  ],
};

export const DOCTOR_PROBLEMS: DoctorReport = {
  checkedAtMs: NOW - 30_000,
  durationMs: 2_100,
  items: [
    { id: "codex-hooks-untrusted", cli: "codex", title: "Codex has not trusted Marshell's hooks", detail: "Hooks added to Codex do not run until they are trusted by hash.", severity: "warning", fixable: false, unfixableReason: "Marshell can't trust hooks for you. Open /hooks in Codex and trust them." },
    { id: "gemini-unparsable", cli: "gemini", title: "Gemini's settings file does not parse", detail: `${HOME}\\.gemini\\settings.json`, severity: "error", fixable: false, unfixableReason: "The file does not parse (line 14). Marshell never edits a file it can't read." },
    { id: "codex-readonly", cli: "codex", title: "Codex's config file is read-only", detail: `${HOME}\\.codex\\config.toml`, severity: "warning", fixable: false, unfixableReason: "The file is read-only." },
  ],
};

export const DOCTOR_CLEAR: DoctorReport = { checkedAtMs: NOW - 4_000, durationMs: 900, items: [] };

export const UNDO_FIXED: UndoState = { fixedAtMs: NOW, count: 4, filesMatch: true };
