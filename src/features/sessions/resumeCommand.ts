import type { CliId } from "../onboarding/types";
import { launchArgs } from "../modes/sessionModes";
import type { Mode } from "../sidebar/types";
import type { SessionInfo } from "../../generated/SessionInfo";

const isCliId = (agent: string): agent is CliId => agent === "claude" || agent === "codex" || agent === "antigravity";

/**
 * The command Resume will run for a session, for display only. It mirrors `resume_args` in
 * crates/core/src/profiles/detect.rs, which is what the core really runs; keep the two together.
 * A plain shell has no agent, so it starts again as its profile. With a mode, the CLI's own flags for it follow.
 */
export function resumeCommand(agent: string | null, profileName: string, mode?: Mode): string {
  if (agent === null) return profileName;
  if (!isCliId(agent)) return agent; // The core refuses an agent it does not know, with its own words; show the name until then.
  const base = { claude: "claude --continue", codex: "codex resume --last", antigravity: "agy --continue" }[agent];
  // The mode's own flags, as `mode_args` gives them; a mode the CLI can't have adds nothing.
  const flags = mode === undefined ? [] : (launchArgs(agent, mode, false)?.args ?? []);
  return [base, ...flags].join(" ");
}

/** The Resume bar's command for a session. The core resumes with the mode it was last started in, so the flags show too. */
export function sessionResumeCommand(info: Pick<SessionInfo, "agent" | "mode">, profileName: string): string {
  return resumeCommand(info.agent, profileName, info.mode ?? undefined);
}
