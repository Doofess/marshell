import { kindName } from "../../components/AgentMark/kinds";
import { GLYPHS } from "../../components/StatusGlyph/glyphs";
import { formatDuration, formatUsage, modeCaution } from "./format";
import { sessionWord } from "./sessionNumber";
import type { Density, RowModel } from "./types";

const MINUTE = 60_000;

/** Strips Markdown code ticks so screen readers don't read "backtick". */
const plain = (s: string) => s.replace(/`/g, "");

/** "Claude Opus 5.5", or just "Claude" when the CLI doesn't report a model. */
const agentAndModel = (r: RowModel) => [kindName(r.agent), r.model].filter(Boolean).join(" ");

/** "my-app · main", with the Task tag right after the project when the row shows it ("my-app Task · marshell/fix-login"), as it is drawn. */
const projectAndBranch = (r: RowModel) => {
  const project = r.task ? `${r.project} Task` : r.project;
  return r.branch ? `${project} · ${r.branch}` : project;
};

/**
 * The row's accessible name. It begins with the session's number ("Session 6", the words a person says to an agent),
 * then the text drawn in the row, in the order it is drawn, so the name always contains the visible label (WCAG 2.5.3,
 * and a voice-control user can say what they see); what only the glyphs say (the agent, the state, the mode, muted,
 * the context level) follows.
 */
export function rowLabel(r: RowModel, density: Density = "comfortable"): string {
  const aged = r.waitingMs ?? (r.status === "done-unseen" || r.status === "done-seen" || r.status === "ended" ? r.ageMs : undefined);
  // Under a minute the age ticks every second, which would re-announce a focused row each time: the name leaves it out.
  const time = aged !== undefined && aged >= MINUTE ? aged : undefined;
  const phrase = `${plain(r.phrase)}${r.muted ? " \u00b7 muted" : ""}`;
  const seen: Array<string | null | undefined> = [
    r.name,
    projectAndBranch(r),
    time !== undefined ? formatDuration(time) : null,
    r.model && phrase ? `${r.model} \u00b7 ${phrase}` : r.model || phrase,
  ];
  const caution = modeCaution(r.mode);
  if (density === "expanded") {
    seen.push(r.model, r.effort, r.mode === "plan" ? "Plan" : null, caution, r.subagents !== undefined ? `${r.subagents} subagents` : null);
    if (r.recap) seen.push(r.phrase.includes(r.recap.text) ? null : r.recap.text, r.recap.ageMs >= MINUTE ? `${formatDuration(r.recap.ageMs)} ago` : null);
    seen.push(formatUsage(r.usage));
  }
  const parts = [`${sessionWord(r.number)}, ${seen.filter(Boolean).join(" ")}.`, `${agentAndModel(r)}, ${GLYPHS[r.status].label}.`];
  if (r.elevated) parts.push("Administrator.");
  if (caution) parts.push(`${caution}.`);
  if (r.muted) parts.push("Muted.");
  if (r.statusNote) parts.push(`Status limited: ${r.statusNote}.`);
  if (r.contextPct !== undefined && Number.isFinite(r.contextPct))
    parts.push(`Context ${Math.round(Math.min(100, Math.max(0, r.contextPct)))}% used.`);
  return parts.join(" ");
}

export function rowTitle(r: RowModel): string {
  return [sessionWord(r.number), r.name, projectAndBranch(r), agentAndModel(r), r.phrase, ...(r.note ? [r.note] : [])].join("\n");
}
