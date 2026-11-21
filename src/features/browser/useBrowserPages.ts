import { useEffect, useEffectEvent, useMemo, useRef, useSyncExternalStore } from "react";
import type { PaneView } from "../../generated/PaneView";
import { appScheme } from "../terminal/terminalTheme";
import { isBrowserTab, tabsOf, type PaneAction } from "../pane/paneState";
import { PAGE_REFUSED } from "../pane/paneCommands";
import { passingBlank, shellRefuses } from "./address";
import { browser, onBrowser, pageKey } from "./browserApi";
import { nextLive } from "./pages";
import { dispatchPage, pageIcon, setLoadable, startPageStore, subscribePages, type PageEvent } from "./pageStore";

const ignore = () => {};

/** Where a page's key points: its session and its pane tab. */
export type Owner = { session: string; paneTab: string };

/**
 * Every browser tab of every open session, by its page's key: the pages that may be loaded. File tabs are not counted. The pane
 * keeps a closed session's tabs, so that reopening it brings them back; `sessions` (the ids in the session list) leaves those
 * out, and their pages unload. Null while the list isn't known: every session counts, and nothing unloads.
 */
export function owners(pane: PaneView, sessions: ReadonlySet<string> | null): Map<string, Owner> {
  const out = new Map<string, Owner>();
  for (const session of Object.keys(pane.sessions)) {
    if (sessions !== null && !sessions.has(session)) continue;
    for (const t of tabsOf(pane, session).tabs) if (isBrowserTab(t)) out.set(pageKey(session, t.id), { session, paneTab: t.id });
  }
  return out;
}

/** The keys of every browser tab of every open session. */
export const openKeys = (pane: PaneView, sessions: ReadonlySet<string> | null): Set<string> => new Set(owners(pane, sessions).keys());

/**
 * What a page's popup does to the pane: a new browser tab in the popup's own session, selected when that session is on screen, and
 * a background tab otherwise (it never selects, shows or hides anything). Nothing for an address the shell
 * would refuse (one of the app's own origins, in `own`).
 */
export function popupActions(pane: PaneView, session: string, url: string, onScreen: boolean, own: readonly (readonly [string, number])[]): PaneAction[] {
  if (shellRefuses(url, own)) return [];
  return onScreen ? [{ type: "openPage", session, url }] : [{ type: "openPage", session, url, background: true }];
}

/**
 * What a page's report does to its tab: the tab takes the page's address and title. Nothing for a page no tab holds, and nothing
 * while a blank page is loading: a page reports `about:blank` before its first address has loaded, and the tab keeps the address
 * it was saved with until the page is somewhere.
 */
export function pageAction(owner: Owner | undefined, p: { url: string; title: string; loading: boolean }): PaneAction | null {
  if (!owner || passingBlank(p.url, p.loading)) return null;
  return { type: "page", session: owner.session, paneTab: owner.paneTab, url: p.url, title: p.title === "" ? null : p.title };
}

/**
 * Unloads the pages the loaded list dropped: the shell closes each, and the store forgets it once the shell has answered, so
 * its tab opens it again when it is next shown. A close that fails forgets the page too: either way the tab has no page to show.
 */
export async function unload(keys: readonly string[], close: (key: string) => Promise<unknown>, forget: (e: PageEvent) => void): Promise<void> {
  await Promise.all(
    keys.map((key) =>
      close(key)
        .catch(ignore)
        .then(() => forget({ type: "closed", key })),
    ),
  );
}

type Options = {
  /** The ids of the sessions in the list, or null while the list isn't known. A session that is not there has no page loaded. */
  sessions: readonly string[] | null;
  dispatch(action: PaneAction): void;
  /** The session on screen, if any. */
  focusedSession(): string | null | undefined;
  /** The app's own origins now (host and port): a popup to one opens no tab. */
  own(): readonly (readonly [string, number])[];
  /** Says something to the person. */
  announce(text: string): void;
};

/**
 * Keeps the app's side of the pages. The loaded list (`nextLive`: at most 8, the one shown longest ago unloaded) closes the pages it
 * drops and the store forgets them; the shell's `browser://` events reach the store, and a page's address and title reach its tab; a popup opens a tab; the
 * pages follow the app's theme. Nothing is closed on unmount: the shell closes the pages at quit.
 */
export function useBrowserPages(pane: PaneView, shownKey: string | null, { sessions, dispatch, focusedSession, own, announce }: Options): void {
  const live = useRef<string[]>([]);
  const owned = owners(pane, sessions === null ? null : new Set(sessions));
  const open = new Set(owned.keys());
  useEffect(() => {
    // Before the unloading: a tab still drawn must know it may not open the page again.
    setLoadable(open);
    const next = nextLive(live.current, shownKey, open);
    live.current = next.live;
    void unload(next.close, browser.close, dispatchPage);
  });

  const ownersRef = useRef(owned);
  ownersRef.current = owned;
  const paneRef = useRef(pane);
  paneRef.current = pane;
  const send = useEffectEvent((action: PaneAction) => dispatch(action));
  const onScreen = useEffectEvent(() => focusedSession());
  const ownOrigins = useEffectEvent(() => own());
  const say = useEffectEvent((text: string) => announce(text));
  useEffect(() => {
    const stops = [
      startPageStore(),
      onBrowser("state", (p) => {
        const action = pageAction(ownersRef.current.get(p.key), p);
        if (action) send(action);
      }),
      onBrowser("popup", (p) => {
        const owner = ownersRef.current.get(p.key);
        if (!owner) return;
        const actions = popupActions(paneRef.current, owner.session, p.url, onScreen() === owner.session, ownOrigins());
        if (actions.length === 0) say(PAGE_REFUSED);
        for (const action of actions) send(action);
      }),
    ];
    return () => stops.forEach((stop) => stop());
  }, []);

  // The pages follow the app's theme: sent on mount and on every change.
  useEffect(() => {
    const push = () => void browser.theme(appScheme() === "dark").catch(ignore);
    push();
    const scheme = matchMedia("(prefers-color-scheme: dark)");
    scheme.addEventListener("change", push);
    const watch = new MutationObserver(push);
    watch.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => {
      scheme.removeEventListener("change", push);
      watch.disconnect();
    };
  }, []);
}

/** The site icon of each key's page, for the tab strip: an image address, or null before one arrives. Draws again when an icon changes. */
export function usePageIcons(keys: readonly string[]): ReadonlyMap<string, string | null> {
  const joined = useSyncExternalStore(
    subscribePages,
    () => keys.map((k) => `${k}\n${pageIcon(k) ?? ""}`).join("\n\n"),
    () => "",
  );
  // `joined` names the keys and icons, so the map is rebuilt only when one of them changes.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  return useMemo(() => new Map(keys.map((k): [string, string | null] => [k, pageIcon(k)])), [joined]);
}
