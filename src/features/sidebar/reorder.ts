/** The order after moving `id` one place (-1 up, +1 down), or null when it is at the end, unknown, or nothing would change. */
export function moveBy(ids: string[], id: string, by: -1 | 1): string[] | null {
  const from = ids.indexOf(id);
  const to = from + by;
  if (from < 0 || to < 0 || to >= ids.length) return null;
  const next = [...ids];
  next[from] = next[to]!;
  next[to] = id;
  return next;
}

/** The order after dropping `dragId` before or after the row `overId`, or null when the order would stay the same. */
export function dropOrder(ids: string[], dragId: string, overId: string, edge: "before" | "after"): string[] | null {
  if (dragId === overId || !ids.includes(dragId) || !ids.includes(overId)) return null;
  const rest = ids.filter((i) => i !== dragId);
  const at = rest.indexOf(overId) + (edge === "after" ? 1 : 0);
  const next = [...rest.slice(0, at), dragId, ...rest.slice(at)];
  return next.every((id, i) => id === ids[i]) ? null : next;
}
