import { isAgentId } from "../../components/AgentMark/agents";
import type { SessionKind } from "../../components/AgentMark/kinds";
import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { RowModel } from "../sidebar/types";

/** The mark for a shell session, from its profile's family. A profile that no longer exists gets the generic program mark. */
function shellKind(profile: Profile | undefined): SessionKind {
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
function lastSegment(path: string): string {
  return path.split(/[\\/]+/).filter(Boolean).pop() ?? path;
}

/**
 * The sidebar row for one open session the core lists. Before the hooks arrive (phase 3) the core knows nothing about
 * what an agent is doing, so a running agent is "limited" and a running shell is "idle": never "working".
 */
export function toRow(info: SessionInfo, profiles: Profile[]): RowModel {
  const profile = profiles.find((p) => p.id === info.profile_id);
  const agent: SessionKind = info.agent === null ? shellKind(profile) : isAgentId(info.agent) ? info.agent : "generic";
  const base = { id: info.tab_id, number: info.number, name: info.name, project: lastSegment(info.cwd), agent };
  if (info.state.kind === "exited") return { ...base, status: "ended", phrase: `Exited with code ${info.state.code}` };
  if (info.agent !== null) return { ...base, status: "limited", phrase: "Status arrives with hooks" };
  return { ...base, status: "idle", phrase: "" };
}
