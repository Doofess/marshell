import { isAgentId } from "../../components/AgentMark/agents";
import type { AgentCatalog } from "../../generated/AgentCatalog";
import type { CatalogAgent } from "../../generated/CatalogAgent";
import type { CreateSessionRequest } from "../../generated/CreateSessionRequest";
import type { FolderInfo } from "../../generated/FolderInfo";
import type { LayoutDoc } from "../../generated/LayoutDoc";
import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import type { ToolCatalog } from "../../generated/ToolCatalog";
import type { Task } from "../../generated/Task";
import type { TaskCreateRequest } from "../../generated/TaskCreateRequest";
import type { TaskPlanRequest } from "../../generated/TaskPlanRequest";
import { ApiError } from "../../lib/api/client";
import type { Project } from "../launcher/filter";
import type { PathCheck } from "../launcher/folderRows";
import { samePath } from "../tasks/taskOf";
import type { TaskConfirm } from "../launcher/Launcher";
import { LAUNCH_PROGRAMS, isAgentProgram, programById, type LaunchProgram, type LauncherChoice, type Mode, type ProgramId } from "../launcher/launchChoice";
import { catalogModes, offeredMode } from "../modes/sessionModes";
import { shellKind } from "./toRow";

/** The agents with a Rust adapter. Without a catalog these are all the live launcher lists, as before the core had one. */
const ADAPTER_AGENTS = ["claude", "codex", "antigravity"];
const PROFILE_PREFIX = "profile:";
const PRESET_PREFIX = "preset:";
const TOOL_PREFIX = "tool:";

const askFirst = (name: string) => `What should ${name} do first? (optional)`;

/** A program for an agent the catalog lists. An adapter keeps the launcher's own names; any other agent is named by its manifest. */
function agentProgram(a: CatalogAgent): LaunchProgram {
  const own = ADAPTER_AGENTS.includes(a.id) ? LAUNCH_PROGRAMS.find((p) => p.id === a.id) : undefined;
  const base: LaunchProgram = own ?? { id: a.id, kind: isAgentId(a.mark) ? a.mark : "generic", label: a.name, cli: a.cli, group: "Agents", first: askFirst(a.name) };
  // A CLI with no way to take a first prompt gets no box for one: the core refuses a prompt it can't pass.
  return { ...base, modes: catalogModes(a.modes), ...(a.prompt.style === "none" ? { noFirst: true } : {}) };
}

/**
 * What the live launcher offers: the agents the catalog found on this machine (adapters first, then the shipped agents, then the
 * person's own), the presets whose agent is found, then one shell per profile the core knows (marked by its family).
 * With no catalog (it could not be read) it lists the three adapters as before and claims nothing about any other agent.
 */
export function livePrograms(catalog: AgentCatalog | null, profiles: Profile[], tools?: ToolCatalog | null): LaunchProgram[] {
  const found = catalog === null ? null : catalog.agents.filter((a) => a.found);
  const agents = found === null ? ADAPTER_AGENTS.map((id) => LAUNCH_PROGRAMS.find((p) => p.id === id)!) : found.map(agentProgram);
  const presets = (catalog?.presets ?? []).flatMap((preset): LaunchProgram[] => {
    const target = found?.find((a) => a.id === preset.agent);
    if (target === undefined) return [];
    const starts = agentProgram(target);
    return [
      {
        ...starts,
        id: `${PRESET_PREFIX}${preset.id}`,
        kind: isAgentId(preset.mark) ? preset.mark : "generic",
        label: preset.name,
        cli: preset.name,
        first: askFirst(preset.name),
        preset: preset.id,
        agent: preset.agent,
        ...(preset.problem === null ? {} : { problem: preset.problem }),
      },
    ];
  });
  const shells = profiles.map(
    (p): LaunchProgram => ({ id: `${PROFILE_PREFIX}${p.id}`, kind: shellKind(p), label: p.name, cli: p.name, group: "Shells", first: "", noFirst: true }),
  );
  // The npx launcher profile has no presets, so it adds nothing here.
  const toolPresets = (tools?.tools ?? []).flatMap((t) =>
    t.presets.map(
      (p): LaunchProgram => ({
        id: `${TOOL_PREFIX}${t.id}:${p.id}`,
        kind: "generic",
        label: p.name,
        cli: t.name,
        group: "Tools",
        first: "",
        noFirst: true,
        fields: p.fields,
        toolLaunch: { tool: t.id, preset: p.id },
        ...(p.found ? {} : { problem: "Not installed, or not on PATH." }),
      }),
    ),
  );
  return [...agents, ...presets, ...shells, ...toolPresets];
}

/** True when the program asks for no values, or each of its fields has some text besides spaces. */
export function fieldsFilled(program: LaunchProgram | undefined, values: Record<string, string>): boolean {
  return (program?.fields ?? []).every((f) => (values[f.id] ?? "").trim() !== "");
}

/** The program the launcher shows for a choice: the chosen one if it is listed, else the first listed (an agent that is not installed is not). */
export function listedProgram(id: ProgramId, programs: LaunchProgram[]): ProgramId {
  return programs.some((p) => p.id === id) ? id : (programs[0]?.id ?? id);
}

/** Why the chosen program can't be started yet (a preset without its key, say), or undefined. Starting it shows this and sends nothing. */
export function startProblem(choice: LauncherChoice, programs: LaunchProgram[]): string | undefined {
  return programById(choice.program, programs)?.problem;
}

/** The launcher's folder for one of the core's recent projects. The path is the id: the core knows no other. */
export function toLauncherProject(p: ProjectInfo): Project {
  return { id: p.path, name: p.name, path: p.path, lastUsedMs: p.last_used_ms, ...(p.branch === null ? {} : { branch: p.branch }) };
}

export { samePath };

/** The launcher's folder for a folder the core checked. Its canonical path is the id, as for a recent project. */
function folderProject(folder: FolderInfo): Project {
  return { id: folder.path, name: folder.name, path: folder.path, lastUsedMs: 0, ...(folder.branch === null ? {} : { branch: folder.branch }) };
}

/**
 * Using a typed or browsed folder: the recent project at the same path (however it is spelt) is selected, so nothing is added twice;
 * otherwise `extra` is a project for the folder, to list beside the recent ones.
 */
export function chooseFolder(projects: Project[], folder: FolderInfo): { projectId: string; extra: Project | null } {
  const same = projects.find((p) => samePath(p.path, folder.path));
  return same ? { projectId: same.id, extra: null } : { projectId: folder.path, extra: folderProject(folder) };
}

/** What the folder field shows for the core's answer to a typed path: found, or missing (404); a 400 (not a full path) shows no row; any other failure is "failed", not a guess. */
export function checkState(path: string, result: { ok: FolderInfo } | { status: number }): PathCheck | null {
  if ("ok" in result) return { path, state: "found", folder: result.ok };
  if (result.status === 400) return null;
  return { path, state: result.status === 404 ? "missing" : "failed" };
}

/**
 * The Browse… flow: pick a folder, have the core check it, take it. Every way out except taking puts focus back in the field
 * (a cancel or a failure shows its message there), so a keyboard user keeps their place.
 */
export async function browseFolder(deps: {
  start: string | null;
  pick: (start: string | null) => Promise<string | null>;
  check: (path: string) => Promise<FolderInfo>;
  take: (folder: FolderInfo) => void;
  fail: (message: string) => void;
  focusField: () => void;
}): Promise<void> {
  const why = (e: unknown) => (e instanceof Error ? e.message : String(e));
  let picked: string | null;
  try {
    picked = await deps.pick(deps.start);
  } catch (e) {
    deps.fail(`Couldn't open the folder picker: ${why(e)}`);
    deps.focusField();
    return;
  }
  if (picked === null) {
    deps.focusField();
    return;
  }
  let folder: FolderInfo;
  try {
    folder = await deps.check(picked);
  } catch (e) {
    deps.fail(`Couldn't use ${picked}: ${why(e)}`);
    deps.focusField();
    return;
  }
  deps.take(folder);
}

/** Numbers the requests so only the newest one's answer is used: an older answer that arrives late is dropped. */
export function latestOnly() {
  let n = 0;
  return {
    next: () => ++n,
    isLatest: (k: number) => k === n,
  };
}

/**
 * The program last started in this folder, when the launcher can still offer it: an agent the launcher lists, or a shell
 * whose profile still exists. Undefined means nothing is remembered, or what was remembered is gone.
 */
export function preselect(doc: LayoutDoc | null, projectPath: string, programs: LaunchProgram[]): ProgramId | undefined {
  const remembered = doc?.launcher[projectPath];
  if (!remembered) return undefined;
  const preset = remembered.preset === undefined ? undefined : programById(`${PRESET_PREFIX}${remembered.preset}`, programs);
  if (preset !== undefined) return preset.id;
  const id: ProgramId = remembered.agent === null ? `${PROFILE_PREFIX}${remembered.profile_id}` : (remembered.agent as ProgramId);
  return programById(id, programs)?.id;
}

/** The mode last used in this folder, which the core stores only for the modes that ask first (never Full auto or Bypass). */
export function preselectMode(doc: LayoutDoc | null, projectPath: string): Mode | undefined {
  return doc?.launcher[projectPath]?.mode ?? undefined;
}

/**
 * The request for what the launcher shows. An agent runs inside the default shell and may take a first prompt; a shell
 * profile is just that shell and takes none. No folder means the core's default (the home folder).
 */
export function launchRequest(
  choice: LauncherChoice,
  prompt: string,
  programs: LaunchProgram[],
  profiles: ProfilesDoc,
  projects: Project[],
  size: { cols: number; rows: number },
  /** The folder of a task made for this launch; it takes the place of the chosen folder. */
  taskPath?: string,
  /** The values typed for a tool preset's fields, by field id. They go into this request and nowhere else. */
  fields: Record<string, string> = {},
): CreateSessionRequest {
  const cwd = taskPath ?? projects.find((p) => p.id === choice.projectId)?.path;
  const base: CreateSessionRequest = { cols: size.cols, rows: size.rows, ...(cwd === undefined ? {} : { cwd }) };
  const program = programById(choice.program, programs);
  // A tool preset runs inside the default shell; the core refuses a request that mixes it with an agent, prompt, mode or preset.
  if (program?.toolLaunch !== undefined) {
    const own = Object.fromEntries((program.fields ?? []).map((f) => [f.id, fields[f.id] ?? ""]));
    return { ...base, profile_id: profiles.default_id, tool: { ...program.toolLaunch, fields: own } };
  }
  const id = program?.id ?? choice.program;
  if (id.startsWith(PROFILE_PREFIX)) return { ...base, profile_id: id.slice(PROFILE_PREFIX.length) };
  // A preset starts its agent (Claude Code) with the preset named; the core reads whatever the preset needs, such as a key, itself.
  const withAgent = { ...base, agent: program?.agent ?? id, profile_id: profiles.default_id, ...(program?.preset === undefined ? {} : { preset: program.preset }) };
  if (!isAgentProgram(id, programs)) return withAgent;
  // Only what the agent maps is sent: the core refuses a mode its manifest has no flag for, and a prompt to a CLI that takes none.
  const mode = offeredMode(program?.modes, choice.mode);
  const withMode = mode === undefined ? withAgent : { ...withAgent, mode };
  return prompt.trim() !== "" && program?.noFirst !== true ? { ...withMode, prompt } : withMode;
}

/**
 * True when Start must first show the worktree commands: the worktree chip is on and the chosen folder is in git (it has a
 * branch). Any program can start in a worktree, a plain shell too. The chip is not offered outside git, so a stale flag never counts.
 */
export function wantsTask(choice: LauncherChoice, projects: Project[]): boolean {
  const inGit = projects.find((p) => p.id === choice.projectId)?.branch !== undefined;
  return choice.worktree && inGit;
}

/** The text the task is named from until the user types a name: the first prompt, or nothing for a program that takes none. */
export function nameSource(choice: LauncherChoice, prompt: string, programs: LaunchProgram[]): string {
  return programById(choice.program, programs)?.noFirst ? "" : prompt;
}

/** What the core is asked to plan: the chosen folder and the name as typed (the core slugs it). */
export function taskRequest(choice: LauncherChoice, projects: Project[], name: string): TaskPlanRequest {
  return { cwd: projects.find((p) => p.id === choice.projectId)?.path ?? "", name };
}

/** The create request: the plan request plus the commands the confirm view shows. Null until it shows some. */
export function createRequest(plan: TaskPlanRequest, confirm: TaskConfirm | null): TaskCreateRequest | null {
  return confirm?.state === "ready" ? { ...plan, commands: confirm.commands } : null;
}

/** A create the core refused (409: the commands changed, or it can't be made now): show why and plan again, so the next try shows what would run. */
export function replansAfter(e: unknown): boolean {
  return e instanceof ApiError && e.status === 409;
}

/** Creates the task, and only when that has answered starts the session in its folder. A failed create never launches. */
export async function createThenLaunch<T>(create: () => Promise<Task>, launch: (task: Task) => Promise<T>): Promise<T> {
  return launch(await create());
}

/**
 * The task made for the request being confirmed. If the session then fails to start, trying again must not make a second
 * worktree; but leaving the confirm view with Back forgets it, because the next plan may show other commands (another free name).
 */
export function rememberTasks() {
  let made: { key: string; task: Task } | null = null;
  return {
    async get(req: TaskCreateRequest, create: () => Promise<Task>): Promise<Task> {
      const key = JSON.stringify(req);
      if (made?.key === key) return made.task;
      const task = await create();
      made = { key, task };
      return task;
    },
    forget() {
      made = null;
    },
  };
}

/** The one status line of the confirm view while it works: the worktree first, then the program in it. */
export function progressText(phase: "creating" | "starting", program: string): string {
  return phase === "creating" ? "Creating the worktree…" : `Starting ${program}…`;
}

/** What takes focus when the launcher opens: the prompt, where Enter starts. A shell has no prompt, so the Start button (the program chip would open its list). */
export function initialFocus(root: { querySelector(selector: string): HTMLElement | null }): HTMLElement | null {
  return root.querySelector(".launcher__prompt") ?? root.querySelector(".launcher__start");
}

/** What the focus depends on: the chosen program, whether it takes a prompt, and whether Start is enabled. */
export type FocusState = { program: string; prompt: boolean; ready: boolean };

/**
 * Where focus goes after the choice changed, so Enter starts. After a pick the focus is on the program chip, where Enter reopens
 * the list. A program without a prompt takes focus to Start once it is enabled; one with a prompt, to the prompt.
 */
export function refocus(prev: FocusState | null, next: FocusState): "prompt" | "start" | null {
  if (prev === null || (prev.program === next.program && prev.ready === next.ready)) return null;
  if (next.prompt) return prev.program === next.program ? null : "prompt";
  return next.ready ? "start" : null;
}

/** What the launcher says when starting failed. The core's own text (a 4xx explaining a refusal) is shown as it is. */
export function startFailure(e: unknown): string {
  return e instanceof ApiError ? e.message : `Couldn't start the session: ${String(e)}`;
}
