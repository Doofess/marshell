import type { PaneTab } from "../../generated/PaneTab";
import type { PaneView } from "../../generated/PaneView";
import type { Os } from "../../lib/keymap";
import { isPageUrl, pageTitle, shellRefuses } from "../browser/address";
import { isUnsaved } from "../editor/drafts";
import { panelHasNews } from "./paneNews";
import { PANE_LIMITS, activeTab, openRefused, tabsOf } from "./paneState";
import { PANELS, PANEL_LABEL, onScreen, panelOfPath, panelTabs, stackOf, type PanelId } from "./panels";
import type { TabKind } from "./tabType";

/** What the announcer says for the side pane. */
export const NO_SESSION_TO_OPEN = "No session to open it in.";
export const NO_TABS_OPEN = "No tabs open.";
export const noTabsText = (panel: PanelId) => `No tabs open in ${PANEL_LABEL[panel]}.`;
/**
 * What a tab command (next, previous, close) says when it has no tab to act on: the panel that is empty, or that no tabs are open.
 * While a view replaces the terminal no panel is on screen though the session may hold tabs, so it says where the pane shows instead.
 */
export const noTabText = (panel: PanelId | null, replaced: boolean): string => (replaced ? PANE_BESIDE_SESSION : panel ? noTabsText(panel) : NO_TABS_OPEN);
export const panelHiddenText = (panel: PanelId) => `${PANEL_LABEL[panel]} is hidden.`;
export const PANE_BESIDE_SESSION = "The side pane shows beside a session.";

/** The title-bar button and the cursor: toggle the pane when a session is on screen and no main view replaces it, else say where it shows. */
export function paneToggleOutcome(session: string | null | undefined, viewing: boolean): "toggle" | "blocked" {
  return session == null || viewing ? "blocked" : "toggle";
}
export const TAB_LIMIT = `This session has ${PANE_LIMITS.tabs} tabs. Close one to open another.`;
export const FOLDER_REFUSED = "A folder can't be shown in the side pane.";
export const openedText = (name: string, panel: PanelId) => `Opened ${name} in ${PANEL_LABEL[panel]}.`;
/** The announcement for a tab an agent opened with `marshell open`; `who` is the agent's name. */
export const openedByText = (who: string, name: string, panel: PanelId) => `${who} opened ${name} in ${PANEL_LABEL[panel]}.`;
export const closedText = (name: string) => `Closed ${name}.`;
export const movedText = (name: string, to: number, count: number) => `Moved ${name} to position ${to + 1} of ${count}.`;
export const coverText = (cover: string) => `Close ${cover} to open a file.`;

export const PAGE_REFUSED = "That address can't be opened in the side pane.";
/** What the announcer says for a page that opened: its host, or "New tab" for a blank one. */
export const pageOpenedText = (url: string) => openedText(pageTitle(null, url), "browser");

/** Whether an open of this web address can go ahead: a session on screen, an address a page may hold and the shell would not refuse (`own` is the app's own origins), and room in the session (or the address already open). */
export function pageOpenCheck(pane: PaneView, session: string | null | undefined, url: string, own: readonly (readonly [string, number])[]): "no-session" | "refused" | "limit" | "ok" {
  if (session == null) return "no-session";
  if (!isPageUrl(url) || shellRefuses(url, own)) return "refused";
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

/** What the next-tab and previous-tab commands do in the panel: nothing to step through, show the hidden pane first, or step. */
export function paneTabOutcome(pane: PaneView, session: string | null | undefined, panel: PanelId | null): "none" | "show" | "step" {
  if (!pane.open) return tabsOf(pane, session).tabs.length > 0 ? "show" : "none";
  if (panel === null || panelTabs(pane, session, panel).length === 0) return "none";
  return "step";
}

/** What `{ type: "panel" }` is about to do, by the reducer's own rule: it shows a panel that is not on screen, and hides the pane with its last panel. */
export function panelToggleOutcome(pane: PaneView, session: string | null | undefined, panel: PanelId): { shows: boolean; paneHides: boolean } {
  const shows = !onScreen(pane, session).includes(panel);
  const only = stackOf(pane, session).shown;
  return { shows, paneHides: !shows && only.length === 1 && only[0] === panel };
}

/** The panel a tab command acts on: the one with the keyboard; else the last one that had it, when it is on screen and holds tabs; else the first on screen that holds tabs; else the first on screen; null with none on screen. */
export function commandPanel(on: readonly PanelId[], focused: PanelId | null, last: PanelId | null, holdsTabs: (p: PanelId) => boolean): PanelId | null {
  if (on.length === 0) return null;
  if (focused !== null && on.includes(focused)) return focused;
  if (last !== null && on.includes(last) && holdsTabs(last)) return last;
  return on.find(holdsTabs) ?? on[0]!;
}

/** Where the keyboard goes when `panel` hides: nowhere when it was elsewhere; to the terminal; or, where the pane covers the terminal and another panel stays, to that panel. */
export function hideFocus(panel: PanelId, focused: PanelId | null, covers: boolean, staying: readonly PanelId[]): { to: "stay" } | { to: "terminal" } | { to: "panel"; panel: PanelId } {
  if (focused !== panel) return { to: "stay" };
  if (covers && staying.length > 0) return { to: "panel", panel: staying[0]! };
  return { to: "terminal" };
}

/** `current` first, then the others in the stack's order. */
export function panelOrder(on: readonly PanelId[], current: PanelId | null): PanelId[] {
  return current !== null && on.includes(current) ? [current, ...on.filter((p) => p !== current)] : [...on];
}

/** The first panel's answer that is not null. */
export function firstWith<T>(order: readonly PanelId[], get: (p: PanelId) => T | null): T | null {
  for (const p of order) {
    const v = get(p);
    if (v !== null) return v;
  }
  return null;
}

/** The panels on screen whose selected tab is marked new: the ones a look at the pane clears. */
export function unseenPanels(pane: PaneView, session: string | null | undefined, on: readonly PanelId[]): PanelId[] {
  return on.filter((p) => activeTab(pane, session, p)?.unseen === true);
}

/** The panel the tab's history entry after a Back (`by` -1) or Forward (1) belongs to; null at either end. A tab that lands in the other panel moves there. */
export function landingPanel(tab: PaneTab, by: -1 | 1): PanelId | null {
  const entry = tab.history[tab.index + by];
  return entry ? panelOfPath(entry.path) : null;
}

/** The title bar's buttons. `keys(panel)` is the panel command's shortcut label. */
export function panelButtonsFor(on: readonly PanelId[], news: ReadonlySet<string>, keys: (p: PanelId) => string): { id: PanelId; label: string; shown: boolean; news: boolean; keys: string }[] {
  return PANELS.map((id) => ({ id, label: PANEL_LABEL[id], shown: on.includes(id), news: panelHasNews(news, id), keys: keys(id) }));
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
 * What leaving a pane tab for the terminal does (Esc, or Ctrl+Shift+Tab, in a tab). Beside the terminal, the keyboard just moves
 * there. In a window so narrow that the pane covers the terminal, the pane is hidden as well, so the keys never go to a program
 * out of sight.
 */
export function leavePaneOutcome(covers: boolean): "hide" | "focus" {
  return covers ? "hide" : "focus";
}
/** Said when leaving a tab hid the pane: the key that did it doesn't name the pane, so the line does, with the keys that bring it back. */
export const paneHiddenText = (keys: string) => `The side pane is hidden. ${keys} shows it.`;
/**
 * The same hide caused by a navigation key (Ctrl+Shift+Tab), which has a receipt of its own ("Selected session 2, api"). The
 * announcer holds one notice, so the two are one line: said apart, the receipt would replace the hide before anyone heard it.
 */
export const receiptWithPaneHidden = (receipt: string, keys: string) => `${receipt}. ${paneHiddenText(keys)}`;

/**
 * What the recent files of the session on screen are kept under: its repository's root (the row's `projectKey`), not the row's
 * label, so two projects whose folders share a name keep their own lists. Empty with no session on screen.
 */
export function projectKeyOf(rows: ReadonlyArray<{ id: string; projectKey: string }>, session: string | null | undefined): string {
  return rows.find((r) => r.id === session)?.projectKey ?? "";
}

/** The tab "Close tab" closes: the selected one of the panel the command acts on. */
export function closeTabOutcome(pane: PaneView, session: string | null | undefined, panel: PanelId | null): { paneTab: string; path: string } | null {
  if (panel === null) return null;
  const tab = activeTab(pane, session, panel);
  const entry = tab?.history[tab.index];
  return tab && entry ? { paneTab: tab.id, path: entry.path } : null;
}

/**
 * Whether closing this tab asks first: a text tab whose file has unsaved edits does, and a PDF tab with unsaved notes, on every
 * path that closes the one tab (the x button, a middle click, Delete in the strip, "Close tab"). Quitting, closing a session and
 * hiding the pane never ask: the draft keeps the edits.
 */
export function closeAsk(tab: { id: string; path: string; kind: TabKind }, unsaved: ReadonlySet<string>, os: Os = "windows"): "ask" | "close" {
  return (tab.kind === "text" || tab.kind === "pdf") && isUnsaved(unsaved, tab.path, os) ? "ask" : "close";
}

/** Whether the stack is written down now: once the window has loaded, with the pane open, a session on screen and no stack saved. Never with no session on screen. */
export function shouldPin(pane: PaneView, session: string | null | undefined, ready: boolean): boolean {
  return ready && pane.open && !pane.panels && session != null;
}
