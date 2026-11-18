/** The most lines either side of the middle may have before the middle is marked whole. */
const LCS_LIMIT = 2000;

/**
 * The 1-based numbers of the lines of `after` that are new or different.
 * The common head and tail are skipped, then a longest common subsequence
 * of the middle decides which lines stayed. A removed line marks the line that
 * now sits where it was (the last line, if it was at the end).
 */
export function changedLines(before: string[], after: string[]): number[] {
  if (after.length === 0) return [];
  let head = 0;
  while (head < before.length && head < after.length && before[head] === after[head]) head++;
  let tail = 0;
  while (
    tail < before.length - head &&
    tail < after.length - head &&
    before[before.length - 1 - tail] === after[after.length - 1 - tail]
  ) {
    tail++;
  }
  const b = before.slice(head, before.length - tail);
  const a = after.slice(head, after.length - tail);
  if (b.length === 0 && a.length === 0) return [];

  const marked = new Set<number>();
  // Where a removal sits in `after`: the line now at that place, or the last line.
  const markPlace = (middleIndex: number) => marked.add(Math.min(head + middleIndex, after.length - 1));

  if (a.length === 0) {
    markPlace(0);
  } else if (b.length > LCS_LIMIT || a.length > LCS_LIMIT) {
    for (let j = 0; j < a.length; j++) marked.add(head + j);
  } else {
    const n = b.length;
    const m = a.length;
    const w = m + 1;
    // lcs[i * w + j]: the longest common subsequence of b[i..] and a[j..].
    const lcs = new Uint16Array((n + 1) * w);
    for (let i = n - 1; i >= 0; i--) {
      for (let j = m - 1; j >= 0; j--) {
        lcs[i * w + j] = b[i] === a[j] ? lcs[(i + 1) * w + j + 1]! + 1 : Math.max(lcs[(i + 1) * w + j]!, lcs[i * w + j + 1]!);
      }
    }
    let i = 0;
    let j = 0;
    while (i < n && j < m) {
      if (b[i] === a[j]) {
        i++;
        j++;
      } else if (lcs[(i + 1) * w + j]! >= lcs[i * w + j + 1]!) {
        markPlace(j);
        i++;
      } else {
        marked.add(head + j);
        j++;
      }
    }
    if (i < n) markPlace(j);
    for (; j < m; j++) marked.add(head + j);
  }
  return [...marked].sort((x, y) => x - y).map((i) => i + 1);
}
