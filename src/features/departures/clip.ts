import { flipFrame } from "./flip";

/** The 10 second motion clip: only the status and waiting cells ever flip, rows never change group or order. */
export const CLIP_DURATION_MS = 10_000;

export type ClipEvent = { at: number; rowId: string; cell: "status" | "waiting"; from: string; to: string };

export const CLIP_EVENTS: ClipEvent[] = [
  { at: 1_000, rowId: "working", cell: "status", from: "WORKING", to: "NEEDS YOU" },
  { at: 1_000, rowId: "working", cell: "waiting", from: "–", to: "<1 MIN" },
  { at: 3_200, rowId: "needs-permission", cell: "waiting", from: "1 MIN", to: "2 MIN" },
  { at: 5_000, rowId: "error", cell: "status", from: "ERROR", to: "WORKING" },
  { at: 6_500, rowId: "needs-question", cell: "waiting", from: "4 MIN", to: "5 MIN" },
  { at: 8_000, rowId: "working", cell: "status", from: "NEEDS YOU", to: "WORKING" },
  { at: 8_000, rowId: "working", cell: "waiting", from: "<1 MIN", to: "–" },
];

/** The text of a cell at time t: the baseline until its first event, then each event's flip in turn. */
export function cellAt(rowId: string, cell: "status" | "waiting", baseline: string, t: number): string {
  let text = baseline;
  for (const e of CLIP_EVENTS) if (e.rowId === rowId && e.cell === cell && t >= e.at) text = flipFrame(e.from, e.to, t - e.at);
  return text;
}

/**
 * One cell's whole clip as frames, for a cell that flips more than once (the review page plays them from the start of the
 * clip). It lists a frame only where the text changes, so a still cell is just its baseline.
 */
export function clipTrack(rowId: string, cell: "status" | "waiting", baseline: string, stepMs = 16): Array<{ t: number; text: string }> {
  const track: Array<{ t: number; text: string }> = [{ t: 0, text: baseline }];
  for (let t = stepMs; t <= CLIP_DURATION_MS; t += stepMs) {
    const text = cellAt(rowId, cell, baseline, t);
    if (text !== track[track.length - 1]!.text) track.push({ t, text });
  }
  const end = cellAt(rowId, cell, baseline, CLIP_DURATION_MS);
  if (end !== track[track.length - 1]!.text) track.push({ t: CLIP_DURATION_MS, text: end });
  return track;
}
