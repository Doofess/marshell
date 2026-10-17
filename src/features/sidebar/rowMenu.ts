import type { MenuItem } from "../../components/ContextMenu/menuKeys";

/** True for a target inside the rename text box, which keeps its own keys and its own native context menu. */
export function inRenameBox(target: object | null): boolean {
  // The target is any DOM node here; only elements have `closest`.
  return Boolean((target as { closest?: (s: string) => unknown } | null)?.closest?.(".session-row__rename"));
}

/** Shift+F10 and the Menu key open a row's menu, unless the key is meant for the rename text box. */
export function opensRowMenu(e: { key: string; shiftKey: boolean; ctrlKey: boolean; altKey: boolean; metaKey: boolean; target: object | null }): boolean {
  if (inRenameBox(e.target)) return false;
  if (e.key === "ContextMenu") return true;
  return e.key === "F10" && e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey;
}

/** Where a keyboard-opened menu appears: under the row, a little in from its left edge. */
export function keyboardAnchor(rect: { left: number; bottom: number }): { x: number; y: number } {
  return { x: rect.left + 8, y: rect.bottom };
}

/** An open menu belongs to a row. Once that row is gone from the list, the menu must close rather than act on it. */
export function menuStillValid(openId: string | null, ids: readonly string[]): boolean {
  return openId !== null && ids.includes(openId);
}

/** The menu's items are captured when it opens; it is stale once the row's live items differ (a resume or an exit changes them). Compared by id, in order, since the objects are new on every call. */
export function sameMenuItems(a: readonly MenuItem[], b: readonly MenuItem[]): boolean {
  const key = (i: MenuItem) => ("separator" in i ? "-" : i.id);
  return a.length === b.length && a.every((it, n) => key(it) === key(b[n]!));
}
