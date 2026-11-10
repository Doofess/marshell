import type { ViewLayout } from "../../generated/ViewLayout";
import { type PaneAction, paneReducer } from "../pane/paneState";
import { SIDEBAR, type SidebarMode, type SidebarPref } from "./layout";

/** What the window remembers: the saved layout, plus the sidebar pref to return to when focus mode ends (not saved). */
export type ViewState = ViewLayout & { lastPref?: SidebarPref };

export type ViewAction =
  /** The session goes into the one pane. */
  | { type: "select"; tabId: string }
  /** The tabs that are open now; a pane showing any other tab becomes empty. */
  | { type: "sessions"; openIds: string[] }
  | { type: "sidebar"; pref: SidebarPref }
  | { type: "sidebarWidth"; width: number }
  | { type: "resetSidebarWidth" }
  | { type: "toggleFocus" }
  | { type: "toggleShell" }
  | { type: "toggleGrouped" }
  /** The palette ran this command; it goes to the front of the recent list. */
  | { type: "ranCommand"; id: string }
  /** An action of the side pane's own reducer. */
  | { type: "pane"; action: PaneAction };

/** What a window with nothing saved starts from. Matches the core's `ViewLayout::default()`. */
export const DEFAULT_VIEW: ViewLayout = {
  panes: [null],
  active: 0,
  sidebar: "expanded",
  sidebar_width: SIDEBAR.default,
  shell_hidden: false,
  grouped: false,
  recent_commands: [],
  pane: { open: false, width: null, sessions: {}, recent: [] },
};

/** How many recent commands the palette remembers. */
export const RECENT_LIMIT = 5;

/** The recent list after running `id`: it moves to the front, its duplicates go, and the list is cut to RECENT_LIMIT. */
export function recentAfter(list: readonly string[], id: string): string[] {
  return [id, ...list.filter((x) => x !== id)].slice(0, RECENT_LIMIT);
}

/** The saved sidebar choice; a value this build does not know is the expanded sidebar. */
export function sidebarPrefOf(view: Pick<ViewLayout, "sidebar">): SidebarPref {
  return view.sidebar === "rail" || view.sidebar === "focus" ? view.sidebar : "expanded";
}

/** The sidebar command: the expanded sidebar folds to the rail; the rail, or focus mode, opens the sidebar. */
export function railToggle(pref: SidebarPref): SidebarPref {
  return pref === "expanded" ? "rail" : "expanded";
}

/**
 * What the sidebar button sets: `railToggle`, except that an expanded preference shown as the rail (a window under 960 px wide)
 * cannot open, so there is nothing to set (null).
 */
export function sidebarToggleFor(pref: SidebarPref, mode: SidebarMode): SidebarPref | null {
  if (pref === "expanded" && mode !== "expanded") return null;
  return railToggle(pref);
}

const clamp = (n: number, min: number, max: number) => Math.min(max, Math.max(min, n));

/** Whether the window should put the store's selected session into its pane: it is empty, and a session is selected. */
export function fillsEmptyPane(panes: readonly (string | null)[], selectedId: string | undefined): boolean {
  return panes.length === 1 && panes[0] === null && selectedId !== undefined;
}

function setSidebar(s: ViewState, pref: SidebarPref): ViewState {
  if (pref === s.sidebar) return s;
  const { lastPref: _drop, ...rest } = s;
  // Entering focus remembers where it came from, so leaving it can go back there.
  return pref === "focus" ? { ...rest, sidebar: pref, lastPref: s.sidebar as SidebarPref } : { ...rest, sidebar: pref };
}

export function viewReducer(s: ViewState, a: ViewAction): ViewState {
  switch (a.type) {
    case "select":
      return { ...s, panes: [a.tabId], active: 0 };
    case "sessions": {
      const open = new Set(a.openIds);
      const first = s.panes[0];
      if (first === null || first === undefined || open.has(first)) return s;
      return { ...s, panes: [null] };
    }
    case "sidebar":
      return setSidebar(s, a.pref);
    case "sidebarWidth":
      return Number.isFinite(a.width) ? { ...s, sidebar_width: Math.round(clamp(a.width, SIDEBAR.min, SIDEBAR.max)) } : s;
    case "resetSidebarWidth":
      return { ...s, sidebar_width: SIDEBAR.default };
    case "toggleFocus":
      return setSidebar(s, s.sidebar === "focus" ? (s.lastPref ?? "expanded") : "focus");
    case "toggleShell":
      return { ...s, shell_hidden: !s.shell_hidden };
    case "toggleGrouped":
      return { ...s, grouped: !s.grouped };
    case "ranCommand":
      return { ...s, recent_commands: recentAfter(s.recent_commands, a.id) };
    case "pane": {
      const pane = paneReducer(s.pane, a.action);
      return pane === s.pane ? s : { ...s, pane };
    }
  }
}

/** What goes to layout.json: the state without the part that only lives for this run. */
export function toSaved(s: ViewState): ViewLayout {
  const { lastPref: _drop, ...saved } = s;
  return saved;
}
