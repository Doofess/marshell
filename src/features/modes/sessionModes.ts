import type { AgentCatalog } from "../../generated/AgentCatalog";
import type { SessionMode } from "../../generated/SessionMode";
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

/** The modes a catalog entry offers, in the chip's order. Don't ask is only ever reported, so it is never offered. */
export const catalogModes = (offered: readonly SessionMode[]): Mode[] => MODES.filter((m) => offered.includes(m));

/**
 * The mode to use for an agent that offers `modes`: the one asked for if it is offered, else the first offered in `MODES` order,
 * else none (the agent maps no mode, so none is sent or shown). `undefined` means the five every adapter has.
 */
export function offeredMode(modes: readonly Mode[] | undefined, mode: Mode): Mode | undefined {
  if (modes === undefined || modes.includes(mode)) return mode;
  return MODES.find((m) => modes.includes(m));
}

/** The mode the chip moves to next: the following offered mode, wrapping round; from a mode that is not offered, the first. */
export function nextOffered(modes: readonly Mode[] | undefined, mode: Mode): Mode {
  const list = modes === undefined ? MODES : modes;
  return list[(list.indexOf(mode) + 1) % list.length] ?? mode;
}

export type Launch = { args: string[]; native: boolean };
/** What a permission request touches: an edit (inside the project or not), a command, a fetch, an MCP tool. */
export type Request = { kind: "edit" | "command" | "fetch" | "mcp" | "other"; inProject: boolean };

/** Claude Code, Codex and Antigravity are the agents with an adapter; every other agent is a manifest and has no entry below. */
const isAdapter = (cli: string): cli is CliId => cli === "claude" || cli === "codex" || cli === "antigravity";

/**
 * The modes a running session can switch to: the five for an adapter, a manifest agent's mapped ones from the catalog, and none
 * for a manifest agent the catalog doesn't list (or no catalog), since the core refuses a mode it can't map.
 */
export function offeredModes(agent: string | null | undefined, catalog: AgentCatalog | null): Mode[] {
  if (!agent) return [];
  if (isAdapter(agent)) return MODES;
  const entry = catalog?.agents.find((a) => a.id === agent);
  return entry ? catalogModes(entry.modes) : [];
}

const STRICT: Record<CliId, string[]> = {
  claude: ["--permission-mode", "manual"],
  codex: ["--sandbox", "read-only", "--ask-for-approval", "on-request"],
  antigravity: [],
};

/** Each CLI's own flag per mode; null where the CLI has no such mode. `mode_args` in crates/core/src/profiles/detect.rs is what the core really runs: keep the two together. */
const NATIVE: Record<CliId, Record<Mode, string[] | null>> = {
  claude: {
    manual: STRICT.claude,
    plan: ["--permission-mode", "plan"],
    "auto-edit": ["--permission-mode", "acceptEdits"],
    "full-auto": ["--permission-mode", "auto"],
    bypass: ["--permission-mode", "bypassPermissions"],
    // Only reported by Claude Code, never offered: no launcher or Mode menu entry reaches this flag.
    "dont-ask": ["--permission-mode", "dontAsk"],
  },
  codex: {
    manual: STRICT.codex,
    // Codex has no plan flag: a read-only sandbox, where nothing can change, stands in (the same start as Manual).
    plan: null,
    "auto-edit": ["--sandbox", "workspace-write", "--ask-for-approval", "on-request"],
    "full-auto": ["--approve-for-me"],
    bypass: ["--dangerously-bypass-approvals-and-sandbox"],
    "dont-ask": null,
  },
  antigravity: {
    manual: STRICT.antigravity,
    plan: ["--mode", "plan"],
    "auto-edit": ["--mode", "accept-edits"],
    "full-auto": ["--mode", "auto"],
    bypass: ["--dangerously-skip-permissions"],
    "dont-ask": null,
  },
};

/** A stand-in that needs no hook: Codex's read-only sandbox, where nothing can change, for Plan. */
const STAND_IN: Partial<Record<CliId, Partial<Record<Mode, string[]>>>> = { codex: { plan: STRICT.codex } };

/** The modes whose meaning lives in the CLI itself, so the session must start in them even with Marshell's hook on. */
const KEEPS_NATIVE: Mode[] = ["plan", "full-auto"];

/**
 * How to start a CLI in a mode. With the hook on, the strictest start unless the mode needs the CLI's own; null when the mode can't be had.
 * A manifest agent's flags live in its manifest and the core adds them, so there is none to give for it here.
 */
export function launchArgs(cli: string, mode: Mode, hooksOn: boolean): Launch | null {
  if (!isAdapter(cli)) return null;
  const own = NATIVE[cli][mode];
  const standIn = STAND_IN[cli]?.[mode];
  // Antigravity starts in its own flags with the hooks on or off (slice 6d decision 7: no strict start); the core's policy holds a call only in Bypass.
  if (cli === "antigravity") return own ? { args: own, native: true } : null;
  if (!hooksOn) return own ? { args: own, native: true } : standIn ? { args: standIn, native: false } : null;
  if (KEEPS_NATIVE.includes(mode)) return own ? { args: own, native: true } : { args: STRICT[cli], native: false };
  return { args: STRICT[cli], native: mode === "manual" };
}

/** Whether the CLI judges requests itself in this mode (Full auto with a classifier of its own; none is known for Antigravity's `--mode auto`, so Marshell's risk check stands in). */
const classifies = (cli: CliId, mode: Mode) => mode === "full-auto" && cli !== "antigravity";

/** What Marshell's hook answers when a CLI asks: allow, or ask (a card; the CLI's own prompt if approval is off). */
export function decide(cli: CliId, mode: Mode, request: Request, risky: boolean): "allow" | "ask" {
  if (risky) return "ask";
  switch (mode) {
    case "manual":
    case "plan":
    case "dont-ask": // Claude refuses what its rules don't allow; anything that still reaches the hook goes to a person.
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

/**
 * Changing mode mid-session: instant when only Marshell's answer changes, native when the CLI's own mode must change too.
 * A manifest agent has no Marshell hook, so a mode it maps (`offered`, from the catalog) is always native and any other is unavailable.
 */
export function switchKind(cli: string, from: Mode, to: Mode, hooksOn: boolean, offered?: readonly Mode[]): "instant" | "native" | "unavailable" {
  if (!isAdapter(cli)) return offered?.includes(to) ? "native" : "unavailable";
  if (!launchArgs(cli, to, hooksOn)) return "unavailable";
  // Antigravity's mode is its own flag whatever Marshell's hook does, so a switch is never only Marshell's answer.
  if (!hooksOn || cli === "antigravity") return "native";
  const a = launchArgs(cli, from, true)!;
  const b = launchArgs(cli, to, true)!;
  return a.args.join(" ") === b.args.join(" ") ? "instant" : "native";
}

/** A CLI setting that skips every prompt (e.g. Antigravity's dangerouslySkipPermissions) makes the session Bypass whatever was picked. */
export function effectiveMode(mode: Mode, skipsAll: boolean): Mode {
  return skipsAll ? "bypass" : mode;
}
