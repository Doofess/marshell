/** The most pages loaded in the whole app; the one shown longest ago is unloaded for the next. */
export const LIVE_PAGES = 8;

/**
 * The loaded set after `shown` is shown: most recently shown first. A page whose tab is gone (not in `open`) is dropped, and
 * so is whatever passes the eighth place. `close` is every key that was loaded and no longer is.
 */
export function nextLive(prev: readonly string[], shown: string | null, open: ReadonlySet<string>): { live: string[]; close: string[] } {
  const head = shown !== null && open.has(shown) ? [shown] : [];
  const rest = prev.filter((k) => k !== shown && open.has(k));
  const live = [...new Set([...head, ...rest])].slice(0, LIVE_PAGES);
  const kept = new Set(live);
  return { live, close: prev.filter((k) => !kept.has(k)) };
}
