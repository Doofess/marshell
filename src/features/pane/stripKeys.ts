/** What a key on a focused tab does. `rtl` swaps Left and Right. */
export type StripKey = { type: "select"; id: string } | { type: "close"; id: string } | { type: "move"; id: string; to: number } | null;

export function stripKey(
  e: { key: string; altKey: boolean; shiftKey: boolean; ctrlKey: boolean; metaKey: boolean },
  ids: readonly string[],
  focused: string,
  rtl: boolean,
): StripKey {
  const at = ids.indexOf(focused);
  if (at < 0 || e.ctrlKey || e.metaKey) return null;
  const n = ids.length;
  const plain = !e.altKey && !e.shiftKey;
  if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
    const step = (e.key === "ArrowRight") !== rtl ? 1 : -1;
    if (plain) return { type: "select", id: ids[(at + step + n) % n]! };
    if (e.altKey && e.shiftKey) {
      const to = at + step;
      return to < 0 || to >= n ? null : { type: "move", id: focused, to };
    }
    return null;
  }
  if (!plain) return null;
  if (e.key === "Home") return { type: "select", id: ids[0]! };
  if (e.key === "End") return { type: "select", id: ids[n - 1]! };
  if (e.key === "Delete") return { type: "close", id: focused };
  return null;
}

/** The index a dragged tab should take: the tab whose middle the pointer has passed. */
export function dragIndex(tabs: ReadonlyArray<{ left: number; right: number }>, x: number, from: number): number {
  const mid = (i: number) => (tabs[i]!.left + tabs[i]!.right) / 2;
  // The pointer has passed neighbour `j` of `i` when it is beyond j's middle, on the side away from i.
  const passed = (i: number, j: number) => (mid(j) >= mid(i) ? x > mid(j) : x < mid(j));
  let at = from;
  while (at + 1 < tabs.length && passed(at, at + 1)) at++;
  if (at !== from) return at;
  while (at - 1 >= 0 && passed(at, at - 1)) at--;
  return at;
}
