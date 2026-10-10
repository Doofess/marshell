import type { AgentId } from "../../components/AgentMark/agents";
import type { GlyphKind } from "../../components/StatusGlyph/glyphs";

export type { AgentId };
export type Density = "comfortable" | "expanded";
export type Mode = "manual" | "plan" | "auto-edit" | "full-auto" | "bypass";
export type Usage = { inContext?: number; window?: number; tokensIn?: number; tokensOut?: number; costUsd?: number };

/** Phase 0 view model for one sidebar row; phase 3 maps the core's generated session types onto it. */
export type RowModel = {
  id: string;
  name: string;
  project: string;
  branch?: string;
  agent: AgentId;
  status: GlyphKind;
  /** Line 2, e.g. "Wants to run `npm test`". */
  phrase: string;
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
};
