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
