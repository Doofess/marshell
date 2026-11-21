import type { PanelId } from "./panels";

/** What changes the set of (panel, session) pairs that have a tab `marshell open` added and nobody has looked at. */
export type NewsAction =
  | { type: "add"; items: readonly { session: string; panel: PanelId }[] }
  /** The panel is on screen for the session. */
  | { type: "seen"; session: string; panel: PanelId }
  | { type: "closed"; session: string };

/** A key of the set: the panel id never holds ":", so the session is what follows the first one. */
const keyOf = (panel: PanelId, session: string) => `${panel}:${session}`;

/** The pairs with news, as `panel:session`. The same set comes back when nothing changes. */
export function newsReducer(news: ReadonlySet<string>, a: NewsAction): ReadonlySet<string> {
  switch (a.type) {
    case "add": {
      const keys = a.items.map((i) => keyOf(i.panel, i.session));
      if (keys.every((k) => news.has(k))) return news;
      return new Set([...news, ...keys]);
    }
    case "seen": {
      const k = keyOf(a.panel, a.session);
      if (!news.has(k)) return news;
      const next = new Set(news);
      next.delete(k);
      return next;
    }
    case "closed": {
      const gone = [...news].filter((k) => k.slice(k.indexOf(":") + 1) === a.session);
      if (gone.length === 0) return news;
      const next = new Set(news);
      for (const k of gone) next.delete(k);
      return next;
    }
  }
}

/** True when any session has news for the panel. */
export function panelHasNews(news: ReadonlySet<string>, panel: PanelId): boolean {
  return [...news].some((k) => k.startsWith(`${panel}:`));
}
