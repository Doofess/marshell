/**
 * Splits a branch name for middle truncation: the head may be cut with an ellipsis, the tail always shows.
 * Tail = the last segment after the final "-", "/" or "_" (with its separator) if that is at most 8 characters,
 * else the last 6 characters. Works on code points so emoji are never split.
 */
export function splitForMiddle(s: string): { head: string; tail: string } {
  const cps = [...s];
  if (cps.length <= 8) return { head: "", tail: s };
  let sep = -1;
  for (let i = cps.length - 1; i >= 0; i--) {
    if (cps[i] === "-" || cps[i] === "/" || cps[i] === "_") {
      sep = i;
      break;
    }
  }
  const cut = sep > 0 && cps.length - sep <= 8 ? sep : cps.length - 6;
  return { head: cps.slice(0, cut).join(""), tail: cps.slice(cut).join("") };
}

/** True for code points that terminals and most UI fonts draw two columns wide (CJK, Hangul, fullwidth, emoji). */
function isWide(cp: number): boolean {
  return (
    (cp >= 0x1100 && cp <= 0x115f) ||
    (cp >= 0x2e80 && cp <= 0xa4cf) ||
    (cp >= 0xac00 && cp <= 0xd7a3) ||
    (cp >= 0xf900 && cp <= 0xfaff) ||
    (cp >= 0xfe30 && cp <= 0xfe4f) ||
    (cp >= 0xff00 && cp <= 0xff60) ||
    (cp >= 0xffe0 && cp <= 0xffe6) ||
    (cp >= 0x1f300 && cp <= 0x1faff) ||
    (cp >= 0x20000 && cp <= 0x3fffd)
  );
}

/** Approximate width in columns: wide characters count 2. Decides whether a name is short enough never to shrink. */
export function displayWidth(s: string): number {
  let w = 0;
  for (const ch of s) w += isWide(ch.codePointAt(0)!) ? 2 : 1;
  return w;
}
