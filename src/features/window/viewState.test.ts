import { describe, expect, it } from "vitest";
import type { ViewLayout } from "../../generated/ViewLayout";
import { RATIO } from "./splitLayout";
import { type ViewAction, type ViewState, adoptTarget, checkedView, fillsEmptyPane, focusedIndex, paneToggle, railToggle, ratiosOf, roomForPane, sidebarPrefOf, toRatioList, toSaved, viewReducer, visibleSessions } from "./viewState";

const base: ViewLayout = {
  panes: ["a"],
  arrangement: null,
  ratios: [],
  active: 0,
  zoomed: null,
  sidebar: "expanded",
  sidebar_width: 288,
  shell_hidden: false,
  grouped: false,
};
const state = (patch: Partial<ViewState> = {}): ViewState => ({ ...base, ...patch });
const run = (s: ViewState, ...actions: ViewAction[]) => actions.reduce(viewReducer, s);

describe("viewReducer", () => {
  it("split adds an empty pane up to four", () => {
    const four = run(state(), { type: "split" }, { type: "split" }, { type: "split" });
    expect(four.panes).toEqual(["a", null, null, null]);
    expect(four.active).toBe(3);
    expect(run(four, { type: "split" })).toBe(four);
  });

  it("a closed session leaves an empty pane in place", () => {
    const s = run(state({ panes: ["a", "b", "c"], active: 2 }), { type: "sessions", openIds: ["a", "c"] });
    expect(s.panes).toEqual(["a", null, "c"]);
    expect(s.active).toBe(2);
  });

  it("sessions with every id still open leaves panes unchanged", () => {
    const s = run(state({ panes: ["a", "b", "c"], active: 1 }), { type: "sessions", openIds: ["a", "b", "c", "d"] });
    expect(s.panes).toEqual(["a", "b", "c"]);
    expect(s.active).toBe(1);
  });

  it("select on screen focuses that pane", () => {
    const s = run(state({ panes: ["a", "b"], active: 0 }), { type: "select", tabId: "b" });
    expect(s.panes).toEqual(["a", "b"]);
    expect(s.active).toBe(1);
  });

  it("select off screen fills the active pane", () => {
    const s = run(state({ panes: ["a", "b"], active: 1 }), { type: "select", tabId: "c" });
    expect(s.panes).toEqual(["a", "c"]);
    expect(s.active).toBe(1);
  });

  it("assign swaps when the session is already shown", () => {
    const s = run(state({ panes: ["a", "b", null] }), { type: "assign", index: 0, tabId: "b" });
    expect(s.panes).toEqual(["b", "a", null]);
    expect(run(s, { type: "assign", index: 2, tabId: "b" }).panes).toEqual([null, "a", "b"]);
  });

  it("sidebar width clamps to 200..400 and resets to 288", () => {
    expect(run(state(), { type: "sidebarWidth", width: 100 }).sidebar_width).toBe(200);
    expect(run(state(), { type: "sidebarWidth", width: 900 }).sidebar_width).toBe(400);
    expect(run(state(), { type: "sidebarWidth", width: 333.4 }).sidebar_width).toBe(333);
    expect(run(state({ sidebar_width: 350 }), { type: "resetSidebarWidth" }).sidebar_width).toBe(288);
  });

  it("focus toggles back to the previous sidebar pref", () => {
    const focus = run(state({ sidebar: "rail" }), { type: "toggleFocus" });
    expect(focus.sidebar).toBe("focus");
    expect(run(focus, { type: "toggleFocus" }).sidebar).toBe("rail");
    // A file saved in focus mode has no remembered pref: leaving focus opens the sidebar.
    expect(run(state({ sidebar: "focus" }), { type: "toggleFocus" }).sidebar).toBe("expanded");
  });

  it("remove never drops the last pane and keeps active and zoomed pointing at the same panes", () => {
    expect(run(state(), { type: "remove", index: 0 }).panes).toEqual(["a"]);
    const s = run(state({ panes: ["a", "b", "c"], active: 2, zoomed: 2 }), { type: "remove", index: 0 });
    expect(s.panes).toEqual(["b", "c"]);
    expect([s.active, s.zoomed]).toEqual([1, 1]);
    const gone = run(state({ panes: ["a", "b", "c"], active: 1, zoomed: 1 }), { type: "remove", index: 1 });
    expect(gone.panes).toEqual(["a", "c"]);
    expect([gone.active, gone.zoomed]).toEqual([1, null]);
  });

  it("count trims from the end, appends empty panes, clears zoom and drops an arrangement that no longer fits", () => {
    const s = state({ panes: ["a", "b", "c", "d"], active: 3, zoomed: 3, arrangement: "grid" });
    const two = run(s, { type: "count", count: 2 });
    expect(two).toMatchObject({ panes: ["a", "b"], active: 1, zoomed: null, arrangement: null });
    expect(run(two, { type: "count", count: 3 }).panes).toEqual(["a", "b", null]);
    expect(run(state({ panes: ["a", "b"], arrangement: "rows" }), { type: "count", count: 3 }).arrangement).toBe("rows");
  });

  it("nextPane wraps over non-empty panes only", () => {
    const s = state({ panes: ["a", null, "c"], active: 0 });
    const next = run(s, { type: "nextPane" });
    expect(next.active).toBe(2);
    expect(run(next, { type: "nextPane" }).active).toBe(0);
    expect(run(state({ panes: ["a", null], active: 0 }), { type: "nextPane" }).active).toBe(0);
  });

  it("zoom toggles on the active pane", () => {
    const z = run(state({ panes: ["a", "b"], active: 1 }), { type: "zoom" });
    expect(z.zoomed).toBe(1);
    expect(run(z, { type: "zoom" }).zoomed).toBeNull();
  });

  it("focusPane ignores an index that is not a pane", () => {
    const s = state({ panes: ["a", "b"] });
    expect(run(s, { type: "focusPane", index: 1 }).active).toBe(1);
    expect(run(s, { type: "focusPane", index: 5 })).toBe(s);
  });

  it("arrange sets or clears the arrangement", () => {
    const rows = run(state({ panes: ["a", "b"] }), { type: "arrange", arrangement: "rows" });
    expect(rows.arrangement).toBe("rows");
    expect(run(rows, { type: "arrange", arrangement: null }).arrangement).toBeNull();
  });

  it("ratios are stored as [col, row] with the even default for a missing side", () => {
    expect(run(state(), { type: "ratios", ratios: { col: 0.3 } }).ratios).toEqual([0.3, RATIO.default]);
    expect(run(state(), { type: "ratios", ratios: { row: 0.7 } }).ratios).toEqual([RATIO.default, 0.7]);
    expect(run(state({ ratios: [0.3, 0.7] }), { type: "ratios", ratios: {} }).ratios).toEqual([]);
    expect(ratiosOf(state({ ratios: [0.3, 0.7] }))).toEqual({ col: 0.3, row: 0.7 });
    expect(ratiosOf(state())).toEqual({});
    expect(toRatioList({ col: 0.4 })).toEqual([0.4, RATIO.default]);
  });

  it("ratios outside the range are clamped and a non-finite side is even", () => {
    expect(run(state(), { type: "ratios", ratios: { col: 0.05, row: 0.99 } }).ratios).toEqual([RATIO.min, RATIO.max]);
    expect(run(state(), { type: "ratios", ratios: { col: Number.NaN, row: 0.6 } }).ratios).toEqual([RATIO.default, 0.6]);
    expect(run(state({ ratios: [0.3, 0.7] }), { type: "ratios", ratios: { col: Number.NaN, row: Infinity } }).ratios).toEqual([]);
  });

  it("arrange keeps only an arrangement that suits the pane count", () => {
    const two = state({ panes: ["a", "b"] });
    expect(run(two, { type: "arrange", arrangement: "grid" }).arrangement).toBeNull();
    expect(run(two, { type: "arrange", arrangement: "columns" }).arrangement).toBe("columns");
  });

  it("a loaded view keeps only an arrangement that suits its panes", () => {
    const view = { ...base, panes: ["a", "b"], arrangement: "grid" };
    expect(checkedView(view).arrangement).toBeNull();
    expect(checkedView({ ...view, arrangement: "rows" })).toEqual({ ...view, arrangement: "rows" });
    expect(checkedView({ ...view, panes: [] }).arrangement).toBeNull();
  });

  it("the sidebar pref, shell panel and grouping toggle", () => {
    expect(run(state(), { type: "sidebar", pref: "rail" }).sidebar).toBe("rail");
    expect(run(state(), { type: "toggleShell" }).shell_hidden).toBe(true);
    expect(run(state(), { type: "toggleGrouped" }).grouped).toBe(true);
  });

  it("toSaved drops lastPref", () => {
    const saved = toSaved(run(state({ sidebar: "rail" }), { type: "toggleFocus" }));
    expect("lastPref" in saved).toBe(false);
    expect(saved).toEqual({ ...base, sidebar: "focus" });
  });

  it("select while active is hidden fills a visible pane", () => {
    // Two panes are shown of four; the active one is the hidden fourth.
    const s = run(state({ panes: ["a", "b", "c", "d"], active: 3 }), { type: "select", tabId: "x", visible: 2 });
    expect(s.panes).toEqual(["a", "x", "c", "d"]);
    expect(s.active).toBe(1);
  });

  it("select of a tab in a hidden pane swaps it into view", () => {
    const s = run(state({ panes: ["a", "b", "c", "d"], active: 1 }), { type: "select", tabId: "d", visible: 2 });
    expect(s.panes).toEqual(["a", "d", "c", "b"]);
    expect(s.active).toBe(1);
    // The active pane was hidden too: the swap goes into the clamped one.
    const hidden = run(state({ panes: ["a", "b", "c", "d"], active: 3 }), { type: "select", tabId: "c", visible: 2 });
    expect(hidden.panes).toEqual(["a", "c", "b", "d"]);
    expect(hidden.active).toBe(1);
  });

  it("select of a tab in a visible pane still focuses it when visible is given", () => {
    const s = run(state({ panes: ["a", "b", "c"], active: 2 }), { type: "select", tabId: "a", visible: 2 });
    expect(s.panes).toEqual(["a", "b", "c"]);
    expect(s.active).toBe(0);
  });

  it("split at the fit limit does nothing", () => {
    const two = state({ panes: ["a", "b"], active: 0 });
    expect(run(two, { type: "split", fit: 2 })).toBe(two);
    expect(run(two, { type: "split", fit: 1 })).toBe(two);
    expect(run(two, { type: "split", fit: 3 }).panes).toEqual(["a", "b", null]);
  });

  it("roomForPane is true only below both the fit and the limit", () => {
    expect(roomForPane(1, 2)).toBe(true);
    expect(roomForPane(2, 2)).toBe(false);
    expect(roomForPane(3, undefined)).toBe(true);
    expect(roomForPane(4, 9)).toBe(false);
  });

  it("select off screen fills the first empty visible pane and focuses it", () => {
    const s = run(state({ panes: ["a", null], active: 0 }), { type: "select", tabId: "x", visible: 2 });
    expect(s.panes).toEqual(["a", "x"]);
    expect(s.active).toBe(1);
  });

  it("select while zoomed replaces the zoomed pane, not an empty one behind it", () => {
    const s = run(state({ panes: ["a", null], active: 0, zoomed: 0 }), { type: "select", tabId: "x", visible: 2 });
    expect(s.panes).toEqual(["x", null]);
  });
});

describe("adoptTarget", () => {
  const view = (panes: (string | null)[], active = 0): ViewState => state({ panes, active });

  it("takes the first visible empty pane", () => {
    expect(adoptTarget(view(["a", null, null], 0), 3, 4)).toEqual({ index: 1, split: false });
    // An empty pane out of sight does not count.
    expect(adoptTarget(view(["a", "b", null], 0), 2, 2)).toEqual({ index: 0, split: false });
  });

  it("splits when no visible pane is empty and there is room", () => {
    expect(adoptTarget(view(["a", "b"], 1), 2, 3)).toEqual({ index: 2, split: true });
  });

  it("goes into the focused visible pane when there is no room", () => {
    expect(adoptTarget(view(["a", "b"], 1), 2, 2)).toEqual({ index: 1, split: false });
    // The active pane is hidden: the last visible one is focused.
    expect(adoptTarget(view(["a", "b", "c"], 2), 2, 2)).toEqual({ index: 1, split: false });
    expect(adoptTarget(view(["a", "b", "c", "d"], 0), 4, 4)).toEqual({ index: 0, split: false });
  });
});

describe("fillsEmptyPane", () => {
  it("fills the one pane of a window that has nothing in it", () => {
    expect(fillsEmptyPane([null], "c")).toBe(true);
  });
  it("leaves a closed session's pane empty when there are several panes", () => {
    expect(fillsEmptyPane(["a", null], "c")).toBe(false);
    expect(fillsEmptyPane([null, null], "c")).toBe(false);
  });
  it("needs a selection, and an empty pane", () => {
    expect(fillsEmptyPane([null], undefined)).toBe(false);
    expect(fillsEmptyPane(["a"], "c")).toBe(false);
  });
});

describe("focusedIndex", () => {
  it("is the active pane when it is on screen", () => {
    expect(focusedIndex(["a", "b", "c"], 1, null)).toBe(1);
  });
  it("is the last pane on screen when the active one is hidden", () => {
    expect(focusedIndex(["a", "b"], 3, null)).toBe(1);
    expect(focusedIndex(["a", "b"], -1, null)).toBe(0);
    expect(focusedIndex(["a", "b"], Number.NaN, null)).toBe(0);
  });
  it("is the zoomed pane while it is on screen", () => {
    expect(focusedIndex(["a", "b", "c"], 0, 2)).toBe(2);
    expect(focusedIndex(["a", "b"], 0, 3)).toBe(0);
    expect(focusedIndex(["a", null], 0, 1)).toBe(0);
  });
});

describe("sidebarPrefOf and railToggle", () => {
  it("reads the saved sidebar, and anything unknown as expanded", () => {
    expect(sidebarPrefOf({ sidebar: "rail" })).toBe("rail");
    expect(sidebarPrefOf({ sidebar: "focus" })).toBe("focus");
    expect(sidebarPrefOf({ sidebar: "expanded" })).toBe("expanded");
    expect(sidebarPrefOf({ sidebar: "wide" })).toBe("expanded");
  });
  it("the sidebar command folds the expanded sidebar to the rail, and the rail or focus back to expanded", () => {
    expect(railToggle("expanded")).toBe("rail");
    expect(railToggle("rail")).toBe("expanded");
    expect(railToggle("focus")).toBe("expanded");
  });
});

describe("visibleSessions", () => {
  it("lists the sessions in shown panes, skipping empty ones", () => {
    expect(visibleSessions(["a", "b", null], 3, null)).toEqual(["a", "b"]);
    expect(visibleSessions(["a", "b", "c"], 2, null)).toEqual(["a", "b"]);
  });

  it("lists only the zoomed pane while one is zoomed", () => {
    expect(visibleSessions(["a", "b"], 2, 1)).toEqual(["b"]);
    // A zoomed pane that isn't shown is ignored.
    expect(visibleSessions(["a", "b", "c"], 2, 2)).toEqual(["a", "b"]);
  });
});

describe("paneToggle", () => {
  const view = (panes: (string | null)[], zoomed: number | null = null) => ({ panes, active: 0, zoomed });

  it("removes a visible session's pane", () => {
    expect(paneToggle(view(["a", "b"]), "b", 2, 2)).toEqual({ kind: "remove", index: 1 });
  });

  it("does nothing to the only pane", () => {
    expect(paneToggle(view(["a"]), "a", 1, 2)).toEqual({ kind: "only" });
  });

  it("shows a session from a hidden pane", () => {
    expect(paneToggle(view(["a", "b", "c"]), "c", 2, 2)).toEqual({ kind: "show" });
    // Zoomed on a: b's pane is not visible.
    expect(paneToggle(view(["a", "b"], 0), "b", 2, 2)).toEqual({ kind: "show" });
  });

  it("shows a session in an empty or a new pane", () => {
    expect(paneToggle(view(["a", null]), "x", 2, 2)).toEqual({ kind: "show" });
    expect(paneToggle(view(["a"]), "x", 1, 2)).toEqual({ kind: "show" });
  });

  it("reports a full window", () => {
    expect(paneToggle(view(["a", "b"]), "x", 2, 2)).toEqual({ kind: "full" });
  });
});
