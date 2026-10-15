import type { CreateSessionRequest } from "../../generated/CreateSessionRequest";
import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import { ApiError } from "../../lib/api/client";
import type { Project } from "../launcher/filter";
import { LAUNCH_PROGRAMS, isAgentProgram, programById, type LaunchProgram, type LauncherChoice } from "../launcher/launchChoice";
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
): CreateSessionRequest {
  const cwd = projects.find((p) => p.id === choice.projectId)?.path;
  const base: CreateSessionRequest = { cols: size.cols, rows: size.rows, ...(cwd === undefined ? {} : { cwd }) };
  const id = programById(choice.program, programs)?.id ?? choice.program;
  if (id.startsWith(PROFILE_PREFIX)) return { ...base, profile_id: id.slice(PROFILE_PREFIX.length) };
  const withAgent = { ...base, agent: id, profile_id: profiles.default_id };
  return isAgentProgram(id, programs) && prompt.trim() !== "" ? { ...withAgent, prompt } : withAgent;
}

/** What takes focus when the launcher opens: the prompt, where Enter starts. A shell has no prompt, so the Start button (the program chip would open its list). */
export function initialFocus(root: { querySelector(selector: string): HTMLElement | null }): HTMLElement | null {
  return root.querySelector(".launcher__prompt") ?? root.querySelector(".launcher__start");
}

/** What the launcher says when starting failed. The core's own text (a 4xx explaining a refusal) is shown as it is. */
export function startFailure(e: unknown): string {
  return e instanceof ApiError ? e.message : `Couldn't start the session: ${String(e)}`;
}
