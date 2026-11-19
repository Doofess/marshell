/** The list's thumbnail buttons by page, read with one query: a query per page made each drawing step quadratic in the page count. */
export function thumbButtons(list: ParentNode): (n: number) => HTMLElement | null {
  const byPage = new Map<number, HTMLElement>();
  list.querySelectorAll<HTMLElement>("[data-thumb]").forEach((b) => byPage.set(Number(b.dataset.thumb), b));
  return (n) => byPage.get(n) ?? null;
}

/** The wanted page that isn't drawn yet and lies nearest to `current`; of two equally near, the later. Null when none is left. */
export function nextToDraw(wanted: ReadonlySet<number>, drawn: ReadonlySet<number>, current: number): number | null {
  let best: number | null = null;
  for (const n of wanted) {
    if (drawn.has(n)) continue;
    if (best === null) {
      best = n;
      continue;
    }
    const d = Math.abs(n - current);
    const bd = Math.abs(best - current);
    if (d < bd || (d === bd && n > best)) best = n;
  }
  return best;
}

/**
 * Where a key takes the list's stop: `at` is a zero-based index in a list of `count`. Up and Down move one, Page Up and Page Down
 * a view (`perView` whole thumbnails, at least one), Home and End go to the ends. Null for another key, or when the stop wouldn't move.
 */
export function listKey(at: number, count: number, key: string, perView: number): number | null {
  if (count <= 0) return null;
  const view = Math.max(1, Math.floor(perView));
  const last = count - 1;
  const to = key === "ArrowUp" ? at - 1 : key === "ArrowDown" ? at + 1 : key === "PageUp" ? at - view : key === "PageDown" ? at + view : key === "Home" ? 0 : key === "End" ? last : null;
  if (to === null) return null;
  const clamped = Math.min(last, Math.max(0, to));
  return clamped === at ? null : clamped;
}
