import { AGENT_NAMES, isAgentId, type AgentId } from "./agents";
import { TOOL_NAMES, isToolId, type ToolId } from "./tools";

/**
 * What a session runs: an AI agent (it thinks, asks, and has a prompt line) or a manual CLI (a plain terminal).
 * `RowModel.agent` carries either, because every surface already draws a mark from it and a vendor-coloured accent;
 * `isTool` is the one test for "plain terminal: no animation, no ring, no model, no prompt line".
 */
export type SessionKind = AgentId | ToolId;

export const isTool = (kind: string): kind is ToolId => isToolId(kind);
export const isKind = (v: string): v is SessionKind => isAgentId(v) || isToolId(v);

/** The name a screen reader and a tooltip use for the mark. */
export function kindName(kind: SessionKind, own?: string): string {
  if (kind === "generic" && own) return own;
  return isToolId(kind) ? TOOL_NAMES[kind] : AGENT_NAMES[kind];
}
