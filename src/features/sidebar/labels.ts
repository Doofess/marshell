import { AGENT_NAMES } from "../../components/AgentMark/agents";
import { GLYPHS } from "../../components/StatusGlyph/glyphs";
import { formatDuration, modeCaution } from "./format";
import type { RowModel } from "./types";

/** Strips Markdown code ticks so screen readers don't read "backtick". */
const plain = (s: string) => s.replace(/`/g, "");

/** "Claude Opus 5.5", or just "Claude" when the CLI doesn't report a model. */
const agentAndModel = (r: RowModel) => [AGENT_NAMES[r.agent], r.model].filter(Boolean).join(" ");

export function rowLabel(r: RowModel): string {
  const parts = [`${r.name}, ${agentAndModel(r)}, ${GLYPHS[r.status].label}.`, `${plain(r.phrase)}.`];
  parts.push(r.branch ? `${r.project}, branch ${r.branch}.` : `${r.project}.`);
  if (r.elevated) parts.push("Administrator.");
  const caution = modeCaution(r.mode);
  if (caution) parts.push(`${caution}.`);
  if (r.muted) parts.push("Muted.");
  if (r.waitingMs !== undefined) parts.push(`Waiting ${formatDuration(r.waitingMs)}.`);
  if (r.contextPct !== undefined && Number.isFinite(r.contextPct))
    parts.push(`Context ${Math.round(Math.min(100, Math.max(0, r.contextPct)))}% used.`);
  return parts.join(" ");
}

export function rowTitle(r: RowModel): string {
  return [r.name, r.branch ? `${r.project} · ${r.branch}` : r.project, agentAndModel(r), r.phrase].join("\n");
}
