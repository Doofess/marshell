import type { CliId } from "../onboarding/types";
import { launchArgs } from "../modes/sessionModes";
import type { Mode } from "../sidebar/types";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { Endpoint } from "../../generated/Endpoint";
import { sessions, type SessionCommandPurpose } from "../../lib/api/sessions";
// The shipped manifests the core reads, so a shipped agent's command can never drift from what the core runs.
import shipped from "../../../crates/core/src/agents/manifests.json";

const isCliId = (agent: string): agent is CliId => agent === "claude" || agent === "codex" || agent === "antigravity" || agent === "qwen" || agent === "copilot";

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
  const base = { claude: "claude --continue", codex: "codex resume --last", antigravity: "agy --continue", qwen: "qwen --continue", copilot: "copilot --continue" }[agent];
  // The mode's own flags, as `mode_args` gives them; a mode the CLI can't have adds nothing.
  const flags = mode === undefined ? [] : (launchArgs(agent, mode, false)?.args ?? []);
  return [base, ...flags].join(" ");
}

/**
 * The Resume bar's command for a session. The core resumes with the mode it was last started in, so the flags show too.
 * An administrator agent is the exception: Resume uses Manual flags (decision 10); its own setting may still make it Bypass.
 */
export function sessionResumeCommand(info: Pick<SessionInfo, "agent" | "mode"> & { elevated?: boolean }, profileName: string, command?: string): string {
  const mode = info.elevated === true && info.agent !== null ? "manual" : (info.mode ?? undefined);
  return command ?? resumeCommand(info.agent, profileName, mode);
}

/** How long a bar waits for the core's command before it opens with the local text. */
export const SESSION_COMMAND_TIMEOUT_MS = 2000;

/** The core's bare command for a bar. A refusal, a failed request or no answer within `timeoutMs` leaves the local text available. */
export function fetchSessionCommand(endpoint: Endpoint, tabId: string, purpose: SessionCommandPurpose, request = sessions.resumeCommand, timeoutMs = SESSION_COMMAND_TIMEOUT_MS): Promise<string | undefined> {
  return new Promise((resolve) => {
    const settle = (command: string | undefined) => { clearTimeout(timer); resolve(command); };
    const timer = setTimeout(() => resolve(undefined), timeoutMs);
    try {
      request(endpoint, tabId, purpose).then((result) => settle(result.ok ? result.command : undefined), () => settle(undefined));
    } catch {
      settle(undefined);
    }
  });
}
