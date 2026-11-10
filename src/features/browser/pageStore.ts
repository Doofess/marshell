import { useSyncExternalStore } from "react";
import { passingBlank } from "./address";
import { browser, onBrowser } from "./browserApi";

export type PageState = {
  url: string;
  title: string;
  canBack: boolean;
  canForward: boolean;
  loading: boolean;
  icon: string | null;
  focused: boolean;
  status: "refused" | "failed" | null;
  /** The shell has the page: it has reported its state. False from the app asking for it until then, and for good when the shell refused to open it. */
  live: boolean;
};

export type PageEvent =
  /** The app is about to ask the shell for the page. */
  | { type: "opening"; key: string }
  | { type: "state"; key: string; url: string; title: string; canBack: boolean; canForward: boolean; loading: boolean }
  | { type: "icon"; key: string; src: string | null }
  | { type: "focus"; key: string; focused: boolean }
  | { type: "refused"; key: string }
  | { type: "failed"; key: string }
  | { type: "closed"; key: string }
  | { type: "gone" };

const blank: PageState = { url: "", title: "", canBack: false, canForward: false, loading: false, icon: null, focused: false, status: null, live: false };

function put(pages: ReadonlyMap<string, PageState>, key: string, change: Partial<PageState>): Map<string, PageState> {
  const next = new Map(pages);
  next.set(key, { ...(pages.get(key) ?? blank), ...change });
  return next;
}

/**
 * The store holds the pages the app asked the shell for, from `opening` to `closed`, and nothing else: what any other page
 * reports is dropped. The shell's events and its answers come on separate channels, so a page's last report can come after the
 * app closed it.
 */
export function pageReducer(pages: ReadonlyMap<string, PageState>, e: PageEvent): ReadonlyMap<string, PageState> {
  if (e.type === "gone") return new Map();
  if (e.type === "opening") return pages.has(e.key) ? pages : put(pages, e.key, {});
  if (!pages.has(e.key)) return pages;
  switch (e.type) {
    case "state": {
      // A refused link or redirect is a cancelled navigation, and the page reports its end like any other: the same address, and
      // still loading or not as it was. "Refused" stays through those, until a navigation starts or the address changes.
      const before = pages.get(e.key);
      const started = e.loading && !before?.loading;
      // A page on its way to its first address says `about:blank`; the bar keeps the address it had.
      const url = passingBlank(e.url, e.loading) ? (before?.url ?? "") : e.url;
      const status = before?.status === "refused" && !started && url === before.url ? "refused" : null;
      return put(pages, e.key, { url, title: e.title, canBack: e.canBack, canForward: e.canForward, loading: e.loading, status, live: true });
    }
    case "icon":
      return put(pages, e.key, { icon: e.src });
    case "refused":
      return put(pages, e.key, { status: "refused" });
    case "failed":
      return put(pages, e.key, { status: "failed", loading: false });
    case "focus": {
      if (!e.focused) return put(pages, e.key, { focused: false });
      const next = new Map<string, PageState>();
      for (const [k, p] of pages) next.set(k, k === e.key || !p.focused ? p : { ...p, focused: false });
      next.set(e.key, { ...(pages.get(e.key) ?? blank), focused: true });
      return next;
    }
    case "closed": {
      const next = new Map(pages);
      next.delete(e.key);
      return next;
    }
  }
}

let pages: ReadonlyMap<string, PageState> = new Map();
const listeners = new Set<() => void>();

/** Applies an event to the app's store; the app sends `opening` itself before it asks for a page, and `closed` when it closes one. */
export function dispatchPage(e: PageEvent): void {
  const next = pageReducer(pages, e);
  if (next === pages) return;
  pages = next;
  for (const l of listeners) l();
}

/** Asks the shell to open the key's page at `url`. The store holds the page from here on, so what it reports is kept. */
export function openPage(key: string, url: string): Promise<void> {
  dispatchPage({ type: "opening", key });
  return browser.open(key, url);
}

function subscribe(l: () => void): () => void {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** Calls `listener` after every change to any page's state. Returns the unsubscribe function. */
export const subscribePages = subscribe;

export function usePage(key: string | null): PageState | undefined {
  return useSyncExternalStore(
    subscribe,
    () => (key === null ? undefined : pages.get(key)),
    () => undefined,
  );
}

/** A page's site icon (an image address), or null while it has none or isn't loaded. */
export function pageIcon(key: string): string | null {
  return pages.get(key)?.icon ?? null;
}

/** The key of the page that has the keyboard, if any. */
export function focusedPage(): string | null {
  for (const [k, p] of pages) if (p.focused) return k;
  return null;
}

/** `focusedPage()` for a component: it draws again when the keyboard goes into a page or leaves it. */
export function useFocusedPage(): string | null {
  return useSyncExternalStore(subscribe, focusedPage, () => null);
}

/** Subscribes to the shell's `browser://` events once. Returns the stop function. */
export function startPageStore(): () => void {
  const stops = [
    onBrowser("state", (p) => dispatchPage({ type: "state", ...p })),
    onBrowser("icon", (p) => dispatchPage({ type: "icon", key: p.key, src: p.src })),
    onBrowser("focus", (p) => dispatchPage({ type: "focus", key: p.key, focused: p.focused })),
    onBrowser("refused", (p) => dispatchPage({ type: "refused", key: p.key })),
    onBrowser("failed", (p) => dispatchPage({ type: "failed", key: p.key })),
    onBrowser("gone", () => dispatchPage({ type: "gone" })),
  ];
  return () => stops.forEach((s) => s());
}
