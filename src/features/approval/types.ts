import type { AgentId } from "../../components/AgentMark/agents";

export type Risk = { level: "normal" | "risky"; reason?: string };
export type DiffLine = { kind: "context" | "add" | "del"; text: string };
export type ToolDetail =
  | { kind: "bash"; command: string; cwd?: string }
  | { kind: "edit"; path: string; add: number; del: number; hunk: DiffLine[] }
  | { kind: "write"; path: string; preview: string; isNew: boolean }
  | { kind: "fetch"; url: string }
  | { kind: "mcp"; server: string; tool: string; args: Record<string, string> }
  /** A tool the card has no layout for (NotebookEdit, ...): its name and input as `key: value` lines. */
  | { kind: "other"; tool: string; args: Record<string, string> }
  | { kind: "question"; question: string; options: string[] };
export type CardState =
  /** `notice`: the core refused the answer (409) and the request is still waiting. */
  | { phase: "pending"; notice?: string }
  /** The request left before the answer reached it. */
  | { phase: "gone"; message: string }
  | { phase: "decided"; verdict: "allowed" | "denied" }
  | { phase: "terminal" }
  | { phase: "released"; afterMs: number };
export type ApprovalRequest = {
  id: string;
  /** `number` is the session's number, shown before its name ("3 · infra"); fixtures that predate numbering leave it out. */
  session: { name: string; agent: AgentId; number?: number };
  sessionCwd: string;
  waitingMs: number;
  detail: ToolDetail;
  risk: Risk;
  /** The exact rule "Always allow" would add, e.g. "Bash(npm test:*)". Absent when no rule can be offered. */
  alwaysRule?: string;
  state: CardState;
  /** The session's tab, so the card can jump to it and the badge can tell muted sessions. */
  tab: string;
  /** A session needing you with no held request (the CLI asks in its own terminal): a one-liner that jumps to the session, never the full card. */
  terminalOnly?: boolean;
  /** An answer is on its way: the card is inert. */
  busy?: boolean;
  /** One-shot: the receipt is leaving (140 ms). */
  leaving?: boolean;
};
