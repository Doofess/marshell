/** What changes the set of sessions that have a tab `marshell open` added and nobody has looked at. */
export type NewsAction =
  | { type: "add"; sessions: readonly string[] }
  /** The session's pane is open and the session is on screen. */
  | { type: "seen"; session: string }
  | { type: "closed"; session: string };

/** The sessions with news. The same set comes back when nothing changes. */
export function newsReducer(news: ReadonlySet<string>, a: NewsAction): ReadonlySet<string> {
  switch (a.type) {
    case "add": {
      if (a.sessions.every((s) => news.has(s))) return news;
      return new Set([...news, ...a.sessions]);
    }
    case "seen":
    case "closed": {
      if (!news.has(a.session)) return news;
      const next = new Set(news);
      next.delete(a.session);
      return next;
    }
  }
}
