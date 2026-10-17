export type ScrollPos = { x: number; y: number };

/** Where a page that has just (re)loaded should scroll to: its last position if it is the same file, the top (null) for a different one or a first load. */
export function restoreScroll(prevPath: string | null, nextPath: string, saved: ScrollPos | null): ScrollPos | null {
  return prevPath !== null && prevPath === nextPath && saved ? saved : null;
}
