import { isAgentId } from "../../components/AgentMark/agents";
import type { SessionKind } from "../../components/AgentMark/kinds";
import { toolForProgram } from "../../components/AgentMark/tools";
import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { RowModel } from "../sidebar/types";

/** The mark for a shell session, from its profile's family. A profile that no longer exists gets the generic program mark. */
export function shellKind(profile: Profile | undefined): SessionKind {
  switch (profile?.family) {
    case "power_shell":
      return "powershell";
    case "cmd":
      return "cmd";
    case "posix":
      return profile.id === "git-bash" ? "gitbash" : "bash";
    case "wsl":
      return "wsl";
    default:
      return "program";
  }
}

/** The last folder of a path, with either kind of separator and any trailing ones. */
export function lastSegment(path: string): string {
  return path.split(/[\\/]+/).filter(Boolean).pop() ?? path;
}

/** The row phrase of a tab with no process: it was open when the app last closed. The Resume strip tells it from an Exited tab by this. */
export const ENDED_PHRASE = "Ended last time";

/**
 * The sidebar row for one open session the core lists. Before the hooks arrive (phase 3) the core knows nothing about
 * what an agent is doing, so a running agent is "limited". A shell row's status comes from its prompt marks: "working"
 * while a command runs, and "idle" at the prompt (or before any mark), naming a failed last command.
 */
export function toRow(info: SessionInfo, profiles: Profile[]): RowModel {
  const profile = profiles.find((p) => p.id === info.profile_id);
  // The foreground only speaks for a shell that is running and not paused; a launched agent's row is its own.
  const fg = info.agent === null && info.state.kind === "running" && !info.paused ? info.foreground : null;
  const agent: SessionKind =
    fg !== null
      ? fg.agent !== null && isAgentId(fg.agent)
        ? fg.agent
        : toolForProgram(fg.program)
      : info.agent === null
        ? shellKind(profile)
        : isAgentId(info.agent)
          ? info.agent
          : "generic";
  const base = {
    id: info.tab_id,
    number: info.number,
    name: info.name,
    project: lastSegment(info.project),
    projectKey: info.project,
    agent,
    ...(info.mode === null ? {} : { mode: info.mode }),
  };
  if (info.state.kind === "ended") return { ...base, status: "ended", phrase: ENDED_PHRASE };
  if (info.state.kind === "exited") return { ...base, status: "ended", phrase: `Exited with code ${info.state.code}` };
  if (info.paused) return { ...base, status: "paused", phrase: "" };
  if (info.agent !== null) return { ...base, status: "limited", phrase: "Status arrives with hooks" };
  if (fg !== null) {
    if (fg.agent !== null) return { ...base, status: "limited", phrase: "Status arrives with hooks" };
    return { ...base, status: "working", phrase: `Running ${fg.command_line ?? fg.program}` };
  }
  const command = info.command;
  if (command.kind === "running") return { ...base, status: "working", phrase: "Running a command" };
  if (command.kind === "prompt" && command.last_exit !== null && command.last_exit !== 0) {
    return { ...base, status: "idle", phrase: `Last command failed (exit ${command.last_exit})` };
  }
  return { ...base, status: "idle", phrase: "" };
}
