import type { AgentId } from "../../components/AgentMark/agents";

export type Risk = { level: "normal" | "risky"; reason?: string };
export type DiffLine = { kind: "context" | "add" | "del"; text: string };
export type ToolDetail =
  | { kind: "bash"; command: string; cwd?: string }
  | { kind: "edit"; path: string; add: number; del: number; hunk: DiffLine[] }
  | { kind: "write"; path: string; preview: string; isNew: boolean }
  | { kind: "fetch"; url: string }
  | { kind: "mcp"; server: string; tool: string; args: Record<string, string> }
  | { kind: "question"; question: string; options: string[] };
export type CardState =
  | { phase: "pending" }
  | { phase: "decided"; verdict: "allowed" | "denied" }
  | { phase: "terminal" }
  | { phase: "released"; afterMs: number };
export type ApprovalRequest = {
  id: string;
  session: { name: string; agent: AgentId };
  sessionCwd: string;
  waitingMs: number;
  detail: ToolDetail;
  risk: Risk;
  /** The exact rule "Always allow" would add, e.g. "Bash(npm test:*)". Absent when no rule can be offered. */
  alwaysRule?: string;
  state: CardState;
};
