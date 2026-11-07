import type { SessionKind } from "../../components/AgentMark/kinds";
import { TOOL_NAMES, type ToolId } from "../../components/AgentMark/tools";
import type { ShellFamily } from "../../generated/ShellFamily";
import { MODES, effectiveMode, offeredMode } from "../modes/sessionModes";
import type { Mode } from "../sidebar/types";
import { modeLabel } from "../window/SessionHeader";
import { ADMIN_OFF_WINDOWS, WAITING_FOR_ADMIN } from "../../lib/adminCopy";
import type { Os } from "../../lib/keymap";
import type { Project } from "./filter";

export { WAITING_FOR_ADMIN };

export type { Mode };
export type Pace = "Quick" | "Balanced" | "Deep";
/** The five modes every agent CLI shares, in the order the chip cycles. */
export { MODES };
export const PACES: Pace[] = ["Quick", "Balanced", "Deep"];

/** What the agent chip lists: agents only. The shell is the first chip's own list (`LaunchShell`). */
export type LaunchGroup = "Agents";

/** What the agent chip picks by: a built-in agent (its kind), any other agent the core lists (its id, such as an agent of the person's own), or a preset as `preset:<id>`. */
export type ProgramId = string;

/** A shell the launcher can start in: a profile of the core's (its id is the profile id) or, in the design, a stand-in. */
export type LaunchShell = { id: string; label: string; kind: ToolId; family: ShellFamily };

/** Something the launcher can start inside a shell: an AI agent. */
export type LaunchProgram = {
  /** The built-in programs use their kind. */
  id: ProgramId;
  /** The mark and accent: the agent or tool this program is. */
  kind: SessionKind;
  label: string;
  /** What the tab says while it starts, e.g. "Starting Claude Code in my-app…". */
  cli: string;
  group: LaunchGroup;
  /** The hint in the box under the sentence: what to say first. */
  first: string;
  /** True when there is nothing to type first: an agent CLI with no way to take a first prompt. */
  noFirst?: boolean;
  /** The modes the agent offers, in `MODES` order. Absent means the five; empty means none, so there is no mode chip. */
  modes?: Mode[];
  /** A preset this program starts its agent with (`deepseek`); the core adds what the preset needs. */
  preset?: string;
  /** The agent a preset starts. */
  agent?: string;
  /** Why the program can't be started yet. Starting it shows this and sends nothing. */
  problem?: string;
  /** The agent's own setting skips every prompt. */
  skipsAll?: { setting: string };
  /** The adapter's reason this program cannot start as administrator. */
  adminUnavailable?: string;
  /** Antigravity's settings have not answered yet, so its administrator start must wait. */
  adminSetupPending?: boolean;
  /** Antigravity's settings could not be read, so an administrator start needs the Bypass confirmation. */
  adminSetupFailed?: boolean;
};

const agent = (id: SessionKind, label: string, cli: string): LaunchProgram => ({ id, kind: id, label, cli, group: "Agents", first: `What should ${label} do first? (optional)` });
const shell = (id: ToolId, family: ShellFamily): LaunchShell => ({ id, label: TOOL_NAMES[id], kind: id, family });

/** The agents the launcher design lists. The live launcher builds its own list from the core's catalog (`liveAgents`). DeepSeek has no command line of its own; it is a preset that starts Claude Code. Google's agent is Antigravity (`agy`), Gemini CLI is dropped. */
export const LAUNCH_AGENTS: LaunchProgram[] = [
  agent("claude", "Claude", "Claude Code"),
  agent("codex", "Codex", "Codex CLI"),
  agent("antigravity", "Antigravity", "Antigravity CLI"),
  agent("copilot", "GitHub Copilot", "Copilot CLI"),
  agent("cursor", "Cursor", "Cursor Agent"),
  agent("opencode", "opencode", "opencode"),
  agent("amp", "Amp", "Amp"),
  agent("qwen", "Qwen", "Qwen Code"),
  agent("grok", "Grok", "Grok CLI"),
];

/** The shells the launcher design lists. The live launcher lists the core's shell profiles (`liveShells`). */
export const LAUNCH_SHELLS: LaunchShell[] = [
  shell("powershell", "power_shell"),
  shell("cmd", "cmd"),
  shell("gitbash", "posix"),
  shell("wsl", "wsl"),
  shell("bash", "posix"),
  shell("zsh", "posix"),
];

export const programById = (id: ProgramId, programs: LaunchProgram[] = LAUNCH_AGENTS) => programs.find((p) => p.id === id);
/** Only an AI agent has a mode and a pace; no agent means just the shell. Any choice can start in a worktree. */
export const isAgentProgram = (id: ProgramId | null): id is ProgramId => id !== null;

/** A null `agent` is "Just the shell". */
export type LauncherChoice = { shell: string; agent: ProgramId | null; projectId: string; mode: Mode; pace: Pace; worktree: boolean };
/** What is remembered per folder. The worktree choice is never remembered: the chip starts off every time. */
export type Remembered = Partial<Pick<LauncherChoice, "mode" | "pace">>;

/** The launcher as one sentence, for the dialog's accessible description: `[Shell] running [Agent] in [folder]`, or `[Shell] with no agent in [folder]`. */
export function launcherSentence(c: LauncherChoice, projects: Project[], programs: LaunchProgram[] = LAUNCH_AGENTS, elevated = false, shells: LaunchShell[] = LAUNCH_SHELLS): string {
  const shellLabel = shells.find((x) => x.id === c.shell)?.label ?? c.shell;
  const p = projects.find((x) => x.id === c.projectId);
  const where = p ? (p.branch ? `${p.name} on ${p.branch}` : p.name) : "an unknown folder";
  const admin = elevated ? ", as administrator" : "";
  const program = c.agent === null ? undefined : programById(c.agent, programs);
  if (program === undefined) return `Start ${shellLabel} with no agent in ${where}${c.worktree ? ", in a new worktree" : ""}${admin}`;
  const start = `Start ${shellLabel} running ${program.label} in ${where}`;
  const offered = offeredMode(program.modes, c.mode);
  const mode = offered === undefined ? undefined : effectiveMode(offered, program.skipsAll !== undefined);
  return `${start}, ${mode === undefined ? "" : `${modeLabel(mode)} mode, `}${c.pace}, ${c.worktree ? "in a new worktree" : "no worktree"}${admin}`;
}

/** What the new tab shows within 100 ms, before the process answers. */
export function startingText(program: ProgramId, projectName: string, programs: LaunchProgram[] = LAUNCH_AGENTS): string {
  return `Starting ${programById(program, programs)?.cli ?? program} in ${projectName}…`;
}

export function applyRemembered(c: LauncherChoice, r: Remembered | undefined, programs: LaunchProgram[] = LAUNCH_AGENTS): LauncherChoice {
  // Only mode and pace: an old entry that still carries a worktree flag must not turn the chip on.
  // A remembered mode the program does not offer falls back to the first it does; a program with none keeps the remembered one, unsent.
  const mode = r?.mode === undefined ? undefined : (offeredMode(c.agent === null ? undefined : programById(c.agent, programs)?.modes, r.mode) ?? r.mode);
  return { ...c, ...(mode === undefined ? {} : { mode }), ...(r?.pace === undefined ? {} : { pace: r.pace }) };
}

export function rememberedNote(projectName: string, r: Remembered | undefined): string | null {
  if (!r) return null;
  const parts: string[] = [r.mode === undefined ? undefined : modeLabel(r.mode), r.pace].filter((x) => x !== undefined);
  return parts.length === 0 ? null : `Remembered for ${projectName}: ${parts.join(" · ")}. Change a choice to update it.`;
}

/**
 * The Administrator chip is launcher state: it is not in `LauncherChoice` and never remembered. It is on only where the OS and adapter allow it;
 * an agent that starts as administrator starts in Manual, so its Mode chip shows that and is locked.
 */
export function adminChip(elevated: boolean | undefined, os: Os, unavailable?: string): { enabled: boolean; on: boolean; reason?: string } {
  const reason = os === "windows" ? unavailable : ADMIN_OFF_WINDOWS;
  const enabled = reason === undefined;
  return { enabled, on: enabled && elevated === true, ...(reason === undefined ? {} : { reason }) };
}

/** The longest session name, as in the sidebar's rename. */
export const MAX_SESSION_NAME = 120;

/** Control characters and the bidi controls the rename rules turn into spaces (`label_text` in the core's `history/marks.rs`). */
const NAME_DROPPED = /[\p{Cc}\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/gu;

/** Typed text as the field keeps it: those characters dropped, at most 120 characters. The core applies the rename rules again on create, so any caller holds to them. */
export function cleanName(text: string): string {
  return Array.from(text.replace(NAME_DROPPED, "")).slice(0, MAX_SESSION_NAME).join("");
}

/** The core's name for an agent session that is given none (`agent_display_name`, server/sessions.rs). */
const AGENT_NAMES: Record<string, string> = { claude: "Claude Code", codex: "Codex", antigravity: "Antigravity", qwen: "Qwen Code", copilot: "Copilot CLI" };

/** The name a session gets when none is typed: the agent's (a preset takes the agent it starts), else the shell's own name. Null is Just the shell. */
export function defaultSessionName(agent: ProgramId | null, agents: LaunchProgram[], shell: LaunchShell): string {
  const program = agent === null ? undefined : programById(agent, agents);
  const id = program?.agent ?? program?.id ?? agent;
  return (id === null ? undefined : AGENT_NAMES[id]) ?? shell.label;
}

/** The name to send: what was typed (cleaned, trimmed), else the task's name while a task is made for this launch, else nothing so the core names it. */
export function sessionNameFor(typed: string, taskName: string): string {
  return cleanName(typed).trim() || taskName.trim();
}
