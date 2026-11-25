import type { PanelId } from "./panels";

/** What changes the set of (panel, session) pairs that have a tab `marshell open` added and nobody has looked at. */
export type NewsAction =
  | { type: "add"; items: readonly { session: string; panel: PanelId }[] }
  /** The panel is on screen for the session. */
  | { type: "seen"; session: string; panel: PanelId }
  /** The sessions still in the list: a pair of any other session is forgotten, since nobody can look at its panel any more. */
  | { type: "only"; sessions: ReadonlySet<string> }
  /** The shell's pairs follow it to its new session: every `panel:from` becomes `panel:to`. */
  | { type: "rename"; from: string; to: string };

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
    case "rename": {
      const moving = [...news].filter((k) => k.slice(k.indexOf(":") + 1) === a.from);
      if (moving.length === 0) return news;
      const next = new Set(news);
      for (const k of moving) {
        next.delete(k);
        next.add(`${k.slice(0, k.indexOf(":"))}:${a.to}`);
      }
      return next;
    }
    case "only": {
      const gone = [...news].filter((k) => !a.sessions.has(k.slice(k.indexOf(":") + 1)));
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
