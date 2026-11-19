import type { Place } from "../../pane/paneState";

/** One page's box on screen: its number from 1, and its top and height in client coordinates. */
export type PageBox = { page: number; top: number; height: number };

const SAME = 0.005;

/**
 * The place at the top of the view: the page whose box spans `viewTop` and how far down it is. In the gap between two pages it is the
 * next page's top, above the first page it is that page's top, and past the last page it is that page's end. `pages` run top to bottom.
 */
export function placeAt(viewTop: number, pages: readonly PageBox[]): Place | null {
  for (const box of pages) {
    if (viewTop < box.top) return { page: box.page, top: 0 };
    if (viewTop < box.top + box.height) return { page: box.page, top: box.height > 0 ? (viewTop - box.top) / box.height : 0 };
  }
  const last = pages[pages.length - 1];
  return last ? { page: last.page, top: 1 } : null;
}

/** How far the scroller must move so the place, on its page's box, lands at `viewTop`. */
export function scrollFor(place: Place, box: PageBox, viewTop: number): number {
  return box.top + place.top * box.height - viewTop;
}

/** A place on a page the file doesn't have becomes the last page's top; a fraction outside 0..1 is pulled in. */
export function clampPlace(place: Place, pages: number): Place {
  if (place.page > pages) return { page: Math.max(pages, 1), top: 0 };
  return { page: Math.max(place.page, 1), top: Math.min(Math.max(place.top, 0), 1) };
}

/** Two places that are one for a jump: the same page, and within half a percent of its height. */
export function samePlace(a: Place, b: Place): boolean {
  return a.page === b.page && Math.abs(a.top - b.top) < SAME;
}

/** What the announcer says after a jump, and after Back or Forward within the same PDF. */
export function placeText(page: number, of: number): string {
  return `Page ${page} of ${of}.`;
}
