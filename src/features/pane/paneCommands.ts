import type { PaneView } from "../../generated/PaneView";
import { isPageUrl, pageTitle } from "../browser/address";
import { PANE_LIMITS, activeTab, openRefused, tabsOf } from "./paneState";

/** What the announcer says for the side pane. */
export const NO_SESSION_TO_OPEN = "No session to open it in.";
export const NO_TABS_OPEN = "No tabs open.";
export const TAB_LIMIT = `This session has ${PANE_LIMITS.tabs} tabs. Close one to open another.`;
export const FOLDER_REFUSED = "A folder can't be shown in the side pane.";
export const openedText = (name: string) => `Opened ${name} in the side pane.`;
export const closedText = (name: string) => `Closed ${name}.`;
export const movedText = (name: string, to: number, count: number) => `Moved ${name} to position ${to + 1} of ${count}.`;
export const coverText = (cover: string) => `Close ${cover} to open a file.`;

export const PAGE_REFUSED = "That address can't be opened in the side pane.";
/** What the announcer says for a page that opened: its host, or "New tab" for a blank one. */
export const pageOpenedText = (url: string) => openedText(pageTitle(null, url));

/** Whether an open of this web address can go ahead: a session on screen, an address a page may hold, and room in the session (or the address already open). */
export function pageOpenCheck(pane: PaneView, session: string | null | undefined, url: string): "no-session" | "refused" | "limit" | "ok" {
  if (session == null) return "no-session";
  if (!isPageUrl(url)) return "refused";
  return openRefused(pane, session, url) ? "limit" : "ok";
}

/** Whether an open of this path can go ahead: it needs a session on screen, and a session with room (or already showing the path). */
export function openCheck(pane: PaneView, session: string | null | undefined, path: string): "no-session" | "refused" | "ok" {
  if (session == null) return "no-session";
  return openRefused(pane, session, path) ? "refused" : "ok";
}

/** What the pane shows for the session: its tabs, or the empty state (also while the pane is open with no session on screen). */
export function paneBody(pane: PaneView, session: string | null | undefined): "tabs" | "empty" {
  return tabsOf(pane, session).tabs.length > 0 ? "tabs" : "empty";
}

/** Ctrl+Shift+Q: the switch flips, and the pane shows the empty state when the session has no tabs. */
export function toggleOutcome(pane: PaneView, session: string | null | undefined): { shown: boolean; body: "tabs" | "empty" } {
  return { shown: !pane.open, body: paneBody(pane, session) };
}

/** What the next-tab and previous-tab commands do: nothing to step through, show the hidden pane first, or step. */
export function paneTabOutcome(pane: PaneView, session: string | null | undefined): "none" | "show" | "step" {
  if (tabsOf(pane, session).tabs.length === 0) return "none";
  return pane.open ? "step" : "show";
}

/**
 * Whether the next-tab and previous-tab commands move focus into the pane. They leave it alone (Ctrl+Tab steps the tabs while
 * the person types in the terminal) except in two cases, where focus would be left on something that is no longer drawn:
 * - a step taken from inside a tab's body hides that body, so focus goes to the body of the tab now showing;
 * - showing the hidden pane in a window so narrow that it covers the terminal, so the keys stop going to a program out of sight.
 */
export function paneTabFocus(s: { outcome: "none" | "show" | "step"; from: "panel" | "elsewhere"; covers: boolean }): boolean {
  if (s.outcome === "step") return s.from === "panel";
  return s.outcome === "show" && s.covers;
}

/**
 * What the recent files of the session on screen are kept under: its repository's root (the row's `projectKey`), not the row's
 * label, so two projects whose folders share a name keep their own lists. Empty with no session on screen.
 */
export function projectKeyOf(rows: ReadonlyArray<{ id: string; projectKey: string }>, session: string | null | undefined): string {
  return rows.find((r) => r.id === session)?.projectKey ?? "";
}

/** The tab "Close tab" closes: the selected one of the session. */
export function closeTabOutcome(pane: PaneView, session: string | null | undefined): { paneTab: string; path: string } | null {
  const tab = activeTab(pane, session);
  const entry = tab?.history[tab.index];
  return tab && entry ? { paneTab: tab.id, path: entry.path } : null;
}
