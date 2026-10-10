import { isTool, type SessionKind } from "../../components/AgentMark/kinds";
import { TOOL_NAMES, type ToolId } from "../../components/AgentMark/tools";
import type { Project } from "./filter";

export type Mode = "Manual" | "Plan" | "Auto-edit";
export type Pace = "Quick" | "Balanced" | "Deep";
export const MODES: Mode[] = ["Manual", "Plan", "Auto-edit"];
export const PACES: Pace[] = ["Quick", "Balanced", "Deep"];

/** The first chip lists AI agents, shells and tools under these headings. Dev tools and cloud tools share "Tools". */
export const LAUNCH_GROUPS = ["Agents", "Shells", "Tools"] as const;
export type LaunchGroup = (typeof LAUNCH_GROUPS)[number];

/** Something the launcher can start: an AI agent, or a manual CLI that is just a terminal. */
export type LaunchProgram = {
  id: SessionKind;
  label: string;
  /** What the tab says while it starts, e.g. "Starting Claude Code in my-app…". */
  cli: string;
  group: LaunchGroup;
  /** The hint in the box under the sentence: what to say first, or the command to run first. */
  first: string;
};

const agent = (id: SessionKind, label: string, cli: string): LaunchProgram => ({ id, label, cli, group: "Agents", first: `What should ${label} do first? (optional)` });
const shell = (id: ToolId): LaunchProgram => ({ id, label: TOOL_NAMES[id], cli: TOOL_NAMES[id], group: "Shells", first: "Run a command first (optional)" });
const tool = (id: ToolId, first: string, label = TOOL_NAMES[id], cli = label): LaunchProgram => ({ id, label, cli, group: "Tools", first });

/** Every AI agent CLI with a first-party command line. DeepSeek has none, so it is not listed. Which ones can be started still depends on the adapters of phase 6; the launcher lists them all. */
export const LAUNCH_PROGRAMS: LaunchProgram[] = [
  agent("claude", "Claude", "Claude Code"),
  agent("codex", "Codex", "Codex CLI"),
  agent("gemini", "Gemini", "Gemini CLI"),
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

export const programsIn = (group: LaunchGroup) => LAUNCH_PROGRAMS.filter((p) => p.group === group);
export const programById = (id: string) => LAUNCH_PROGRAMS.find((p) => p.id === id);
/** Only AI agents have a mode, a pace and a worktree choice; a manual CLI is a plain terminal. */
export const isAgentProgram = (id: string) => !isTool(id);

export type LauncherChoice = { program: SessionKind; projectId: string; mode: Mode; pace: Pace; worktree: boolean };
export type Remembered = Partial<Pick<LauncherChoice, "mode" | "pace" | "worktree">>;

/** The launcher as one sentence, for the dialog's accessible description. */
export function launcherSentence(c: LauncherChoice, projects: Project[]): string {
  const label = programById(c.program)?.label ?? c.program;
  const p = projects.find((x) => x.id === c.projectId);
  const where = p ? (p.branch ? `${p.name} on ${p.branch}` : p.name) : "an unknown folder";
  const start = `Start ${label} in ${where}`;
  return isAgentProgram(c.program) ? `${start}, ${c.mode} mode, ${c.pace}, ${c.worktree ? "in a new worktree" : "no worktree"}` : start;
}

/** What the new tab shows within 100 ms, before the process answers. */
export function startingText(program: SessionKind, projectName: string): string {
  return `Starting ${programById(program)?.cli ?? program} in ${projectName}…`;
}

export function applyRemembered(c: LauncherChoice, r: Remembered | undefined): LauncherChoice {
  return { ...c, ...r };
}

export function rememberedNote(projectName: string, r: Remembered | undefined): string | null {
  if (!r) return null;
  const parts = [r.mode, r.pace, r.worktree ? "worktree" : undefined].filter((x): x is string => Boolean(x));
  return parts.length === 0 ? null : `Remembered for ${projectName}: ${parts.join(" · ")}. Change a choice to update it.`;
}
