import { unifiedDiff } from "../../lib/diff";
import { CLI_NAME, type Capability, type ConsentPlan, type Detection } from "./consent";

const HOME = "C:\\Users\\Home Office";
const CLAUDE_PATH = `${HOME}\\.claude\\settings.json`;
const CODEX_PATH = `${HOME}\\.codex\\config.toml`;
const GEMINI_PATH = `${HOME}\\.gemini\\settings.json`;
const MANAGED = { path: "C:\\Program Files\\ClaudeCode\\managed-settings.json" };

const claude: Detection = { cli: "claude", installed: true, version: "2.1.296", exePath: `${HOME}\\.local\\bin\\claude.exe`, configPath: CLAUDE_PATH, configState: "ok" };
const codex: Detection = { cli: "codex", installed: true, version: "0.9.3", exePath: `${HOME}\\AppData\\Roaming\\npm\\codex.cmd`, configPath: CODEX_PATH, configState: "ok" };
const gemini: Detection = { cli: "gemini", installed: true, version: "0.4.2", exePath: `${HOME}\\AppData\\Roaming\\npm\\gemini.cmd`, configPath: GEMINI_PATH, configState: "ok" };
const absent = (d: Detection): Detection => ({ ...d, installed: false, version: undefined, exePath: undefined, configState: "missing" });

/** The scenarios the stories show. A list is what detection returns: one entry per CLI Marshell knows. */
export const DETECTIONS: Record<"allThree" | "onlyClaude" | "none" | "unparsable" | "readOnly" | "managedPresent", Detection[]> = {
  allThree: [claude, codex, gemini],
  onlyClaude: [claude, absent(codex), absent(gemini)],
  none: [absent(claude), absent(codex), absent(gemini)],
  unparsable: [claude, codex, { ...gemini, configState: "unparsable", parseError: { line: 14, message: "Unexpected token }" } }],
  readOnly: [claude, { ...codex, configState: "read-only" }, gemini],
  managedPresent: [{ ...claude, managed: MANAGED }, codex, gemini],
};

const BRIDGE = '"C:/Users/Home Office/.marshell/bin/marshell-hook.exe"';
const EVENTS = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Notification", "SubagentStop", "Stop", "SessionEnd"];

const claudeBefore = `{
  "model": "opus",
  "cleanupPeriodDays": 30,
  "statusLine": { "type": "command", "command": "python ~/.claude/statusline.py" },
  "theme": "dark"
}
`;
const claudeAfter = `{
  "model": "opus",
  "cleanupPeriodDays": 30,
  "statusLine": { "type": "command", "command": "${BRIDGE.replace(/"/g, '\\"')} statusline --agent claude --tag mshl1" },
  "hooks": {
${EVENTS.map((e) => `    "${e}": [{ "hooks": [{ "type": "command", "command": "${BRIDGE.replace(/"/g, '\\"')} event --agent claude --tag mshl1" }] }],`).join("\n")}
    "PermissionRequest": [{ "hooks": [{ "type": "command", "command": "${BRIDGE.replace(/"/g, '\\"')} permission --agent claude --tag mshl1", "timeout": 600 }] }]
  },
  "theme": "dark"
}
`;

const codexBefore = 'model = "gpt-5"\napproval_policy = "on-request"\n';
const codexAfter =
  codexBefore +
  Array.from({ length: 40 }, (_, i) => `\n[[hooks]]\nevent = "Event${i}"\ncommand = '${BRIDGE.replace(/"/g, "")} event --agent codex --tag mshl1'\ntimeout = 30\n`).join("");

const geminiBefore = '{\n  "theme": "dark"\n}\n';
const geminiAfter = `{
  "theme": "dark",
  "hooksConfig": { "enabled": true },
  "hooks": {
${EVENTS.slice(0, 7).map((e) => `    "${e}": [{ "hooks": [{ "type": "command", "command": "${BRIDGE.replace(/"/g, '\\"')} event --agent gemini --tag mshl1" }] }]`).join(",\n")}
  }
}
`;

/** The exact change each CLI's consent card shows under "View exact changes". */
export const CONSENT_DIFFS = {
  claude: unifiedDiff(CLAUDE_PATH, claudeBefore, claudeAfter),
  codex: unifiedDiff(CODEX_PATH, codexBefore, codexAfter.slice(0, codexBefore.length + 600)),
  codexLong: unifiedDiff(CODEX_PATH, codexBefore, codexAfter),
  gemini: unifiedDiff(GEMINI_PATH, geminiBefore, geminiAfter),
};

const ALL_ON: Record<Capability, boolean> = { status: true, approve: true, statusline: true };
const REMOVAL = "Settings → Advanced → Remove all hooks restores the file byte for byte.";
const plan = (d: Detection, diff: string, on: Record<Capability, boolean> = ALL_ON): ConsentPlan => ({
  cli: d.cli,
  on,
  configPath: d.configPath,
  backupPath: `${HOME}\\.marshell\\snapshots\\2026-10-10T20-15\\${CLI_NAME[d.cli].toLowerCase().replace(/ /g, "-")}-${d.configPath.split("\\").pop()}`,
  removal: REMOVAL,
  diff,
  needsTrust: d.cli === "codex" && (on.status || on.approve),
});

/** A consent card's input: what was found, and what Marshell proposes. */
export const CONSENT: Record<"claude" | "codex" | "gemini" | "unparsableGemini" | "claudeManaged", { detection: Detection; plan: ConsentPlan }> = {
  claude: { detection: claude, plan: plan(claude, CONSENT_DIFFS.claude) },
  codex: { detection: codex, plan: plan(codex, CONSENT_DIFFS.codexLong) },
  gemini: { detection: gemini, plan: plan(gemini, CONSENT_DIFFS.gemini, { status: true, approve: false, statusline: false }) },
  unparsableGemini: { detection: DETECTIONS.unparsable[2]!, plan: plan(DETECTIONS.unparsable[2]!, "") },
  claudeManaged: { detection: { ...claude, managed: MANAGED }, plan: plan({ ...claude, managed: MANAGED }, CONSENT_DIFFS.claude) },
};
