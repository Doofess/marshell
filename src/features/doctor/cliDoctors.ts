import type { DoctorReport } from "./types";

const CONTACTS = "Codex's doctor contacts OpenAI.";

/** How long ago `atMs` was, in whole seconds, or minutes from 60 s: `10 s`, `2 min`. */
export function agoText(atMs: number, nowMs: number): string {
  const s = Math.max(0, Math.round((nowMs - atMs) / 1000));
  return s >= 60 ? `${Math.floor(s / 60)} min` : `${s} s`;
}

/** The status line under the summary: when the CLI doctors last ran, counted the way the header counts `Checked`. */
export function cliLine(checkedAtMs: number | undefined, nowMs: number): string {
  if (checkedAtMs === undefined) return `CLI doctors haven't run. ${CONTACTS}`;
  return `CLI doctors ran ${agoText(checkedAtMs, nowMs)} ago. ${CONTACTS}`;
}

/** Whether the report shows a CLI doctors' run newer than `base`, the one the report had when the run was asked for. */
export const cliFinishedSince = (base: number | undefined, at: number | undefined): boolean =>
  at !== undefined && (base === undefined || at > base);

/** What the announcer says when the CLI doctors finish: the Broken and Waste items they added. */
export function cliDoneAnnouncement(report: DoctorReport): string {
  const n = report.items.filter((i) => i.id.startsWith("cli.") && (i.group === "broken" || i.group === "waste")).length;
  return n === 0 ? "The CLI doctors found nothing." : `The CLI doctors found ${n} ${n === 1 ? "issue" : "issues"}.`;
}
