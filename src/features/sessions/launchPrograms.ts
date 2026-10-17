import type { CreateSessionRequest } from "../../generated/CreateSessionRequest";
import type { LayoutDoc } from "../../generated/LayoutDoc";
import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import type { Task } from "../../generated/Task";
import type { TaskCreateRequest } from "../../generated/TaskCreateRequest";
import type { TaskPlanRequest } from "../../generated/TaskPlanRequest";
import { ApiError } from "../../lib/api/client";
import type { Project } from "../launcher/filter";
import type { TaskConfirm } from "../launcher/Launcher";
import { LAUNCH_PROGRAMS, isAgentProgram, programById, type LaunchProgram, type LauncherChoice, type Mode, type ProgramId } from "../launcher/launchChoice";
import { shellKind } from "./toRow";

/** The agents the core can start today. The rest of `LAUNCH_PROGRAMS` stays hidden until their adapters exist. */
const LIVE_AGENTS = ["claude", "codex", "antigravity"];
const PROFILE_PREFIX = "profile:";

/** What the live launcher offers: the three agents, then one shell per profile the core knows (marked by its family). */
export function livePrograms(profiles: Profile[]): LaunchProgram[] {
  const agents = LIVE_AGENTS.map((id) => LAUNCH_PROGRAMS.find((p) => p.id === id)!);
  const shells = profiles.map(
    (p): LaunchProgram => ({ id: `${PROFILE_PREFIX}${p.id}`, kind: shellKind(p), label: p.name, cli: p.name, group: "Shells", first: "", noFirst: true }),
  );
  return [...agents, ...shells];
}

/** The launcher's folder for one of the core's recent projects. The path is the id: the core knows no other. */
export function toLauncherProject(p: ProjectInfo): Project {
  return { id: p.path, name: p.name, path: p.path, lastUsedMs: p.last_used_ms, ...(p.branch === null ? {} : { branch: p.branch }) };
}

/**
 * The program last started in this folder, when the launcher can still offer it: an agent the launcher lists, or a shell
 * whose profile still exists. Undefined means nothing is remembered, or what was remembered is gone.
 */
export function preselect(doc: LayoutDoc | null, projectPath: string, programs: LaunchProgram[]): ProgramId | undefined {
  const remembered = doc?.launcher[projectPath];
  if (!remembered) return undefined;
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
): CreateSessionRequest {
  const cwd = taskPath ?? projects.find((p) => p.id === choice.projectId)?.path;
  const base: CreateSessionRequest = { cols: size.cols, rows: size.rows, ...(cwd === undefined ? {} : { cwd }) };
  const id = programById(choice.program, programs)?.id ?? choice.program;
  if (id.startsWith(PROFILE_PREFIX)) return { ...base, profile_id: id.slice(PROFILE_PREFIX.length) };
  const withAgent = { ...base, agent: id, profile_id: profiles.default_id };
  if (!isAgentProgram(id, programs)) return withAgent;
  const withMode = { ...withAgent, mode: choice.mode };
  return prompt.trim() !== "" ? { ...withMode, prompt } : withMode;
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
