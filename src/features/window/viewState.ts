import type { ViewLayout } from "../../generated/ViewLayout";
import { SIDEBAR, type SidebarPref } from "./layout";
import { ARRANGEMENTS, type Arrangement, PANE_LIMIT, type PaneCount, RATIO, type Ratios } from "./splitLayout";

/** What the window remembers: the saved layout, plus the sidebar pref to return to when focus mode ends (not saved). */
export type ViewState = ViewLayout & { lastPref?: SidebarPref };

export type ViewAction =
  | { type: "split" }
  | { type: "remove"; index: number }
  | { type: "assign"; index: number; tabId: string }
  | { type: "select"; tabId: string }
  | { type: "focusPane"; index: number }
  | { type: "nextPane" }
  | { type: "zoom" }
  | { type: "count"; count: PaneCount }
  /** `null` means automatic. */
  | { type: "arrange"; arrangement: Arrangement | null }
  | { type: "ratios"; ratios: Ratios }
  /** The tabs that are open now; a pane showing any other tab becomes empty. */
  | { type: "sessions"; openIds: string[] }
  | { type: "sidebar"; pref: SidebarPref }
  | { type: "sidebarWidth"; width: number }
  | { type: "resetSidebarWidth" }
  | { type: "toggleFocus" }
  | { type: "toggleShell" }
  | { type: "toggleGrouped" };

/** What a window with nothing saved starts from. Matches the core's `ViewLayout::default()`. */
export const DEFAULT_VIEW: ViewLayout = {
  panes: [null],
  arrangement: null,
  ratios: [],
  active: 0,
  zoomed: null,
  sidebar: "expanded",
  sidebar_width: SIDEBAR.default,
  shell_hidden: false,
  grouped: false,
};

/** The saved `[col, row]` pair as splitLayout's `Ratios`; an empty list means even. */
export function ratiosOf(view: Pick<ViewLayout, "ratios">): Ratios {
  const [col, row] = view.ratios;
  const r: Ratios = {};
  if (col !== undefined) r.col = col;
  if (row !== undefined) r.row = row;
  return r;
}

/** `Ratios` as the saved list: both sides, the missing one even. Nothing set is the empty list. */
export function toRatioList(r: Ratios): number[] {
  if (r.col === undefined && r.row === undefined) return [];
  return [r.col ?? RATIO.default, r.row ?? RATIO.default];
}

const clamp = (n: number, min: number, max: number) => Math.min(max, Math.max(min, n));

/** After the pane count changed: the focus stays on a real pane, and an arrangement that does not suit the count goes back to automatic. */
function fit(s: ViewState, panes: (string | null)[]): ViewState {
  const count = panes.length as PaneCount;
  const arrangement = s.arrangement !== null && !(ARRANGEMENTS[count] as string[]).includes(s.arrangement) ? null : s.arrangement;
  return { ...s, panes, arrangement, active: Math.min(s.active, panes.length - 1) };
}

function setSidebar(s: ViewState, pref: SidebarPref): ViewState {
  if (pref === s.sidebar) return s;
  const { lastPref: _drop, ...rest } = s;
  // Entering focus remembers where it came from, so leaving it can go back there.
  return pref === "focus" ? { ...rest, sidebar: pref, lastPref: s.sidebar as SidebarPref } : { ...rest, sidebar: pref };
}

export function viewReducer(s: ViewState, a: ViewAction): ViewState {
  switch (a.type) {
    case "split": {
      if (s.panes.length >= PANE_LIMIT) return s;
      return { ...fit(s, [...s.panes, null]), active: s.panes.length };
    }
    case "remove": {
      if (s.panes.length <= 1 || a.index < 0 || a.index >= s.panes.length) return s;
      const panes = s.panes.filter((_, i) => i !== a.index);
      const after = (i: number) => (i > a.index ? i - 1 : i);
      return {
        ...fit(s, panes),
        active: Math.min(after(s.active), panes.length - 1),
        zoomed: s.zoomed === null || s.zoomed === a.index ? null : after(s.zoomed),
      };
    }
    case "assign": {
      if (a.index < 0 || a.index >= s.panes.length) return s;
      const from = s.panes.findIndex((t, i) => t === a.tabId && i !== a.index);
      const panes = [...s.panes];
      if (from >= 0) panes[from] = panes[a.index]!;
      panes[a.index] = a.tabId;
      return { ...s, panes };
    }
    case "select": {
      const at = s.panes.indexOf(a.tabId);
      if (at >= 0) return { ...s, active: at };
      const panes = [...s.panes];
      panes[s.active] = a.tabId;
      return { ...s, panes };
    }
    case "focusPane":
      return a.index < 0 || a.index >= s.panes.length ? s : { ...s, active: a.index };
    case "nextPane": {
      const n = s.panes.length;
      for (let step = 1; step < n; step++) {
        const i = (s.active + step) % n;
        if (s.panes[i] !== null) return { ...s, active: i };
      }
      return s;
    }
    case "zoom":
      return { ...s, zoomed: s.zoomed === null ? s.active : null };
    case "count": {
      const panes = s.panes.slice(0, a.count);
      while (panes.length < a.count) panes.push(null);
      return { ...fit(s, panes), zoomed: null };
    }
    case "arrange":
      return { ...s, arrangement: a.arrangement };
    case "ratios":
      return { ...s, ratios: toRatioList(a.ratios) };
    case "sessions": {
      const open = new Set(a.openIds);
      if (s.panes.every((t) => t === null || open.has(t))) return s;
      return { ...s, panes: s.panes.map((t) => (t !== null && !open.has(t) ? null : t)) };
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
  }
}

/** What goes to layout.json: the state without the part that only lives for this run. */
export function toSaved(s: ViewState): ViewLayout {
  const { lastPref: _drop, ...saved } = s;
  return saved;
}
