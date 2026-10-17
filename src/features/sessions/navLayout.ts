import type { NavLayout, Rect, Target } from "../../lib/navigation/types";

/** One element the cursor can reach, as the page drew it: its `data-nav-*` attribute, its box on screen, and the box of the list that scrolls it, if any. */
export type Measured = { attrs: { pane?: string; row?: string; top?: string }; rect: Rect; clip?: Rect };

const SELECTOR = "[data-nav-pane],[data-nav-row],[data-nav-top]";
/** The lists that scroll their rows: a row outside its list's box is scrolled out of sight. */
const SCROLLERS = ".sidebar__list,.rail__list";

const overlaps = (a: Rect, b: Rect) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

function targetOf({ pane, row, top }: Measured["attrs"]): Target | null {
  if (pane !== undefined) return /^\d+$/.test(pane) ? { kind: "pane", index: Number(pane) } : null;
  if (row !== undefined) return /^\d+$/.test(row) ? { kind: "row", number: Number(row) } : null;
  return top ? { kind: "top", id: top } : null;
}

/**
 * The navigation layout from what is on screen: each marked element becomes a target, with its rect made relative to the window
 * (`origin`). An element with no size is not drawn (`display: none`), a row outside its list's box is scrolled out of sight, and
 * an attribute that names no pane, row or control is a mistake in the markup, so all three are left out.
 */
export function navLayoutOf(items: Measured[], origin: Rect): NavLayout {
  return items.flatMap(({ attrs, rect, clip }) => {
    const target = targetOf(attrs);
    if (!target || rect.w <= 0 || rect.h <= 0 || (clip && !overlaps(rect, clip))) return [];
    return [{ target, rect: { x: rect.x - origin.x, y: rect.y - origin.y, w: rect.w, h: rect.h } }];
  });
}

const box = (el: Element): Rect => {
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.top, w: r.width, h: r.height };
};

/** Measures the window `root` now. Called on a key press, never on render. */
export function measureNavLayout(root: HTMLElement): NavLayout {
  const items = [...root.querySelectorAll<HTMLElement>(SELECTOR)].map((el) => {
    const list = el.closest(SCROLLERS);
    return { attrs: { pane: el.dataset.navPane, row: el.dataset.navRow, top: el.dataset.navTop }, rect: box(el), clip: list ? box(list) : undefined };
  });
  return navLayoutOf(items, box(root));
}
