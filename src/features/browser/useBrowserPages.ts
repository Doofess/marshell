import { useCallback, useEffect, useEffectEvent, useMemo, useRef, useSyncExternalStore } from "react";
import type { PaneView } from "../../generated/PaneView";
import { appScheme } from "../terminal/terminalTheme";
import type { CookieRule } from "../../generated/CookieRule";
import { currentEntry, isBrowserTab, openRefused, paneReducer, tabsOf, type PaneAction } from "../pane/paneState";
import { PAGE_REFUSED, tabLimitText } from "../pane/paneCommands";
import { SHELL_OWNER } from "../pane/paneOwner";
import { passingBlank, shellRefuses } from "./address";
import { browser, onBrowser, pageKey } from "./browserApi";
import { nextLive } from "./pages";
import { useBrowserSettings } from "./browserSettings";
import { cookiesSaid, needsReload, ruleToTell } from "./cookies";
import { dispatchPage, openPage, pageIcon, setLoadable, startPageStore, subscribePages, type PageEvent } from "./pageStore";

const ignore = () => {};

/** Where a page's key points: its session and its pane tab. */
export type Owner = { session: string; paneTab: string };

/**
 * Every browser tab of every open session, by its page's key: the pages that may be loaded. File tabs are not counted. The pane
 * keeps a closed session's tabs, so that reopening it brings them back; `sessions` (the ids in the session list) leaves those
 * out, and their pages unload. Null while the list isn't known: every session counts, and nothing unloads.
 * The shell panel's tabs (the pane's `"shell"` set) are never filtered by `sessions`: their pages are keyed by `shellTabId`, the
 * panel shell's id, so a handover (the set renamed to that id) keeps every key. While `shellTabId` is null they have no pages.
 */
export function owners(pane: PaneView, sessions: ReadonlySet<string> | null, shellTabId: string | null): Map<string, Owner> {
  const out = new Map<string, Owner>();
  for (const session of Object.keys(pane.sessions)) {
    const shell = session === SHELL_OWNER;
    if (shell ? shellTabId === null : sessions !== null && !sessions.has(session)) continue;
    const base = shell ? shellTabId! : session;
    for (const t of tabsOf(pane, session).tabs) if (isBrowserTab(t)) out.set(pageKey(base, t.id), { session, paneTab: t.id });
  }
  return out;
}

/** The keys of every browser tab of every open session, and of the shell panel's. */
export const openKeys = (pane: PaneView, sessions: ReadonlySet<string> | null, shellTabId: string | null): Set<string> => new Set(owners(pane, sessions, shellTabId).keys());

/**
 * What a page's popup does to the pane: a new browser tab in the popup's own session, selected when that session's Browser panel is on screen, and
 * a background tab otherwise (it never selects, shows or hides anything). Nothing for an address the shell
 * would refuse (one of the app's own origins, in `own`).
 */
export function popupActions(pane: PaneView, session: string, url: string, onScreen: boolean, own: readonly (readonly [string, number])[]): PaneAction[] {
  if (shellRefuses(url, own)) return [];
  return onScreen ? [{ type: "openPage", session, url }] : [{ type: "openPage", session, url, background: true }];
}

/**
 * What a popup does and what the app says about it: a session at the tab limit gets the limit line when the popup would add a tab (an address already open is selected instead), an address the shell would
 * refuse gets the refusal, and a popup that opens says nothing.
 */
export function popupOutcome(pane: PaneView, session: string, url: string, onScreen: boolean, own: readonly (readonly [string, number])[]): { actions: PaneAction[]; say: string | null } {
  const actions = popupActions(pane, session, url, onScreen, own);
  if (actions.length === 0) return { actions, say: PAGE_REFUSED };
  if (openRefused(pane, session, url)) return { actions: [], say: tabLimitText(session) };
  return { actions, say: null };
}

/**
 * The sessions a popup's actions leave a new tab in the background of: their Browser panel gets its dot. A session gets none when
 * the action adds no tab to `pane` (the address is already open there, or the session is at its tab limit): the dot says a tab is
 * waiting, and none would be.
 */
export function popupNews(actions: readonly PaneAction[], pane: PaneView): string[] {
  const count = (s: PaneView, session: string) => tabsOf(s, session).tabs.length;
  return actions.flatMap((a) => (a.type === "openPage" && a.background === true && count(paneReducer(pane, a), a.session) > count(pane, a.session) ? [a.session] : []));
}

/** The pane after a popup's actions, in order: what a second popup in the same tick reads. */
export function popupFold(pane: PaneView, actions: readonly PaneAction[]): PaneView {
  return actions.reduce(paneReducer, pane);
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

/** What loading every loaded page again does: each key's tab address, or null for a key whose tab is gone (closed, not opened again). */
export function reloadPlan(pane: PaneView, keys: readonly string[], shellTabId: string | null): { key: string; url: string | null }[] {
  const owned = owners(pane, null, shellTabId);
  return keys.map((key) => {
    const o = owned.get(key);
    const tab = o ? tabsOf(pane, o.session).tabs.find((t) => t.id === o.paneTab) : undefined;
    return { key, url: tab ? currentEntry(tab).path : null };
  });
}

/**
 * Closes every page, then opens the ones with an address again, shown or not (a page behind another tab loads there, as it was).
 * Every close comes before the first open: private pages share one store of cookies and site data, which ends only when the last
 * of them is closed (S14), so a page opened while another is still there would find it as it was.
 * The calls go through the shell's queue, so they come after a `cookies` call asked before them. Returns how many were opened.
 */
export function reloadPages(plan: readonly { key: string; url: string | null }[]): number {
  for (const { key } of plan) {
    void browser.close(key).catch(ignore);
    dispatchPage({ type: "closed", key });
  }
  let opened = 0;
  for (const { key, url } of plan) {
    if (url === null) continue;
    void openPage(key, url).catch(ignore);
    opened += 1;
  }
  return opened;
}

type Options = {
  /** The ids of the sessions in the list, or null while the list isn't known. A session that is not there has no page loaded. */
  sessions: readonly string[] | null;
  dispatch(action: PaneAction): void;
  /** The session whose Browser panel is on screen now, if any (read when a popup arrives). */
  browserSession(): string | null | undefined;
  /** A tab opened in the background of this session's Browser panel: its button gets a dot. */
  news(session: string): void;
  /** The app's own origins now (host and port): a popup to one opens no tab. */
  own(): readonly (readonly [string, number])[];
  /** Says something to the person. */
  announce(text: string): void;
  /** The panel shell's tab id, or null while it isn't there: the shell's browser tabs are keyed by it. */
  shellTabId: string | null;
};

/**
 * Keeps the app's side of the pages. The loaded list (`nextLive`: at most 8, the one shown longest ago unloaded) closes the pages it
 * drops and the store forgets them; the shell's `browser://` events reach the store, and a page's address and title reach its tab; a popup opens a tab; the
 * pages follow the app's theme. Nothing is closed on unmount: the shell closes the pages at quit.
 */
export function useBrowserPages(pane: PaneView, shownKey: string | null, { sessions, dispatch, browserSession, news, own, announce, shellTabId }: Options): { reloadPrivate(): number } {
  const live = useRef<string[]>([]);
  const owned = owners(pane, sessions === null ? null : new Set(sessions), shellTabId);
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
  const shellRef = useRef(shellTabId);
  shellRef.current = shellTabId;
  const send = useEffectEvent((action: PaneAction) => dispatch(action));
  const onScreen = useEffectEvent(() => browserSession());
  const tell = useEffectEvent((session: string) => news(session));
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
        const pane = paneRef.current;
        const { actions, say: line } = popupOutcome(pane, owner.session, p.url, onScreen() === owner.session, ownOrigins());
        if (line !== null) say(line);
        for (const action of actions) send(action);
        for (const session of popupNews(actions, pane)) tell(session);
        // A second popup before the next render reads the pane with this one's tab in it.
        paneRef.current = popupFold(pane, actions);
      }),
    ];
    return () => stops.forEach((stop) => stop());
  }, []);

  // The cookie rule: told before any page is made (the shell's queue holds every call until then), and told again on a change. A
  // change to or from "never" makes the loaded pages again, since a page's privacy is decided when it is made.
  // Reads refs only, so it is safe in an effect and in an event handler alike.
  const reloadLoaded = useCallback((): number => reloadPages(reloadPlan(paneRef.current, live.current, shellRef.current)), []);
  const { settings, failed } = useBrowserSettings();
  const rule = ruleToTell(settings, failed);
  const told = useRef<CookieRule | null>(null);
  useEffect(() => {
    if (rule === null || rule === told.current) return;
    const last = told.current;
    told.current = rule;
    // Asked first, so the closes and opens of the reload queue behind it.
    void browser.cookies(rule).catch(ignore);
    if (needsReload(last, rule)) say(cookiesSaid(rule, reloadLoaded()));
    // `say` and `reloadLoaded` read only refs and effect events.
  }, [rule, reloadLoaded]);

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

  // After "Clear browsing data…": under "never" the pages' cookies live in memory, so the pages are made again to lose them.
  const reloadPrivate = useCallback((): number => (told.current === "never" ? reloadLoaded() : 0), []);
  return { reloadPrivate };
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
