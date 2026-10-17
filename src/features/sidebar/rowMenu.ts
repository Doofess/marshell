/** Shift+F10 and the Menu key open a row's menu, unless the key is meant for the rename text box. */
export function opensRowMenu(e: { key: string; shiftKey: boolean; ctrlKey: boolean; altKey: boolean; metaKey: boolean; target: object | null }): boolean {
  // The target is any DOM node here; only elements have `closest`.
  if ((e.target as { closest?: (s: string) => unknown } | null)?.closest?.(".session-row__rename")) return false;
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
