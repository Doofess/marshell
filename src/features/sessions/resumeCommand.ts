import type { CliId } from "../onboarding/types";
import { launchArgs } from "../modes/sessionModes";
import type { Mode } from "../sidebar/types";
import type { SessionInfo } from "../../generated/SessionInfo";
// The shipped manifests the core reads, so a shipped agent's command can never drift from what the core runs.
import shipped from "../../../crates/core/src/agents/manifests.json";

const isCliId = (agent: string): agent is CliId => agent === "claude" || agent === "codex" || agent === "antigravity";

type ShippedManifest = { id: string; commands: string[]; continue?: string[]; modes: Partial<Record<Mode, string[]>> };
const MANIFESTS = shipped as ShippedManifest[];

/**
 * A shipped manifest agent's resume command, as the core builds it: its command, the arguments that continue its last
 * conversation, then the flags of the mode it started in (none for a mode the manifest does not map). Undefined for an agent
 * with no shipped manifest, such as one the person defined: the frontend does not know its command.
 */
function manifestResume(agent: string, mode?: Mode): string | undefined {
  const m = MANIFESTS.find((x) => x.id === agent);
  if (m?.commands[0] === undefined) return undefined;
  return [m.commands[0], ...(m.continue ?? []), ...(mode === undefined ? [] : (m.modes[mode] ?? []))].join(" ");
}

/**
 * The command Resume will run for a session, for display only. It mirrors `resume_args` in
 * crates/core/src/profiles/detect.rs, which is what the core really runs; keep the two together.
 * A plain shell has no agent, so it starts again as its profile. With a mode, the CLI's own flags for it follow.
 */
export function resumeCommand(agent: string | null, profileName: string, mode?: Mode): string {
  if (agent === null) return profileName;
  if (!isCliId(agent)) return manifestResume(agent, mode) ?? agent; // An agent with no manifest here: the core answers for it in its own words, so show the name.
  const base = { claude: "claude --continue", codex: "codex resume --last", antigravity: "agy --continue" }[agent];
  // The mode's own flags, as `mode_args` gives them; a mode the CLI can't have adds nothing.
  const flags = mode === undefined ? [] : (launchArgs(agent, mode, false)?.args ?? []);
  return [base, ...flags].join(" ");
}

/** The Resume bar's command for a session. The core resumes with the mode it was last started in, so the flags show too. */
export function sessionResumeCommand(info: Pick<SessionInfo, "agent" | "mode">, profileName: string): string {
  return resumeCommand(info.agent, profileName, info.mode ?? undefined);
}
