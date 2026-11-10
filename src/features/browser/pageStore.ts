import { useSyncExternalStore } from "react";
import { onBrowser } from "./browserApi";

export type PageState = {
  url: string;
  title: string;
  canBack: boolean;
  canForward: boolean;
  loading: boolean;
  icon: string | null;
  focused: boolean;
  status: "refused" | "failed" | null;
};

export type PageEvent =
  | { type: "state"; key: string; url: string; title: string; canBack: boolean; canForward: boolean; loading: boolean }
  | { type: "icon"; key: string; src: string | null }
  | { type: "focus"; key: string; focused: boolean }
  | { type: "refused"; key: string }
  | { type: "failed"; key: string }
  | { type: "closed"; key: string }
  | { type: "gone" };

const blank: PageState = { url: "", title: "", canBack: false, canForward: false, loading: false, icon: null, focused: false, status: null };

function put(pages: ReadonlyMap<string, PageState>, key: string, change: Partial<PageState>): Map<string, PageState> {
  const next = new Map(pages);
  next.set(key, { ...(pages.get(key) ?? blank), ...change });
  return next;
}

export function pageReducer(pages: ReadonlyMap<string, PageState>, e: PageEvent): ReadonlyMap<string, PageState> {
  switch (e.type) {
    case "state":
      return put(pages, e.key, { url: e.url, title: e.title, canBack: e.canBack, canForward: e.canForward, loading: e.loading, status: null });
    case "icon":
      return put(pages, e.key, { icon: e.src });
    case "refused":
      return put(pages, e.key, { status: "refused" });
    case "failed":
      return pages.has(e.key) ? put(pages, e.key, { status: "failed", loading: false }) : pages;
    case "focus": {
      if (!e.focused) return pages.has(e.key) ? put(pages, e.key, { focused: false }) : pages;
      const next = new Map<string, PageState>();
      for (const [k, p] of pages) next.set(k, k === e.key || !p.focused ? p : { ...p, focused: false });
      next.set(e.key, { ...(pages.get(e.key) ?? blank), focused: true });
      return next;
    }
    case "closed": {
      if (!pages.has(e.key)) return pages;
      const next = new Map(pages);
      next.delete(e.key);
      return next;
    }
    case "gone":
      return new Map();
  }
}

let pages: ReadonlyMap<string, PageState> = new Map();
const listeners = new Set<() => void>();

/** Applies an event to the app's store; the app sends `closed` itself when it closes a page. */
export function dispatchPage(e: PageEvent): void {
  const next = pageReducer(pages, e);
  if (next === pages) return;
  pages = next;
  for (const l of listeners) l();
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
