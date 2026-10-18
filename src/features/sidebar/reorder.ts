/** How far (px) the pointer must move from the press before it is a drag and not a click. */
export const DRAG_THRESHOLD = 4;

/** True once the pointer is `DRAG_THRESHOLD` px or more from where it was pressed. */
export function dragStarted(dx: number, dy: number): boolean {
  return Math.hypot(dx, dy) >= DRAG_THRESHOLD;
}

/** Which half of a row the pointer is in: the top half drops before it, the bottom half after. */
export function edgeAt(clientY: number, top: number, height: number): "before" | "after" {
  return clientY < top + height / 2 ? "before" : "after";
}

/** The session id of the row under an element (the closest `[data-row-id]`), or null when it is not in a row. */
export function rowIdAt(el: Element | null): string | null {
  return el?.closest("[data-row-id]")?.getAttribute("data-row-id") ?? null;
}

/** A row being dragged, and the edge of the row it is over (where it would land). */
export type DragState = { id: string; over?: { id: string; edge: "before" | "after" } };

/** The drag after the pointer moves over `overId`: no indicator on the dragged row itself, which would not move. */
export function dragOver(drag: DragState, overId: string, edge: "before" | "after"): DragState {
  return overId === drag.id ? { id: drag.id } : { id: drag.id, over: { id: overId, edge } };
}

/** A stretch of the list a move must stay inside (a project group): `start` is its first index, `end` one past its last. */
export type Bounds = { start: number; end: number };

/** The order after moving `id` one place (-1 up, +1 down), or null when it is at the end (of `bounds`, when given), unknown, or nothing would change. */
export function moveBy(ids: string[], id: string, by: -1 | 1, bounds?: Bounds): string[] | null {
  const from = ids.indexOf(id);
  const to = from + by;
  const { start, end } = bounds ?? { start: 0, end: ids.length };
  if (from < start || from >= end || to < start || to >= end) return null;
  const next = [...ids];
  next[from] = next[to]!;
  next[to] = id;
  return next;
}

/** The order after dropping `dragId` before or after the row `overId`, or null when the order would stay the same (or either row is outside `bounds`). */
export function dropOrder(ids: string[], dragId: string, overId: string, edge: "before" | "after", bounds?: Bounds): string[] | null {
  if (dragId === overId || !ids.includes(dragId) || !ids.includes(overId)) return null;
  if (bounds && ![dragId, overId].every((id) => ids.indexOf(id) >= bounds.start && ids.indexOf(id) < bounds.end)) return null;
  const rest = ids.filter((i) => i !== dragId);
  const at = rest.indexOf(overId) + (edge === "after" ? 1 : 0);
  const next = [...rest.slice(0, at), dragId, ...rest.slice(at)];
  return next.every((id, i) => id === ids[i]) ? null : next;
}
