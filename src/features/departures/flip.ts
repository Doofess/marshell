/** Split-flap schedule: 60 ms per character, 15 ms stagger, the whole flip capped at 400 ms. Pure, so tests and the clip read the same frames. */
export const FLIP_CHAR_MS = 60;
export const FLIP_STAGGER_MS = 15;
export const FLIP_CAP_MS = 400;

const GLYPHS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/** When the character at index i has settled on its final letter. */
export const settleAt = (i: number): number => FLIP_CHAR_MS + Math.min(i * FLIP_STAGGER_MS, FLIP_CAP_MS - FLIP_CHAR_MS);

export const flipDuration = (len: number): number => (len <= 0 ? 0 : settleAt(len - 1));

export type FlipRun = { from: string; to: string; start: number };

/** A new target starts at the current frame, even when the previous flip has not settled. */
export function retarget(run: FlipRun | null, shown: string, to: string, now: number): FlipRun | null {
  if (!run && to === shown) return null;
  if (run && to === run.to) return run;
  return { from: run ? flipFrame(run.from, run.to, now - run.start) : shown, to, start: now };
}

export function flipFrame(from: string, to: string, t: number): string {
  const n = Math.max(from.length, to.length);
  const f = from.padEnd(n);
  const g = to.padEnd(n);
  if (t >= flipDuration(n)) return to;
  let out = "";
  for (let i = 0; i < n; i++) {
    // A character that is the same in both texts does not flip: only what changes moves.
    if (f[i] === g[i]) out += g[i];
    else if (t >= settleAt(i)) out += g[i];
    else if (t <= i * FLIP_STAGGER_MS) out += f[i];
    else out += GLYPHS[(i * 7 + Math.floor((t - i * FLIP_STAGGER_MS) / (FLIP_CHAR_MS / 4)) * 5) % GLYPHS.length];
  }
  return out.replace(/ +$/, "");
}

export function flipFrames(from: string, to: string, stepMs = 16): Array<{ t: number; text: string }> {
  const end = flipDuration(Math.max(from.length, to.length));
  const out: Array<{ t: number; text: string }> = [];
  for (let t = 0; t < end; t += stepMs) out.push({ t, text: flipFrame(from, to, t) });
  out.push({ t: end, text: to });
  return out;
}
