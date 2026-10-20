import type { AgentId } from "../../components/AgentMark/agents";
import type { SessionKind } from "../../components/AgentMark/kinds";
import type { GlyphKind } from "../../components/StatusGlyph/glyphs";

export type { AgentId, SessionKind };
export type Density = "comfortable" | "expanded";
/** The five modes Marshell offers, and "dont-ask", a mode Claude Code can report that Marshell never offers. */
export type Mode = "manual" | "plan" | "auto-edit" | "full-auto" | "bypass" | "dont-ask";
export type Usage = { inContext?: number; window?: number; tokensIn?: number; tokensOut?: number; costUsd?: number };

/** Phase 0 view model for one sidebar row; phase 3 maps the core's generated session types onto it. */
export type RowModel = {
  id: string;
  /** The session's number: 1-based, stable until it closes, the lowest free one when a session starts (sessionNumber.ts). */
  number: number;
  name: string;
  /** The label: the last folder of the repository (or of the folder, outside git). */
  project: string;
  /** What groups rows: the repository root, so two repositories that share a folder name stay apart. */
  projectKey: string;
  branch?: string;
  /** The session runs in a task's worktree: the row says "Task" and offers its review. Set by the window from the core's task list. */
  task?: boolean;
  /** An AI agent, or a manual CLI (a plain terminal: no animation, ring, model or prompt line). */
  agent: SessionKind;
  status: GlyphKind;
  /** Line 2, e.g. "Wants to run `npm test`". */
  phrase: string;
  /** The row offers "set up": a Claude Code session whose hooks are not installed, so its status is limited. */
  setupLink?: boolean;
  /** Said in the row's tooltip, not on its lines: why a Working or Idle row is only as sure as the pty's output (no live status). */
  note?: string;
  /** The core's reason for the limited status, when it gave one: the accessible name says it, since the tooltip is not read. */
  statusNote?: string;
  /** Needs-you only: how long it has waited. */
  waitingMs?: number;
  /** Done or ended: time since. */
  ageMs?: number;
  /** Undefined hides the ring. */
  contextPct?: number;
  muted?: boolean;
  elevated?: boolean;
  mode?: Mode;
  model?: string;
  effort?: string;
  subagents?: number;
  recap?: { text: string; ageMs: number };
  usage?: Usage;
  /** One-shot attention flash on the row. */
  flash?: boolean;
  /** The prompt bar's status strip: connected MCP servers, and a named problem when one is failing. Unknown leaves the item out. */
  mcp?: { connected: number; total: number; problem?: string };
  /** Skills available to the session. */
  skills?: number;
  /** The active Python environment's name. */
  venv?: string;
  /** How long the session has been running. */
  sessionMs?: number;
  /** Uncommitted changes in the session's working tree. */
  dirty?: number;
  /** The CLI's version, e.g. "2.1.296". */
  version?: string;
};
