import type { Place } from "../../pane/paneState";

/** One page's box on screen: its number from 1, and its top and height in client coordinates. */
export type PageBox = { page: number; top: number; height: number };

const SAME = 0.005;

/**
 * The page in view: the last one whose top has passed `line` (the view's upper third), or page 1. The page field shows it, and a jump
 * announces it, so a place near the end of one page reads as the next page, which fills the view. `pages` run top to bottom.
 */
export function pageInView(line: number, pages: readonly { page: number; top: number }[]): number {
  let page = 1;
  for (const p of pages) {
    if (p.top <= line) page = p.page;
    else break;
  }
  return page;
}

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

/**
 * The body width the pages are laid out for next, or null for no change. A hidden tab (a kept tab not selected) measures 0 wide:
 * its pages keep their layout, and the browser keeps its scroll offset, so it comes back at the place it was left.
 */
export function bodyWidthChange(prev: number, next: number): number | null {
  return next > 0 && next !== prev ? next : null;
}

/** A place is put only once the document of these bytes and this version is laid out, so a jump during a reload waits for the new file. */
export function placeReady<B>(loaded: { bytes: B; version: number } | null, now: { bytes: B; version: number }, laidOut: boolean): boolean {
  return laidOut && loaded !== null && loaded.bytes === now.bytes && loaded.version === now.version;
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
/** What a jump to the place already in view says instead of moving: the page the Page field shows. Null for a real move, or before the PDF has its page count. */
export function alreadyHere(to: Place, here: Place | null, shown: { page: number; count: number } | null): string | null {
  return here && shown && samePlace(to, here) ? placeText(shown.page, shown.count) : null;
}

export function placeText(page: number, of: number): string {
  return `Page ${page} of ${of}.`;
}
