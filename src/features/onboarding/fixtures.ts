import { unifiedDiff } from "../../lib/diff";
import { CAPABILITIES, CLI_NAME, removalCopy, type Capability, type CliId, type ConsentPlan, type Detection } from "./consent";

const HOME = "C:\\Users\\Home Office";
const CLAUDE_PATH = `${HOME}\\.claude\\settings.json`;
const CODEX_PATH = `${HOME}\\.codex\\config.toml`;
const ANTIGRAVITY_PATH = `${HOME}\\.gemini\\antigravity-cli\\settings.json`;
const MANAGED = { path: "C:\\Program Files\\ClaudeCode\\managed-settings.json" };

const claude: Detection = { cli: "claude", installed: true, version: "2.1.296", exePath: `${HOME}\\.local\\bin\\claude.exe`, configPath: CLAUDE_PATH, configState: "ok" };
const codex: Detection = { cli: "codex", installed: true, version: "0.9.3", exePath: `${HOME}\\AppData\\Roaming\\npm\\codex.cmd`, configPath: CODEX_PATH, configState: "ok" };
// agy's installer puts it in %LOCALAPPDATA%\agy\bin (antigravity.google/docs/getting-started?tab=cli). The version is a stand-in until the CLI is installed (docs/PLAN.md: before phase 6).
const antigravity: Detection = { cli: "antigravity", installed: true, version: "1.0.0", exePath: `${HOME}\\AppData\\Local\\agy\\bin\\agy.exe`, configPath: ANTIGRAVITY_PATH, configState: "ok" };
const absent = (d: Detection): Detection => ({ ...d, installed: false, version: undefined, exePath: undefined, configState: "missing" });

/** The scenarios the stories show. A list is what detection returns: one entry per CLI Marshell knows. */
export const DETECTIONS: Record<"allThree" | "onlyClaude" | "none" | "unparsable" | "readOnly" | "managedPresent", Detection[]> = {
  allThree: [claude, codex, antigravity],
  onlyClaude: [claude, absent(codex), absent(antigravity)],
  none: [absent(claude), absent(codex), absent(antigravity)],
  unparsable: [claude, codex, { ...antigravity, configState: "unparsable", parseError: { line: 14, message: "Unexpected token }" } }],
  readOnly: [claude, { ...codex, configState: "read-only" }, antigravity],
  managedPresent: [{ ...claude, managed: MANAGED }, codex, antigravity],
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
// Codex hooks are event-nested TOML (docs/research/codex-cli.md, "Hook sources, trust, and config safety"): `[[hooks.<Event>]]` is a
// matcher group and `[[hooks.<Event>.hooks]]` its handler. No matcher: Marshell wants every source and every tool. Timeouts are
// seconds, explicit and short: 10 for telemetry, 120 for the permission decision (a person is in the loop), 3 for SessionEnd (Codex
// caps it there). The bridge path has a space, so the command quotes it inside the TOML literal string (itself in single quotes).
const CODEX_TIMEOUT: Record<string, number> = { PermissionRequest: 120, SessionEnd: 3 };
const codexEntry = (event: string, role: "event" | "permission") =>
  `\n[[hooks.${event}]]\n[[hooks.${event}.hooks]]\ntype = "command"\ncommand = '${BRIDGE} ${role} --agent codex --tag mshl1'\ntimeout = ${CODEX_TIMEOUT[event] ?? 10}\n`;
// Exactly the seven documented events the card promises, in lifecycle order: six status events and the permission request.
const CODEX_HOOKS: Array<[event: string, role: "event" | "permission"]> = [
  ["SessionStart", "event"],
  ["UserPromptSubmit", "event"],
  ["PreToolUse", "event"],
  ["PermissionRequest", "permission"],
  ["PostToolUse", "event"],
  ["Stop", "event"],
  ["SessionEnd", "event"],
];
const codexAfter = codexBefore + CODEX_HOOKS.map(([e, role]) => codexEntry(e, role)).join("");
// A stand-in, made long on purpose to show how a long change folds; its card says so in its own headline.
const STAND_IN = 40;
const codexStandIn = codexBefore + Array.from({ length: STAND_IN }, (_, i) => codexEntry(`StandIn${i + 1}`, "event")).join("");

// Antigravity: the only verified change is its status line (antigravity.google/docs/cli/statusline: `statusLine` is
// `{ "type": "command", "command": ... }` in ~/.gemini/antigravity-cli/settings.json). The user's own command is the one the
// docs show; Marshell puts the bridge in front of it and keeps it in the chain file. No hooks: their events are unverified.
const antigravityLine = "~/.gemini/antigravity-cli/statusline.sh";
const antigravityBefore = `{
  "statusLine": { "type": "command", "command": "${antigravityLine}" }
}
`;
const antigravityAfter = `{
  "statusLine": { "type": "command", "command": "${BRIDGE.replace(/"/g, '\\"')} statusline --agent antigravity --tag mshl1" }
}
`;

/** Where the user's own status line command is kept when Marshell chains it: one file per CLI, named on the card and in the diff. */
const chainPath = (cli: CliId) => `${HOME}\\.marshell\\agents\\${cli}\\statusline-chain.json`;
// The user's own status line is saved verbatim in a new file, so the diff shows it is kept and where.
const chainFile = '{ "type": "command", "command": "python ~/.claude/statusline.py" }\n';
const antigravityChainFile = `{ "type": "command", "command": "${antigravityLine}" }\n`;
const newFile = (path: string, text: string) => unifiedDiff(path, "", text).replace(/^--- a\/.*$/m, "--- /dev/null");

/** The exact change each CLI's consent card shows under "View exact changes". */
export const CONSENT_DIFFS = {
  claude: unifiedDiff(CLAUDE_PATH, claudeBefore, claudeAfter) + newFile(chainPath("claude"), chainFile),
  codex: unifiedDiff(CODEX_PATH, codexBefore, codexAfter),
  codexLong: unifiedDiff(CODEX_PATH, codexBefore, codexStandIn),
  antigravity: unifiedDiff(ANTIGRAVITY_PATH, antigravityBefore, antigravityAfter) + newFile(chainPath("antigravity"), antigravityChainFile),
};

const ALL_ON: Record<Capability, boolean> = { status: true, approve: true, statusline: true };
/** Antigravity's card: only the status line is verified, so only that switch starts on. */
const STATUSLINE_ONLY: Record<Capability, boolean> = { status: false, approve: false, statusline: true };
const plan = (d: Detection, diff: string, on: Record<Capability, boolean> = ALL_ON): ConsentPlan => ({
  cli: d.cli,
  on,
  configPath: d.configPath,
  backupPath: `${HOME}\\.marshell\\snapshots\\2026-10-10T20-15\\${CLI_NAME[d.cli].toLowerCase().replace(/ /g, "-")}-${d.configPath.split("\\").pop()}`,
  removal: removalCopy(d.cli, on),
  diff,
  needsTrust: d.cli === "codex" && (on.status || on.approve),
  chainPath: on.statusline && CAPABILITIES[d.cli].statusline.available ? chainPath(d.cli) : undefined,
});

/** A consent card's input: what was found, and what Marshell proposes. */
export const CONSENT: Record<"claude" | "codex" | "codexLong" | "antigravity" | "unparsableAntigravity" | "readOnlyCodex" | "claudeManaged", { detection: Detection; plan: ConsentPlan }> = {
  claude: { detection: claude, plan: plan(claude, CONSENT_DIFFS.claude) },
  codex: { detection: codex, plan: plan(codex, CONSENT_DIFFS.codex) },
  codexLong: { detection: codex, plan: { ...plan(codex, CONSENT_DIFFS.codexLong), headline: `Add ${STAND_IN} hook entries in ${CODEX_PATH}. This is a stand-in change, made long on purpose to show how a long change folds.` } },
  antigravity: { detection: antigravity, plan: plan(antigravity, CONSENT_DIFFS.antigravity, STATUSLINE_ONLY) },
  readOnlyCodex: { detection: DETECTIONS.readOnly[1]!, plan: plan(DETECTIONS.readOnly[1]!, "") },
  unparsableAntigravity: { detection: DETECTIONS.unparsable[2]!, plan: plan(DETECTIONS.unparsable[2]!, "") },
  claudeManaged: { detection: { ...claude, managed: MANAGED }, plan: plan({ ...claude, managed: MANAGED }, CONSENT_DIFFS.claude) },
};
