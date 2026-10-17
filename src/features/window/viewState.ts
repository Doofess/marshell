import type { ViewLayout } from "../../generated/ViewLayout";
import { SIDEBAR, type SidebarPref } from "./layout";
import { ARRANGEMENTS, type Arrangement, PANE_LIMIT, type PaneCount, RATIO, type Ratios } from "./splitLayout";

/** What the window remembers: the saved layout, plus the sidebar pref to return to when focus mode ends (not saved). */
export type ViewState = ViewLayout & { lastPref?: SidebarPref };

export type ViewAction =
  /** `fit` is how many panes the window can show now; a split past it does nothing. */
  | { type: "split"; fit?: number }
  | { type: "remove"; index: number }
  | { type: "assign"; index: number; tabId: string }
  /** `visible` is how many panes the window shows; the selection then never lands in a pane out of sight. */
  | { type: "select"; tabId: string; visible?: number }
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

/** The saved sidebar choice; a value this build does not know is the expanded sidebar. */
export function sidebarPrefOf(view: Pick<ViewLayout, "sidebar">): SidebarPref {
  return view.sidebar === "rail" || view.sidebar === "focus" ? view.sidebar : "expanded";
}

/** The sidebar command: the expanded sidebar folds to the rail; the rail, or focus mode, opens the sidebar. */
export function railToggle(pref: SidebarPref): SidebarPref {
  return pref === "expanded" ? "rail" : "expanded";
}

/** The saved `[col, row]` pair as splitLayout's `Ratios`; an empty list means even. */
export function ratiosOf(view: Pick<ViewLayout, "ratios">): Ratios {
  const [col, row] = view.ratios;
  const r: Ratios = {};
  if (col !== undefined) r.col = col;
  if (row !== undefined) r.row = row;
  return r;
}

/** A ratio within RATIO.min..RATIO.max, or undefined for a missing or non-finite one. */
const cleanRatio = (n: number | undefined): number | undefined =>
  n === undefined || !Number.isFinite(n) ? undefined : Math.min(RATIO.max, Math.max(RATIO.min, n));

/**
 * `Ratios` as the saved list: both sides, each kept within RATIO.min..RATIO.max. A missing or non-finite side is even;
 * when neither side is usable the list is empty.
 */
export function toRatioList(r: Ratios): number[] {
  const col = cleanRatio(r.col);
  const row = cleanRatio(r.row);
  if (col === undefined && row === undefined) return [];
  return [col ?? RATIO.default, row ?? RATIO.default];
}

/** The arrangement if it suits the number of panes, else null (automatic). */
export function arrangementFor(panes: readonly unknown[], arrangement: string | null): string | null {
  const allowed = ARRANGEMENTS[panes.length as PaneCount] as string[] | undefined;
  return arrangement !== null && allowed?.includes(arrangement) ? arrangement : null;
}

/** A view read from the core, with an arrangement that does not suit its panes set back to automatic. */
export function checkedView(view: ViewLayout): ViewLayout {
  const arrangement = arrangementFor(view.panes, view.arrangement);
  return arrangement === view.arrangement ? view : { ...view, arrangement };
}

const clamp = (n: number, min: number, max: number) => Math.min(max, Math.max(min, n));

/**
 * Whether the window should put the store's selected session into an empty pane. Only a window of one empty pane does: it has
 * nothing else to show. In a split, a pane whose session closed stays empty, for the user to fill.
 */
export function fillsEmptyPane(panes: readonly (string | null)[], selectedId: string | undefined): boolean {
  return panes.length === 1 && panes[0] === null && selectedId !== undefined;
}

/**
 * Which of the panes on screen is the focused one: the zoomed pane while it is shown, else the active pane, held to the panes
 * that are shown (the window can hold fewer than the saved layout). The window and the app both ask this, so they agree.
 */
export function focusedIndex(shown: readonly unknown[], active: number, zoomed: number | null | undefined): number {
  if (zoomed !== null && zoomed !== undefined && shown[zoomed] != null) return zoomed;
  return Math.min(Math.max(0, Math.floor(Number.isFinite(active) ? active : 0)), Math.max(0, shown.length - 1));
}

/** Whether one more pane can be added: below the pane limit, and below how many the window can show (`fit`, when known). */
export function roomForPane(count: number, fit: number | undefined): boolean {
  return count < Math.min(PANE_LIMIT, fit ?? PANE_LIMIT);
}

/**
 * Where "Open in a pane" puts a new session: the first empty pane on screen; else a new pane, if the window has room; else the
 * focused pane on screen (the session it held stays in the sidebar). `split` means the pane at `index` is yet to be added.
 */
export function adoptTarget(view: Pick<ViewLayout, "panes" | "active" | "zoomed">, visible: number, fit: number): { index: number; split: boolean } {
  const shown = view.panes.slice(0, visible);
  const empty = shown.indexOf(null);
  if (empty >= 0) return { index: empty, split: false };
  if (roomForPane(view.panes.length, fit)) return { index: view.panes.length, split: true };
  return { index: focusedIndex(shown, view.active, view.zoomed), split: false };
}

/** The sessions the window shows now: only the zoomed pane's while one is zoomed, and none from a pane out of sight. */
export function visibleSessions(panes: readonly (string | null)[], shown: number, zoomed: number | null | undefined): string[] {
  const inSight = panes.slice(0, shown);
  const zoomedId = zoomed !== null && zoomed !== undefined ? inSight[zoomed] : undefined;
  if (zoomedId) return [zoomedId];
  return inSight.filter((id): id is string => id !== null);
}

export type PaneToggle = { kind: "remove"; index: number } | { kind: "only" } | { kind: "show" } | { kind: "full" };

/** What a Ctrl+click on a session's row does: drop its pane if it is visible, otherwise bring it into view if there is room. */
export function paneToggle(view: Pick<ViewLayout, "panes" | "active" | "zoomed">, id: string, shown: number, fit: number): PaneToggle {
  if (visibleSessions(view.panes, shown, view.zoomed).includes(id)) {
    return view.panes.length > 1 ? { kind: "remove", index: view.panes.indexOf(id) } : { kind: "only" };
  }
  if (view.panes.includes(id)) return { kind: "show" };
  const target = adoptTarget(view, shown, fit);
  return target.split || view.panes[target.index] === null ? { kind: "show" } : { kind: "full" };
}

/** After the pane count changed: the focus stays on a real pane, and an arrangement that does not suit the count goes back to automatic. */
function fit(s: ViewState, panes: (string | null)[]): ViewState {
  return { ...s, panes, arrangement: arrangementFor(panes, s.arrangement), active: Math.min(s.active, panes.length - 1) };
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
      if (!roomForPane(s.panes.length, a.fit)) return s;
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
      if (a.visible === undefined) {
        if (at >= 0) return { ...s, active: at };
        const panes = [...s.panes];
        panes[s.active] = a.tabId;
        return { ...s, panes };
      }
      // The active pane may be out of sight (the window shrank, or a pane was added past what fits): the selection goes to one on screen.
      const last = clamp(Math.floor(a.visible) - 1, 0, s.panes.length - 1);
      // While a shown pane is zoomed it is the focused one; the zoom moves with the selection, so the session picked is the one on screen.
      const shown = s.panes.slice(0, last + 1);
      const zoomed = s.zoomed !== null && shown[s.zoomed] != null;
      const active = focusedIndex(shown, s.active, s.zoomed);
      if (at >= 0 && at <= last) return { ...s, active: at, zoomed: zoomed ? at : s.zoomed };
      const empty = s.zoomed === null ? s.panes.slice(0, last + 1).indexOf(null) : -1;
      if (at < 0 && empty >= 0) {
        const panes = [...s.panes];
        panes[empty] = a.tabId;
        return { ...s, panes, active: empty };
      }
      const panes = [...s.panes];
      // A session in a pane out of sight trades places with the one in the focused pane, so neither is dropped.
      if (at >= 0) panes[at] = panes[active]!;
      panes[active] = a.tabId;
      return { ...s, panes, active };
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
      return { ...s, arrangement: arrangementFor(s.panes, a.arrangement) };
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
