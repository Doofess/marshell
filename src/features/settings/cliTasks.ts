import type { AgentInstall } from "../../generated/AgentInstall";
import type { CreateSessionRequest } from "../../generated/CreateSessionRequest";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { CompanyAgent } from "./settingsSections";

export type CliTaskKind = "sign_in" | "update";

/** A sign-in or update tab Settings opened and has not yet seen exit. */
export type PendingTask = { tabId: string; agent: CompanyAgent; kind: CliTaskKind };

/** The request for a sign-in or update tab: the core builds the command and picks the folder, so only the size and the task are set. */
export function cliTaskRequest(agent: CompanyAgent, kind: CliTaskKind, size: { cols: number; rows: number }): CreateSessionRequest {
  return { cols: size.cols, rows: size.rows, cli_task: { agent, kind } };
}

/** How many sessions are running the CLI: started as it, or a shell with it in the foreground. */
export function runningCount(sessions: SessionInfo[], agent: CompanyAgent): number {
  return sessions.filter((s) => s.state.kind === "running" && (s.agent === agent || s.foreground?.agent === agent)).length;
}

/** Splits the pending tabs into those that are done (no longer running, or gone from the list) and those still waiting. */
export function exitedTasks(prev: Map<string, PendingTask>, sessions: SessionInfo[]): { done: PendingTask[]; rest: Map<string, PendingTask> } {
  const running = new Set(sessions.filter((s) => s.state.kind === "running").map((s) => s.tab_id));
  const done: PendingTask[] = [];
  const rest = new Map<string, PendingTask>();
  for (const [id, task] of prev) {
    if (running.has(id)) rest.set(id, task);
    else done.push(task);
  }
  return { done, rest };
}

/** What to say once an update tab has exited and the version was read again. */
export function updateResult(name: string, before: string | null, after: AgentInstall): string {
  if (!after.found || after.version === null) return `Couldn't read ${name}'s version.`;
  return before !== null && before !== after.version ? `${name} is now version ${after.version}.` : `${name} is still version ${after.version}.`;
}
