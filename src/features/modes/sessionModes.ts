import type { CliId } from "../onboarding/types";
import type { Mode } from "../sidebar/types";

/**
 * Session modes: Marshell sets the mode, the same five for every agent CLI (docs/PLAN.md "Session modes"). With Marshell's
 * permission hook on, a session starts in its CLI's strictest native mode, so every request reaches the hook, and the hook
 * answers by the mode; only Plan and a CLI's own classifier (Full auto) need the native mode itself. With the hook off, each mode
 * is the CLI's own launch flag. A risky request gets a card in every mode. Flags checked against `--help` of Claude Code,
 * codex-cli 0.162.1 and agy 1.3.3 on 2026-10-15.
 */
export const MODES: Mode[] = ["manual", "plan", "auto-edit", "full-auto", "bypass"];

export type Launch = { args: string[]; native: boolean };
/** What a permission request touches: an edit (inside the project or not), a command, a fetch, an MCP tool. */
export type Request = { kind: "edit" | "command" | "fetch" | "mcp" | "other"; inProject: boolean };

const STRICT: Record<CliId, string[]> = {
  claude: ["--permission-mode", "manual"],
  codex: ["--sandbox", "read-only", "--ask-for-approval", "on-request"],
  antigravity: [],
};

/** Each CLI's own flag per mode; null where the CLI has no such mode. */
const NATIVE: Record<CliId, Record<Mode, string[] | null>> = {
  claude: {
    manual: STRICT.claude,
    plan: ["--permission-mode", "plan"],
    "auto-edit": ["--permission-mode", "acceptEdits"],
    "full-auto": ["--permission-mode", "auto"],
    bypass: ["--permission-mode", "bypassPermissions"],
  },
  codex: {
    manual: STRICT.codex,
    // Codex has no plan flag: a read-only sandbox, where nothing can change, stands in (the same start as Manual).
    plan: null,
    "auto-edit": ["--sandbox", "workspace-write", "--ask-for-approval", "on-request"],
    "full-auto": ["--approve-for-me"],
    bypass: ["--dangerously-bypass-approvals-and-sandbox"],
  },
  antigravity: {
    manual: STRICT.antigravity,
    plan: ["--mode", "plan"],
    "auto-edit": ["--mode", "accept-edits"],
    // No classifier of its own: Full auto is Marshell's risk check, so it needs the hook.
    "full-auto": null,
    bypass: ["--dangerously-skip-permissions"],
  },
};

/** A stand-in that needs no hook: Codex's read-only sandbox, where nothing can change, for Plan. */
const STAND_IN: Partial<Record<CliId, Partial<Record<Mode, string[]>>>> = { codex: { plan: STRICT.codex } };

/** The modes whose meaning lives in the CLI itself, so the session must start in them even with Marshell's hook on. */
const KEEPS_NATIVE: Mode[] = ["plan", "full-auto"];

/** How to start a CLI in a mode. With the hook on, the strictest start unless the mode needs the CLI's own; null when the mode can't be had. */
export function launchArgs(cli: CliId, mode: Mode, hooksOn: boolean): Launch | null {
  const own = NATIVE[cli][mode];
  const standIn = STAND_IN[cli]?.[mode];
  if (!hooksOn) return own ? { args: own, native: true } : standIn ? { args: standIn, native: false } : null;
  if (KEEPS_NATIVE.includes(mode)) return own ? { args: own, native: true } : { args: STRICT[cli], native: false };
  return { args: STRICT[cli], native: mode === "manual" };
}

/** Whether the CLI judges requests itself in this mode (Full auto with a classifier of its own). */
const classifies = (cli: CliId, mode: Mode) => mode === "full-auto" && NATIVE[cli]["full-auto"] !== null;

/** What Marshell's hook answers when a CLI asks: allow, or ask (a card; the CLI's own prompt if approval is off). */
export function decide(cli: CliId, mode: Mode, request: Request, risky: boolean): "allow" | "ask" {
  if (risky) return "ask";
  switch (mode) {
    case "manual":
    case "plan":
      return "ask";
    case "auto-edit":
      return request.kind === "edit" && request.inProject ? "allow" : "ask";
    case "full-auto":
      // A CLI with its own classifier only asks about what the classifier held back, so that goes to a person.
      return classifies(cli, mode) ? "ask" : "allow";
    case "bypass":
      return "allow";
  }
}

/** Changing mode mid-session: instant when only Marshell's answer changes, native when the CLI's own mode must change too. */
export function switchKind(cli: CliId, from: Mode, to: Mode, hooksOn: boolean): "instant" | "native" | "unavailable" {
  if (!launchArgs(cli, to, hooksOn)) return "unavailable";
  if (!hooksOn) return "native";
  const a = launchArgs(cli, from, true)!;
  const b = launchArgs(cli, to, true)!;
  return a.args.join(" ") === b.args.join(" ") ? "instant" : "native";
}

/** A CLI setting that skips every prompt (e.g. Antigravity's dangerouslySkipPermissions) makes the session Bypass whatever was picked. */
export function effectiveMode(mode: Mode, skipsAll: boolean): Mode {
  return skipsAll ? "bypass" : mode;
}
