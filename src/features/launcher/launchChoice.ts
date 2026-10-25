import { isTool, type SessionKind } from "../../components/AgentMark/kinds";
import { TOOL_NAMES, type ToolId } from "../../components/AgentMark/tools";
import type { PresetField } from "../../generated/PresetField";
import { MODES, offeredMode } from "../modes/sessionModes";
import type { Mode } from "../sidebar/types";
import { modeLabel } from "../window/SessionHeader";
import type { Project } from "./filter";

export type { Mode };
export type Pace = "Quick" | "Balanced" | "Deep";
/** The five modes every agent CLI shares, in the order the chip cycles. */
export { MODES };
export const PACES: Pace[] = ["Quick", "Balanced", "Deep"];

/** The first chip lists AI agents, shells and tools under these headings. Dev tools and cloud tools share "Tools". */
export const LAUNCH_GROUPS = ["Agents", "Shells", "Tools"] as const;
export type LaunchGroup = (typeof LAUNCH_GROUPS)[number];

/**
 * What the launcher picks by: a built-in agent or tool (its kind), any other agent the core lists (its id, such as an agent of the
 * person's own), a live shell profile as `profile:<id>`, or a preset as `preset:<id>`.
 */
export type ProgramId = string;

/** Something the launcher can start: an AI agent, or a manual CLI that is just a terminal. */
export type LaunchProgram = {
  /** The built-in programs use their kind. */
  id: ProgramId;
  /** The mark and accent: the agent or tool this program is. */
  kind: SessionKind;
  label: string;
  /** What the tab says while it starts, e.g. "Starting Claude Code in my-app…". */
  cli: string;
  group: LaunchGroup;
  /** The hint in the box under the sentence: what to say first, or the command to run first. */
  first: string;
  /** True when there is nothing to type first: a shell profile takes no prompt, and neither does an agent CLI with no way to take one. */
  noFirst?: boolean;
  /** The modes the agent offers, in `MODES` order. Absent means the five; empty means none, so there is no mode chip. */
  modes?: Mode[];
  /** A preset this program starts its agent with (`deepseek`); the core adds what the preset needs. */
  preset?: string;
  /** The agent a preset starts. */
  agent?: string;
  /** Why the program can't be started yet. Starting it shows this and sends nothing. */
  problem?: string;
  /** The values a tool preset asks for before it starts, one box each. */
  fields?: PresetField[];
  /** The tool preset this program starts. */
  toolLaunch?: { tool: string; preset: string };
};

const agent = (id: SessionKind, label: string, cli: string): LaunchProgram => ({ id, kind: id, label, cli, group: "Agents", first: `What should ${label} do first? (optional)` });
const shell = (id: ToolId): LaunchProgram => ({ id, kind: id, label: TOOL_NAMES[id], cli: TOOL_NAMES[id], group: "Shells", first: "Run a command first (optional)" });
const tool = (id: ToolId, first: string, label = TOOL_NAMES[id], cli = label): LaunchProgram => ({ id, kind: id, label, cli, group: "Tools", first });

/** The agents the launcher design lists. The live launcher builds its own list from the core's catalog (`livePrograms`). DeepSeek has no command line of its own; it is a preset that starts Claude Code. Google's agent is Antigravity (`agy`), Gemini CLI is dropped. */
export const LAUNCH_PROGRAMS: LaunchProgram[] = [
  agent("claude", "Claude", "Claude Code"),
  agent("codex", "Codex", "Codex CLI"),
  agent("antigravity", "Antigravity", "Antigravity CLI"),
  agent("copilot", "GitHub Copilot", "Copilot CLI"),
  agent("cursor", "Cursor", "Cursor Agent"),
  agent("opencode", "opencode", "opencode"),
  agent("amp", "Amp", "Amp"),
  agent("qwen", "Qwen", "Qwen Code"),
  agent("grok", "Grok", "Grok CLI"),
  shell("powershell"),
  shell("cmd"),
  shell("gitbash"),
  shell("wsl"),
  shell("bash"),
  shell("zsh"),
  tool("python", "Run a script or start the prompt (optional)"),
  tool("node", "Run a script or start the prompt (optional)"),
  tool("docker", "Command to run, e.g. compose up (optional)"),
  tool("kubectl", "Command to run, e.g. get pods (optional)"),
  tool("terraform", "Command to run, e.g. plan (optional)"),
  tool("ssh", "Where to connect, e.g. user@host"),
  tool("aws", "Command to run, e.g. sso login (optional)"),
  tool("program", "Executable and arguments, e.g. htop", "Other program", "your program"),
];

export const programsIn = (group: LaunchGroup, programs: LaunchProgram[] = LAUNCH_PROGRAMS) => programs.filter((p) => p.group === group);
export const programById = (id: ProgramId, programs: LaunchProgram[] = LAUNCH_PROGRAMS) => programs.find((p) => p.id === id);
/** Only AI agents have a mode and a pace; a manual CLI is a plain terminal. Any program can start in a worktree. */
export const isAgentProgram = (id: ProgramId, programs: LaunchProgram[] = LAUNCH_PROGRAMS) => {
  const p = programById(id, programs);
  return p ? p.group === "Agents" : !isTool(id);
};

export type LauncherChoice = { program: ProgramId; projectId: string; mode: Mode; pace: Pace; worktree: boolean };
/** What is remembered per folder. The worktree choice is never remembered: the chip starts off every time. */
export type Remembered = Partial<Pick<LauncherChoice, "mode" | "pace">>;

/** The launcher as one sentence, for the dialog's accessible description. */
export function launcherSentence(c: LauncherChoice, projects: Project[], programs: LaunchProgram[] = LAUNCH_PROGRAMS): string {
  const label = programById(c.program, programs)?.label ?? c.program;
  const p = projects.find((x) => x.id === c.projectId);
  const where = p ? (p.branch ? `${p.name} on ${p.branch}` : p.name) : "an unknown folder";
  const start = `Start ${label} in ${where}`;
  if (isAgentProgram(c.program, programs)) {
    const mode = offeredMode(programById(c.program, programs)?.modes, c.mode);
    return `${start}, ${mode === undefined ? "" : `${modeLabel(mode)} mode, `}${c.pace}, ${c.worktree ? "in a new worktree" : "no worktree"}`;
  }
  return c.worktree ? `${start}, in a new worktree` : start;
}

/** What the new tab shows within 100 ms, before the process answers. */
export function startingText(program: ProgramId, projectName: string, programs: LaunchProgram[] = LAUNCH_PROGRAMS): string {
  return `Starting ${programById(program, programs)?.cli ?? program} in ${projectName}…`;
}

export function applyRemembered(c: LauncherChoice, r: Remembered | undefined, programs: LaunchProgram[] = LAUNCH_PROGRAMS): LauncherChoice {
  // Only mode and pace: an old entry that still carries a worktree flag must not turn the chip on.
  // A remembered mode the program does not offer falls back to the first it does; a program with none keeps the remembered one, unsent.
  const mode = r?.mode === undefined ? undefined : (offeredMode(programById(c.program, programs)?.modes, r.mode) ?? r.mode);
  return { ...c, ...(mode === undefined ? {} : { mode }), ...(r?.pace === undefined ? {} : { pace: r.pace }) };
}

export function rememberedNote(projectName: string, r: Remembered | undefined): string | null {
  if (!r) return null;
  const parts: string[] = [r.mode === undefined ? undefined : modeLabel(r.mode), r.pace].filter((x) => x !== undefined);
  return parts.length === 0 ? null : `Remembered for ${projectName}: ${parts.join(" · ")}. Change a choice to update it.`;
}
