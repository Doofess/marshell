import type { RecordingPart } from "../../../generated/RecordingPart";

/** How long to wait before an event that came `gapMs` after the one before: the gap, capped by the idle limit, at the speed. */
export function playbackDelay(gapMs: number, speed: number, idleCapMs: number | null): number {
  const gap = Math.max(0, gapMs);
  return (idleCapMs === null ? gap : Math.min(gap, idleCapMs)) / speed;
}

/** Two jump targets closer than this are one. */
const MERGE_MS = 500;
/** "Previous" from just after a target goes to the one before it, not back to the same one. */
const BACK_SKIP_MS = 1_000;

/** The prompt marks plus the turn times inside the part, sorted, one per 500 ms. */
export function jumpTargets(part: RecordingPart, turnTimes: number[]): number[] {
  const all = [...part.marks_ms, ...turnTimes.filter((t) => t >= part.started_ms && t <= part.ended_ms)].sort((a, b) => a - b);
  const out: number[] = [];
  for (const t of all) {
    const last = out[out.length - 1];
    if (last === undefined || t - last >= MERGE_MS) out.push(t);
  }
  return out;
}

/** The target after `posMs` (by 1), or the one more than a second before it (by -1); null when there is none. */
export function nextTarget(targets: number[], posMs: number, by: -1 | 1): number | null {
  if (by === 1) return targets.find((t) => t > posMs) ?? null;
  return [...targets].reverse().find((t) => t <= posMs - BACK_SKIP_MS) ?? null;
}

/** `m:ss`, or `h:mm:ss` when the recording (`totalMs`) is an hour or longer. */
export function clock(ms: number, totalMs: number): string {
  const s = Math.floor(Math.max(0, ms) / 1000);
  const two = (n: number) => String(n).padStart(2, "0");
  const sec = two(s % 60);
  if (totalMs >= 3_600_000 || s >= 3_600) return `${Math.floor(s / 3600)}:${two(Math.floor(s / 60) % 60)}:${sec}`;
  return `${Math.floor(s / 60)}:${sec}`;
}
