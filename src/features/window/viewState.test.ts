import { describe, expect, it } from "vitest";
import type { ViewLayout } from "../../generated/ViewLayout";
import { RATIO } from "./splitLayout";
import { type ViewAction, type ViewState, ratiosOf, toRatioList, toSaved, viewReducer } from "./viewState";

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
});
