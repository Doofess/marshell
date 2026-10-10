/** Height of the "+N more" button that closes a list the main window could not show in full. */
export const MORE_HEIGHT = 36;

/**
 * The main window never scrolls, so a list shows the rows that fit and one "+N more" button for the rest. The button
 * takes a 36 px slot only when something is left out.
 */
export function listLayout(count: number, areaHeight: number, rowHeight: number, moreHeight: number = MORE_HEIGHT): { visible: number; hidden: number } {
  const n = Number.isFinite(count) ? Math.max(0, Math.floor(count)) : 0;
  const area = Number.isFinite(areaHeight) ? areaHeight : 0;
  if (n === 0) return { visible: 0, hidden: 0 };
  if (!(rowHeight > 0) || area <= 0) return { visible: 0, hidden: n };
  if (n * rowHeight <= area) return { visible: n, hidden: 0 };
  const visible = Math.max(0, Math.min(n - 1, Math.floor((area - moreHeight) / rowHeight)));
  return { visible, hidden: n - visible };
}
