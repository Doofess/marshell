import { AGENT_NAMES } from "../../components/AgentMark/agents";
import { GLYPHS } from "../../components/StatusGlyph/glyphs";
import { formatDuration, formatUsage, modeCaution } from "./format";
import type { Density, RowModel } from "./types";

/** Strips Markdown code ticks so screen readers don't read "backtick". */
const plain = (s: string) => s.replace(/`/g, "");

/** "Claude Opus 5.5", or just "Claude" when the CLI doesn't report a model. */
const agentAndModel = (r: RowModel) => [AGENT_NAMES[r.agent], r.model].filter(Boolean).join(" ");

/**
 * The row's accessible name. It starts with the text drawn in the row, in the order it is drawn, so the name always
 * contains the visible label (WCAG 2.5.3, and a voice-control user can say what they see); what only the glyphs say
 * (the agent, the state, the mode, muted, the context level) follows.
 */
export function rowLabel(r: RowModel, density: Density = "comfortable"): string {
  const time = r.waitingMs ?? (r.status === "done-unseen" || r.status === "done-seen" || r.status === "ended" ? r.ageMs : undefined);
  const phrase = `${plain(r.phrase)}${r.muted ? " \u00b7 muted" : ""}`;
  const seen: Array<string | null | undefined> = [
    r.name,
    r.branch ? `${r.project} \u00b7 ${r.branch}` : r.project,
    time !== undefined ? formatDuration(time) : null,
    r.model ? `${r.model} \u00b7 ${phrase}` : phrase,
  ];
  const caution = modeCaution(r.mode);
  if (density === "expanded") {
    seen.push(r.model, r.effort, r.mode === "plan" ? "Plan" : null, caution, r.subagents !== undefined ? `${r.subagents} subagents` : null);
    if (r.recap) seen.push(r.recap.text, `${formatDuration(r.recap.ageMs)} ago`);
    seen.push(formatUsage(r.usage));
  }
  const parts = [`${seen.filter(Boolean).join(" ")}.`, `${agentAndModel(r)}, ${GLYPHS[r.status].label}.`];
  if (r.elevated) parts.push("Administrator.");
  if (caution) parts.push(`${caution}.`);
  if (r.muted) parts.push("Muted.");
  if (r.contextPct !== undefined && Number.isFinite(r.contextPct))
    parts.push(`Context ${Math.round(Math.min(100, Math.max(0, r.contextPct)))}% used.`);
  return parts.join(" ");
}

export function rowTitle(r: RowModel): string {
  return [r.name, r.branch ? `${r.project} · ${r.branch}` : r.project, agentAndModel(r), r.phrase].join("\n");
}
