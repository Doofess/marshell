import type { PaneView } from "../../generated/PaneView";
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

/** The tab "Close tab" closes: the selected one of the session. */
export function closeTabOutcome(pane: PaneView, session: string | null | undefined): { paneTab: string; path: string } | null {
  const tab = activeTab(pane, session);
  const entry = tab?.history[tab.index];
  return tab && entry ? { paneTab: tab.id, path: entry.path } : null;
}
