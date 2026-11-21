/**
 * One row of a context menu: an action, or a rule between groups.
 * `disabled` is the reason the row cannot run: it is still listed and reachable, so the reason can be read, but it never runs.
 */
export type MenuRow = {
  id: string;
  label: string;
  shortcut?: string;
  disabled?: string;
  /** Present on a row that is on or off: `menuitemcheckbox`, or `menuitemradio` with `radio`. */
  checked?: boolean;
  radio?: true;
  /** A line under the label that is always shown; the row still runs. */
  note?: string;
  /** With `rows`, the row opens a submenu of them; `submenu: true` alone is the app menu's own marker and stays. */
  submenu?: true;
  rows?: MenuItem[];
};
export type MenuItem = MenuRow | { separator: true };

export type MenuKeyResult =
  | { kind: "move"; index: number }
  | { kind: "run"; id: string }
  | { kind: "close"; refocus: boolean }
  | { kind: "open"; id: string }
  | { kind: "back" }
  | null;

export const isAction = (i: MenuItem): i is MenuRow => !("separator" in i);

/** True for a row that opens a submenu: the app menu's `submenu` marker, or a row that carries `rows`. */
export const opensSubmenu = (i: MenuRow): boolean => i.submenu === true || i.rows !== undefined;

/** The next action index from `from` in direction `dir`, wrapping and stepping over separators; -1 if there are none. */
function step(items: MenuItem[], from: number, dir: 1 | -1): number {
  const n = items.length;
  for (let k = 1; k <= n; k++) {
    const i = (((from + dir * k) % n) + n) % n;
    if (isAction(items[i]!)) return i;
  }
  return -1;
}

/** The modifiers of a key press that make it a shortcut rather than a letter to search for. */
type Mods = { ctrlKey?: boolean; altKey?: boolean; metaKey?: boolean };

/** What a key does in an open menu. Separators are never active, so no result can land on one. */
export function menuKey(key: string, items: MenuItem[], active: number, mods: Mods = {}): MenuKeyResult {
  const move = (index: number): MenuKeyResult => (index < 0 ? null : { kind: "move", index });
  switch (key) {
    case "ArrowDown":
      return move(step(items, active, 1));
    case "ArrowUp":
      return move(step(items, active, -1));
    case "Home":
      return move(step(items, -1, 1));
    case "End":
      return move(step(items, items.length, -1));
    case "Enter":
    case " ": {
      const it = items[active];
      if (!it || !isAction(it) || it.disabled !== undefined) return null;
      return opensSubmenu(it) ? { kind: "open", id: it.id } : { kind: "run", id: it.id };
    }
    case "ArrowRight": {
      const it = items[active];
      return it && isAction(it) && opensSubmenu(it) && it.disabled === undefined ? { kind: "open", id: it.id } : null;
    }
    case "ArrowLeft":
      return { kind: "back" };
    case "Escape":
      return { kind: "close", refocus: true };
    case "Tab":
      return { kind: "close", refocus: false };
  }
  // Ctrl, Alt and Meta make a letter a shortcut (the menu shows some), not a search.
  if (key.length !== 1 || mods.ctrlKey || mods.altKey || mods.metaKey) return null;
  // Type-ahead: search from the item after the active one, wrapping, so repeating a letter cycles.
  const c = key.toLowerCase();
  const n = items.length;
  for (let k = 1; k <= n; k++) {
    const i = (active + k) % n;
    const it = items[i]!;
    if (isAction(it) && it.label.toLowerCase().startsWith(c)) return { kind: "move", index: i };
  }
  return null;
}

/** True for a target inside an open context menu, which owns its keys: the window's capture-phase key listeners skip it. */
export function inMenu(target: object | null): boolean {
  // The target is any DOM node here; only elements have `closest`.
  return Boolean((target as { closest?: (s: string) => unknown } | null)?.closest?.('[role="menu"]'));
}

/** Focus moved to something outside the menu. No target (the window lost focus, an item unmounted) leaves it open: a click away is closed by its pointerdown. */
export function leavesMenu(next: object | null, menu: { contains: (n: never) => boolean }): boolean {
  return next !== null && !menu.contains(next as never);
}

/** Where to put a menu so it stays fully on screen: at the anchor, pulled back from the right and bottom edges. */
export function placeMenu(
  anchor: { x: number; y: number },
  size: { w: number; h: number },
  viewport: { w: number; h: number },
  margin = 8,
): { x: number; y: number } {
  const fit = (a: number, s: number, v: number) => Math.max(margin, Math.min(a, v - s - margin));
  return { x: fit(anchor.x, size.w, viewport.w), y: fit(anchor.y, size.h, viewport.h) };
}
