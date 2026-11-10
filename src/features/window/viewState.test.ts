import { describe, expect, it } from "vitest";
import type { ViewLayout } from "../../generated/ViewLayout";
import { DEFAULT_VIEW, type ViewAction, type ViewState, fillsEmptyPane, railToggle, recentAfter, sidebarPrefOf, sidebarToggleFor, toSaved, viewReducer } from "./viewState";

const base: ViewLayout = {
  panes: ["a"],
  active: 0,
  sidebar: "expanded",
  sidebar_width: 288,
  shell_hidden: false,
  grouped: false,
  recent_commands: [],
  pane: { open: false, width: null, sessions: {}, recent: [] },
};
const state = (patch: Partial<ViewState> = {}): ViewState => ({ ...base, ...patch });
const run = (s: ViewState, ...actions: ViewAction[]) => actions.reduce(viewReducer, s);

describe("viewReducer", () => {
  it("select puts the session in the one pane", () => {
    const s = run(state({ panes: ["a"] }), { type: "select", tabId: "b" });
    expect(s.panes).toEqual(["b"]);
    expect(s.active).toBe(0);
  });

  it("select leaves one pane, even from a view that holds several", () => {
    const s = run(state({ panes: ["a", "b", "c"], active: 2 }), { type: "select", tabId: "b" });
    expect(s.panes).toEqual(["b"]);
    expect(s.active).toBe(0);
  });

  it("a closed session leaves the pane empty", () => {
    const s = run(state({ panes: ["a"] }), { type: "sessions", openIds: ["b"] });
    expect(s.panes).toEqual([null]);
  });

  it("sessions with the session still open leaves the view unchanged", () => {
    const s = state({ panes: ["a"] });
    expect(run(s, { type: "sessions", openIds: ["a", "d"] })).toBe(s);
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

describe("fillsEmptyPane", () => {
  it("an empty pane takes the store's selection", () => {
    expect(fillsEmptyPane([null], "a")).toBe(true);
    expect(fillsEmptyPane([null], undefined)).toBe(false);
    expect(fillsEmptyPane(["b"], "a")).toBe(false);
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

describe("sidebarToggleFor", () => {
  it("folds the expanded sidebar and opens the rail or focus mode", () => {
    expect(sidebarToggleFor("expanded", "expanded")).toBe("rail");
    expect(sidebarToggleFor("rail", "rail")).toBe("expanded");
    expect(sidebarToggleFor("focus", "focus")).toBe("expanded");
  });
  it("is blocked (null) when the expanded sidebar is auto-collapsed by a narrow window", () => {
    expect(sidebarToggleFor("expanded", "rail")).toBeNull();
  });
});

describe("recent commands", () => {
  it("keeps the last five commands run, newest first, each once", () => {
    expect(recentAfter(["a", "b", "c", "d", "e"], "c")).toEqual(["c", "a", "b", "d", "e"]);
    expect(recentAfter(["a", "b", "c", "d", "e"], "f")).toEqual(["f", "a", "b", "c", "d"]);
    const s = viewReducer(DEFAULT_VIEW, { type: "ranCommand", id: "find" });
    expect(toSaved(s).recent_commands).toEqual(["find"]);
  });
});
