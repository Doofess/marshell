import type { ShellAgentProblem } from "../../generated/ShellAgentProblem";
import type { ShellAgents } from "../../generated/ShellAgents";
import type { LaunchProgram, LauncherChoice, ProgramId } from "./launchChoice";

/** What the agent chip can offer for the chosen shell: still asking, the shell could not be asked, or the answer. */
export type ShellOffer =
  | { state: "checking" }
  | { state: "unreached" }
  | { state: "ready"; offered: LaunchProgram[]; missing: MissingAgent[] };

/** An agent the shell lacks, and why. `note` is the core's own sentence with more about the problem, shown as it was sent. */
export type MissingAgent = { label: string; problem: ShellAgentProblem; note?: string };

/**
 * Turns the core's answer for a shell into what the chip lists. `undefined` is not asked yet and an answer for another shell is
 * not this one's, so both are checking; a request that failed, or a shell the core could not reach, offers only the shell.
 * A preset is offered exactly when the agent it starts is, and is never listed as missing (the agent is).
 */
export function offerFor(agents: LaunchProgram[], answer: ShellAgents | "failed" | undefined, shellId: string): ShellOffer {
  if (answer === undefined) return { state: "checking" };
  if (answer === "failed") return { state: "unreached" };
  if (answer.profile_id !== shellId) return { state: "checking" };
  if (!answer.reached) return { state: "unreached" };
  const problems = new Map(answer.agents.map((a) => [a.id, a.problem]));
  const notes = new Map(answer.agents.map((a) => [a.id, a.note]));
  const offered = agents.filter((p) => problems.get(p.agent ?? p.id) === null);
  const missing = agents.flatMap((p): MissingAgent[] => {
    const problem = problems.get(p.id);
    const note = notes.get(p.id);
    return p.preset === undefined && problem !== undefined && problem !== null ? [{ label: p.label, problem, ...(note !== undefined ? { note } : {}) }] : [];
  });
  return { state: "ready", offered, missing };
}

const REASONS: Record<ShellAgentProblem, string> = {
  not_installed: "not installed",
  not_driven_here: "Marshell runs it in WSL later",
  path_has_space: "its path has a space, which Command Prompt can't start",
};

/**
 * The one line under the agent list naming each agent the shell lacks and why, or null when it lacks none. An agent's own
 * sentence from the core follows the list, so the list keeps its shape and the sentence reads whole.
 */
export function missingLine(shellLabel: string, missing: MissingAgent[]): string | null {
  if (missing.length === 0) return null;
  const list = `Not in ${shellLabel}: ${missing.map((m) => `${m.label} (${REASONS[m.problem]})`).join(", ")}`;
  const notes = missing.flatMap((m) => (m.note !== undefined ? [m.note] : []));
  return notes.length === 0 ? list : `${list}. ${notes.join(" ")}`;
}

/**
 * The agent a folder with nothing remembered starts with, so no CLI is the default by its name: the first agent the shell offers.
 * While the shell is being asked, the first one `found` on this machine (in the catalog's order), so the chip keeps it when the
 * answer agrees. Just the shell when the shell offers no agent. A preset is never the default by itself.
 */
export function firstAgent(offer: ShellOffer, found: readonly ProgramId[]): ProgramId | null {
  if (offer.state === "checking") return found[0] ?? null;
  if (offer.state === "unreached") return null;
  return offer.offered.find((p) => p.preset === undefined)?.id ?? null;
}

/**
 * The agent a start uses: the chosen one only while the shell offers it. While the shell is being asked it is the chosen one (Start
 * waits); once the answer is in, an agent the shell lacks becomes Just the shell, and so does any agent when the shell is unreached.
 */
export function effectiveAgent(choice: Pick<LauncherChoice, "agent">, offer: ShellOffer): ProgramId | null {
  if (choice.agent === null) return null;
  if (offer.state === "checking") return choice.agent;
  if (offer.state === "unreached") return null;
  return offer.offered.some((p) => p.id === choice.agent) ? choice.agent : null;
}
