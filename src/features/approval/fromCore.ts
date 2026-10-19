import type { PermissionDetail } from "../../generated/PermissionDetail";
import type { PermissionDecision } from "../../generated/PermissionDecision";
import type { PermissionView } from "../../generated/PermissionView";
import { isAgentId, type AgentId } from "../../components/AgentMark/agents";
import type { RowModel } from "../sidebar/types";
import type { Answer } from "./LiveCard";
import type { ApprovalRequest, ToolDetail } from "./types";

const record = (pairs: Array<[string, string]>): Record<string, string> => Object.fromEntries(pairs);

/** The core's description of what a request asks to do, in the card's own shape. */
export function toDetail(d: PermissionDetail): ToolDetail {
  switch (d.kind) {
    case "bash":
      return { kind: "bash", command: d.command, cwd: d.cwd ?? undefined };
    case "edit":
      return { kind: "edit", path: d.path, add: d.add, del: d.del, hunk: d.hunk };
    case "write":
      return { kind: "write", path: d.path, preview: d.preview, isNew: d.is_new };
    case "fetch":
      return { kind: "fetch", url: d.url };
    case "mcp":
      return { kind: "mcp", server: d.server, tool: d.tool, args: record(d.args) };
    case "other":
      return { kind: "other", tool: d.tool, args: record(d.args) };
  }
}

const agentOf = (row: RowModel | undefined, view: PermissionView): AgentId => (row && isAgentId(row.agent) ? row.agent : isAgentId(view.agent) ? view.agent : "claude");

/** A held request as the lane shows it. `now` is the clock the wait counts from. */
export function toRequest(v: PermissionView, row: RowModel | undefined, now: number): ApprovalRequest {
  return {
    id: v.rid,
    tab: v.tab,
    session: { name: row?.name ?? v.tab, agent: agentOf(row, v), number: row?.number },
    sessionCwd: "",
    waitingMs: Math.max(0, now - v.at_ms),
    detail: toDetail(v.detail),
    risk: { level: v.risk ? "risky" : "normal", reason: v.risk ?? undefined },
    alwaysRule: v.always_rule ?? undefined,
    state: { phase: "pending" },
  };
}

const WAITING = new Set(["needs-permission", "needs-question"]);

/**
 * Everything that needs the person, oldest first: every held request, plus a one-liner for each session that is waiting with no
 * held request (its CLI asks in its own terminal). This list is the needs-you count everywhere.
 */
export function laneEntries(views: PermissionView[], rows: RowModel[], now: number): ApprovalRequest[] {
  const byTab = new Map(rows.map((r) => [r.id, r]));
  const held = new Set(views.map((v) => v.tab));
  const entries = views.map((v) => toRequest(v, byTab.get(v.tab), now));
  for (const row of rows) {
    if (!WAITING.has(row.status) || held.has(row.id)) continue;
    entries.push({
      id: `terminal-${row.id}`,
      tab: row.id,
      session: { name: row.name, agent: isAgentId(row.agent) ? row.agent : "claude", number: row.number },
      sessionCwd: "",
      waitingMs: row.waitingMs ?? 0,
      detail: { kind: "question", question: row.phrase, options: [] },
      risk: { level: "normal" },
      state: { phase: "pending" },
      terminalOnly: true,
    });
  }
  // Array.sort is stable, so equal waits keep the core's order.
  return entries.sort((a, b) => b.waitingMs - a.waitingMs);
}

/** What the core is told for a card's answer; "terminal" is a release, not a decision, so it has none. */
export function decisionFor(a: Exclude<Answer, "terminal">): PermissionDecision {
  switch (a) {
    case "allow":
      return { behavior: "allow", scope: "once", message: null };
    case "always":
      return { behavior: "allow", scope: "project", message: null };
    case "deny":
      return { behavior: "deny", scope: "once", message: null };
  }
}
