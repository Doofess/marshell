import { unifiedDiff } from "../../lib/diff";
import { CAPABILITIES, CLI_NAME, fileName, removalCopy, type Capability, type CliId, type ConsentPlan, type Detection } from "./consent";

const HOME = "C:\\Users\\Home Office";
const CLAUDE_PATH = `${HOME}\\.claude\\settings.json`;
// Marshell writes only Codex's hooks.json (never config.toml, where Codex keeps its trust).
const CODEX_PATH = `${HOME}\\.codex\\hooks.json`;
const ANTIGRAVITY_PATH = `${HOME}\\.gemini\\antigravity-cli\\settings.json`;
// Antigravity reads global hooks from the shared ~/.gemini/config/ folder, not from its settings (agy 1.3.3's bundled hooks doc).
const ANTIGRAVITY_HOOKS_PATH = `${HOME}\\.gemini\\config\\hooks.json`;
const MANAGED = { path: "C:\\Program Files\\ClaudeCode\\managed-settings.json" };

// Versions and paths as found on the dev machine on 2026-10-15 (docs/research/2026-10-15-local-install-check.md).
const claude: Detection = { cli: "claude", installed: true, version: "2.1.296", exePath: `${HOME}\\.local\\bin\\claude.exe`, configPath: CLAUDE_PATH, configState: "ok" };
const codex: Detection = { cli: "codex", installed: true, version: "0.162.1", exePath: `${HOME}\\AppData\\Local\\Programs\\OpenAI\\Codex\\bin\\codex.exe`, configPath: CODEX_PATH, configState: "ok" };
const antigravity: Detection = {
  cli: "antigravity",
  installed: true,
  version: "1.3.3",
  exePath: `${HOME}\\AppData\\Local\\agy\\bin\\agy.exe`,
  configPath: ANTIGRAVITY_PATH,
  configState: "ok",
  hooks: { path: ANTIGRAVITY_HOOKS_PATH, exists: false },
};
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
const EVENTS = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Notification", "SubagentStart", "SubagentStop", "Stop", "StopFailure", "SessionEnd"];

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

// Codex's hooks.json (docs/research/codex-cli.md, and the recording of Task 1): `{"hooks": {"<Event>": [{"hooks": [handler]}]}}`, with no
// matcher because Marshell wants every source and every tool. Timeouts are seconds: 5 for telemetry, 2 for SessionEnd and Interrupt (Codex
// caps them at 3), 600 for the permission decision (a person is in the loop). Codex runs a hook command through PowerShell on Windows,
// where a quoted path followed by arguments is a parse error, so the command is `& "<bridge>" ...`.
const CODEX_TIMEOUT: Record<string, number> = { PermissionRequest: 600, SessionEnd: 2, Interrupt: 2 };
const codexHandler = (event: string, role: "event" | "permission") => ({ type: "command", command: `& ${BRIDGE} ${role} --agent codex --tag mshl1`, timeout: CODEX_TIMEOUT[event] ?? 5 });
// The seven status events, then the permission request: the eight entries the card promises.
const CODEX_HOOKS: Array<[event: string, role: "event" | "permission"]> = [
  ["SessionStart", "event"],
  ["UserPromptSubmit", "event"],
  ["PreToolUse", "event"],
  ["PostToolUse", "event"],
  ["Stop", "event"],
  ["Interrupt", "event"],
  ["SessionEnd", "event"],
  ["PermissionRequest", "permission"],
];
const codexFile = (hooks: Array<[event: string, role: "event" | "permission"]>) =>
  `${JSON.stringify({ hooks: Object.fromEntries(hooks.map(([e, role]) => [e, [{ hooks: [codexHandler(e, role)] }]])) }, null, 2)}\n`;
const codexAfter = codexFile(CODEX_HOOKS);
// A stand-in, made long on purpose to show how a long change folds; its card says so in its own headline.
const STAND_IN = 40;
const codexStandIn = codexFile(Array.from({ length: STAND_IN }, (_, i): [string, "event"] => [`StandIn${i + 1}`, "event"]));

// Antigravity hooks (agy 1.3.3's bundled hooks doc, docs/research/2026-10-15-local-install-check.md): hooks.json holds named hooks,
// each with `enabled` and its events. PreToolUse and PostToolUse are matcher groups (a regex on the tool name; `.*` takes every
// tool), PreInvocation, PostInvocation and Stop are flat handler lists. The payload names no event, so each command says which
// one it is. Timeouts are seconds and default to 30: PreToolUse gets 600 because a person answers it (the bridge releases it
// earlier, answering "ask" so Antigravity's own prompt takes over); the rest are telemetry, capped at 10. Windows runs each
// command through `cmd /c`, and the quoted path with its space is the case spike S10 checks.
const ANTIGRAVITY_HOOKS: Array<[event: string, role: "event" | "permission", grouped: boolean]> = [
  ["PreToolUse", "permission", true],
  ["PostToolUse", "event", true],
  ["PreInvocation", "event", false],
  ["PostInvocation", "event", false],
  ["Stop", "event", false],
];
const agyHandler = (event: string, role: "event" | "permission") =>
  `{ "type": "command", "command": "${BRIDGE.replace(/"/g, '\\"')} ${role} --agent antigravity --event ${event} --tag mshl1", "timeout": ${role === "permission" ? 600 : 10} }`;
const antigravityHooks = `{
  "marshell": {
    "enabled": true,
${ANTIGRAVITY_HOOKS.map(([e, role, grouped]) => `    "${e}": [${grouped ? `{ "matcher": ".*", "hooks": [${agyHandler(e, role)}] }` : agyHandler(e, role)}]`).join(",\n")}
  }
}
`;

// Antigravity's status line (antigravity.google/docs/cli/statusline: `statusLine` is `{ "type": "command", "command": ... }` in
// ~/.gemini/antigravity-cli/settings.json). The user's own command is the one the docs show; Marshell puts the bridge in front
// of it and keeps it in the chain file.
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
// The dev machine's own settings skip every prompt; handing the mode to Marshell removes that one line (docs/PLAN.md "Session modes").
const antigravitySkipsBefore = antigravityBefore.replace(/^\{\n/, '{\n  "dangerouslySkipPermissions": true,\n');
if (antigravitySkipsBefore === antigravityBefore) throw new Error("antigravityBefore must start with an opening brace on its own line");
const antigravityChainFile = `{ "type": "command", "command": "${antigravityLine}" }\n`;
const newFile = (path: string, text: string) => unifiedDiff(path, "", text).replace(/^--- a\/.*$/m, "--- /dev/null");

/** The exact change each CLI's consent card shows under "View exact changes". */
export const CONSENT_DIFFS = {
  claude: unifiedDiff(CLAUDE_PATH, claudeBefore, claudeAfter) + newFile(chainPath("claude"), chainFile),
  codex: newFile(CODEX_PATH, codexAfter),
  codexLong: newFile(CODEX_PATH, codexStandIn),
  antigravity:
    newFile(ANTIGRAVITY_HOOKS_PATH, antigravityHooks) +
    unifiedDiff(ANTIGRAVITY_PATH, antigravityBefore, antigravityAfter) +
    newFile(chainPath("antigravity"), antigravityChainFile),
  antigravityHandOver:
    newFile(ANTIGRAVITY_HOOKS_PATH, antigravityHooks) +
    unifiedDiff(ANTIGRAVITY_PATH, antigravitySkipsBefore, antigravityAfter) +
    newFile(chainPath("antigravity"), antigravityChainFile),
};
const antigravitySkips: Detection = { ...antigravity, skipsAll: { setting: "dangerouslySkipPermissions" } };

const ALL_ON: Record<Capability, boolean> = { status: true, approve: true, statusline: true, skill: false };
const plan = (d: Detection, diff: string, on: Record<Capability, boolean> = ALL_ON, handOver = false): ConsentPlan => ({
  cli: d.cli,
  on,
  handOver: d.skipsAll ? handOver : undefined,
  configPath: d.configPath,
  backupPath: `${HOME}\\.marshell\\snapshots\\2026-10-10T20-15\\${CLI_NAME[d.cli].toLowerCase().replace(/ /g, "-")}-${fileName(d.configPath)}`,
  removal: removalCopy(d.cli, on, d.hooks && { hooksFile: fileName(d.hooks.path), created: !d.hooks.exists }, handOver ? d.skipsAll?.setting : undefined),
  diff,
  needsTrust: d.cli === "codex" && (on.status || on.approve),
  chainPath: on.statusline && CAPABILITIES[d.cli].statusline.available ? chainPath(d.cli) : undefined,
});

/** A consent card's input: what was found, and what Marshell proposes. */
export const CONSENT: Record<"claude" | "codex" | "codexLong" | "antigravity" | "antigravityHandOver" | "unparsableAntigravity" | "readOnlyCodex" | "claudeManaged", { detection: Detection; plan: ConsentPlan }> = {
  antigravityHandOver: { detection: antigravitySkips, plan: plan(antigravitySkips, CONSENT_DIFFS.antigravityHandOver, ALL_ON, true) },
  claude: { detection: claude, plan: plan(claude, CONSENT_DIFFS.claude) },
  codex: { detection: codex, plan: plan(codex, CONSENT_DIFFS.codex) },
  codexLong: { detection: codex, plan: { ...plan(codex, CONSENT_DIFFS.codexLong), headline: `Add ${STAND_IN} hook entries in ${CODEX_PATH}. This is a stand-in change, made long on purpose to show how a long change folds.` } },
  antigravity: { detection: antigravity, plan: plan(antigravity, CONSENT_DIFFS.antigravity) },
  readOnlyCodex: { detection: DETECTIONS.readOnly[1]!, plan: plan(DETECTIONS.readOnly[1]!, "") },
  unparsableAntigravity: { detection: DETECTIONS.unparsable[2]!, plan: plan(DETECTIONS.unparsable[2]!, "") },
  claudeManaged: { detection: { ...claude, managed: MANAGED }, plan: plan({ ...claude, managed: MANAGED }, CONSENT_DIFFS.claude) },
};
