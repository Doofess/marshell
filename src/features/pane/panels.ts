import type { PaneTab } from "../../generated/PaneTab";
import type { PaneView } from "../../generated/PaneView";
import type { SessionTabs } from "../../generated/SessionTabs";
import { isPageUrl } from "../browser/address";
import { kindOf } from "../preview/kinds";

/** The side pane's panels, top to bottom. This file imports nothing from `paneState.ts`: that file imports this one. */
export const PANELS = ["browser", "files"] as const;
export type PanelId = (typeof PANELS)[number];
export const PANEL_LABEL: Record<PanelId, string> = { browser: "Browser", files: "Files" };
export const PANEL_COMMAND: Record<PanelId, string> = { browser: "browser-panel", files: "files-panel" };
/** A panel's weight in the stack's height when it has none saved. */
export const DEFAULT_WEIGHT = 10_000;
export type Weights = Partial<Record<PanelId, number>>;

export function isPanelId(s: string): s is PanelId {
  return (PANELS as readonly string[]).includes(s);
}

/** A web page address or an HTML file is in Browser; every other path is in Files. */
export function panelOfPath(path: string): PanelId {
  return isPageUrl(path) || kindOf(path) === "html" ? "browser" : "files";
}

/** The panel a tab is in now: a browser tab, or a file tab whose current path is a page or an HTML file, is in Browser. */
export function panelOf(tab: PaneTab): PanelId {
  return tab.kind === "browser" ? "browser" : panelOfPath(tab.history[tab.index]?.path ?? "");
}

export type Stack = { shown: PanelId[]; weights: Weights; filled: PanelId | null };

/** The panels that hold at least one tab of the session, in the stack's order. */
export function panelsWithTabs(st: SessionTabs): PanelId[] {
  return PANELS.filter((p) => st.tabs.some((t) => panelOf(t) === p));
}

/** The saved stack, cleaned; with none saved, the panels that hold tabs for the session, Files alone when it has none. */
export function stackOf(pane: PaneView, session: string | null | undefined): Stack {
  const saved = pane.panels;
  if (saved) {
    const listed = Array.isArray(saved.shown) ? saved.shown : [];
    const shown = PANELS.filter((p) => listed.includes(p));
    const weights: Weights = {};
    for (const p of PANELS) {
      const w = saved.weights?.[p];
      if (typeof w === "number") weights[p] = w;
    }
    const filled = typeof saved.filled === "string" && isPanelId(saved.filled) && shown.length === 2 && shown.includes(saved.filled) ? saved.filled : null;
    return { shown: shown.length > 0 ? [...shown] : ["files"], weights, filled };
  }
  const st = session != null ? pane.sessions[session] : undefined;
  const held = st ? panelsWithTabs(st) : [];
  return { shown: held.length > 0 ? held : ["files"], weights: {}, filled: null };
}

/** What the stack draws, whether or not the pane is open: the filled panel alone, else the shown ones. */
export function stackPanels(stack: Stack): PanelId[] {
  return stack.filled ? [stack.filled] : stack.shown;
}

/** What is in front of the person: nothing while the pane is hidden, else `stackPanels`. */
export function onScreen(pane: PaneView, session: string | null | undefined): PanelId[] {
  return pane.open ? stackPanels(stackOf(pane, session)) : [];
}

/** The session's tabs that are in the panel, in the session's order. */
export function panelTabs(pane: PaneView, session: string | null | undefined, panel: PanelId): PaneTab[] {
  const st = session != null ? pane.sessions[session] : undefined;
  return st ? st.tabs.filter((t) => panelOf(t) === panel) : [];
}

/**
 * The panel's selected tab: its own field when that names a tab of the panel, else the other field when it does, else the panel's
 * first tab, else null. The own field is `active_browser` for Browser and `active` for Files.
 */
export function activeIn(st: SessionTabs, panel: PanelId): string | null {
  const own = panel === "browser" ? st.active_browser : st.active;
  const other = panel === "browser" ? st.active : st.active_browser;
  const mine = (id: string | null | undefined) => {
    const t = id == null ? undefined : st.tabs.find((x) => x.id === id);
    return t && panelOf(t) === panel ? t : undefined;
  };
  return (mine(own) ?? mine(other) ?? st.tabs.find((t) => panelOf(t) === panel))?.id ?? null;
}
