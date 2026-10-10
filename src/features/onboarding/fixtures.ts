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
const CODEX_EVENTS = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Stop", "SessionEnd"];
// The bridge path has a space, so the command quotes it inside the TOML literal string (which is itself in single quotes).
const codexEntry = (event: string, role: "event" | "permission") => `\n[[hooks]]\nevent = "${event}"\ncommand = '${BRIDGE} ${role} --agent codex --tag mshl1'\ntimeout = ${role === "permission" ? 600 : 30}\n`;
// Exactly the seven entries the card promises: six status events and the permission request.
const codexAfter = codexBefore + CODEX_EVENTS.map((e) => codexEntry(e, "event")).join("") + codexEntry("PermissionRequest", "permission");
// A stand-in, made long on purpose to show how a long change folds; its card says so in its own headline.
const STAND_IN = 40;
const codexStandIn = codexBefore + Array.from({ length: STAND_IN }, (_, i) => codexEntry(`StandIn${i + 1}`, "event")).join("");

const geminiBefore = '{\n  "theme": "dark"\n}\n';
const geminiAfter = `{
  "theme": "dark",
  "hooksConfig": { "enabled": true },
  "hooks": {
${EVENTS.slice(0, 7).map((e) => `    "${e}": [{ "hooks": [{ "type": "command", "command": "${BRIDGE.replace(/"/g, '\\"')} event --agent gemini --tag mshl1" }] }]`).join(",\n")}
  }
}
`;

const CHAIN_PATH = `${HOME}\\.marshell\\agents\\claude\\statusline-chain.json`;
// The user's own status line is saved verbatim in a new file, so the diff shows it is kept and where.
const chainFile = '{ "type": "command", "command": "python ~/.claude/statusline.py" }\n';
const newFile = (path: string, text: string) => unifiedDiff(path, "", text).replace(/^--- a\/.*$/m, "--- /dev/null");

/** The exact change each CLI's consent card shows under "View exact changes". */
export const CONSENT_DIFFS = {
  claude: unifiedDiff(CLAUDE_PATH, claudeBefore, claudeAfter) + newFile(CHAIN_PATH, chainFile),
  codex: unifiedDiff(CODEX_PATH, codexBefore, codexAfter),
  codexLong: unifiedDiff(CODEX_PATH, codexBefore, codexStandIn),
  gemini: unifiedDiff(GEMINI_PATH, geminiBefore, geminiAfter),
};

const ALL_ON: Record<Capability, boolean> = { status: true, approve: true, statusline: true };
const REMOVAL = "turning Marshell's hooks off, or uninstalling, removes only the entries Marshell added; the rest of the file stays as it is.";
const plan = (d: Detection, diff: string, on: Record<Capability, boolean> = ALL_ON): ConsentPlan => ({
  cli: d.cli,
  on,
  configPath: d.configPath,
  backupPath: `${HOME}\\.marshell\\snapshots\\2026-10-10T20-15\\${CLI_NAME[d.cli].toLowerCase().replace(/ /g, "-")}-${d.configPath.split("\\").pop()}`,
  removal: REMOVAL,
  diff,
  needsTrust: d.cli === "codex" && (on.status || on.approve),
  chainPath: d.cli === "claude" && on.statusline ? CHAIN_PATH : undefined,
});

/** A consent card's input: what was found, and what Marshell proposes. */
export const CONSENT: Record<"claude" | "codex" | "codexLong" | "gemini" | "unparsableGemini" | "readOnlyCodex" | "claudeManaged", { detection: Detection; plan: ConsentPlan }> = {
  claude: { detection: claude, plan: plan(claude, CONSENT_DIFFS.claude) },
  codex: { detection: codex, plan: plan(codex, CONSENT_DIFFS.codex) },
  codexLong: { detection: codex, plan: { ...plan(codex, CONSENT_DIFFS.codexLong), headline: `Add ${STAND_IN} hook entries in ${CODEX_PATH}. This is a stand-in change, made long on purpose to show how a long change folds.` } },
  gemini: { detection: gemini, plan: plan(gemini, CONSENT_DIFFS.gemini, { status: true, approve: false, statusline: false }) },
  readOnlyCodex: { detection: DETECTIONS.readOnly[1]!, plan: plan(DETECTIONS.readOnly[1]!, "") },
  unparsableGemini: { detection: DETECTIONS.unparsable[2]!, plan: plan(DETECTIONS.unparsable[2]!, "") },
  claudeManaged: { detection: { ...claude, managed: MANAGED }, plan: plan({ ...claude, managed: MANAGED }, CONSENT_DIFFS.claude) },
};
