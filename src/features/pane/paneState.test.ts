import { describe, expect, it } from "vitest";
import type { PaneView } from "../../generated/PaneView";
import {
  type PaneAction,
  activeTab,
  keptTabs,
  openRefused,
  paneReducer,
  recentFor,
  samePath,
  tabTitles,
  tabsOf,
} from "./paneState";

const EMPTY: PaneView = { open: false, width: null, sessions: {}, recent: [] };
const run = (s: PaneView, ...actions: PaneAction[]) => actions.reduce(paneReducer, s);
const open = (path: string, o: { session?: string; project?: string; line?: number; col?: number } = {}): PaneAction => ({
  type: "open",
  session: o.session ?? "s",
  project: o.project ?? "p",
  target: { path, line: o.line, col: o.col },
});
const ids = (s: PaneView, session = "s") => tabsOf(s, session).tabs.map((t) => t.id);
const close = (paneTab: string, session = "s"): PaneAction => ({ type: "close", session, paneTab });

describe("paneReducer", () => {
  it("an open makes a tab, selects it and shows the pane", () => {
    const s = run(EMPTY, open("/a.md"));
    const t = tabsOf(s, "s");
    expect(t.tabs).toEqual([
      { id: "t1", kind: "file", history: [{ path: "/a.md", line: null, col: null, view: "rendered" }], index: 0 },
    ]);
    expect(t.active).toBe("t1");
    expect(s.open).toBe(true);
  });

  it("a new tab's id is one past the highest id open in the session", () => {
    let s = run(EMPTY, open("/a"), open("/b"), open("/c"));
    expect(ids(s)).toEqual(["t1", "t2", "t3"]);
    s = run(s, close("t2"), open("/d"));
    expect(ids(s)).toEqual(["t1", "t3", "t4"]);
    s = run(s, close("t4"), close("t3"), open("/e"));
    expect(ids(s)).toEqual(["t1", "t2"]);
  });

  it("an open of a path already open selects its tab", () => {
    const s = run(EMPTY, open("/a"), open("/b"), open("/a"));
    expect(ids(s)).toEqual(["t1", "t2"]);
    expect(tabsOf(s, "s").active).toBe("t1");
  });

  it("a Windows path matches whatever its case and slashes", () => {
    expect(samePath("C:\\work\\README.md", "c:/work/readme.md")).toBe(true);
    expect(samePath("/home/a/B.md", "/home/a/b.md")).toBe(false);
    const s = run(EMPTY, open("C:\\work\\README.md"), open("c:/work/readme.md"));
    expect(ids(s)).toEqual(["t1"]);
  });

  it("a second open with a line moves the open tab to it", () => {
    const s = run(EMPTY, open("/a.ts", { line: 1 }), open("/a.ts", { line: 42, col: 7 }));
    const tab = activeTab(s, "s")!;
    expect(tab.history).toHaveLength(1);
    expect(tab.history[0]).toMatchObject({ line: 42, col: 7 });
  });

  it("a line or column too large to be one is left out, so the layout can still be saved", () => {
    // `app.log:1731234567890` reads as a path and a line; the core's layout holds a line as a u32 and refuses the whole save.
    const s = run(EMPTY, open("/app.log", { line: 1731234567890, col: 4294967296 }));
    expect(activeTab(s, "s")!.history[0]).toMatchObject({ path: "/app.log", line: null, col: null });
    const top = run(EMPTY, open("/b.log", { line: 4294967295, col: 1 }));
    expect(activeTab(top, "s")!.history[0]).toMatchObject({ line: 4294967295, col: 1 });
    const odd = run(EMPTY, open("/c.log", { line: -3, col: 1.5 }));
    expect(activeTab(odd, "s")!.history[0]).toMatchObject({ line: null, col: null });
  });

  it("a second open with a line too large keeps the tab's place", () => {
    const s = run(EMPTY, open("/a.ts", { line: 7 }), open("/a.ts", { line: 1731234567890 }));
    expect(activeTab(s, "s")!.history).toEqual([{ path: "/a.ts", line: 7, col: null, view: "rendered" }]);
    const col = run(EMPTY, open("/a.ts", { line: 7 }), open("/a.ts", { line: 9, col: 1731234567890 }));
    expect(activeTab(col, "s")!.history[0]).toMatchObject({ line: 9, col: null });
  });

  it("a link inside a tab to a line too large moves the tab without the line", () => {
    const s = run(EMPTY, open("/a.md"), { type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path: "/b.log", line: 1731234567890 } });
    expect(activeTab(s, "s")!.history[1]).toMatchObject({ path: "/b.log", line: null });
  });

  it("the 33rd open is refused", () => {
    let s = EMPTY;
    for (let i = 0; i < 32; i++) s = run(s, open(`/f${i}`));
    expect(ids(s)).toHaveLength(32);
    expect(openRefused(s, "s", "/extra")).toBe(true);
    expect(openRefused(s, "s", "/f3")).toBe(false);
    expect(paneReducer(s, open("/extra"))).toBe(s);
    expect(ids(paneReducer(s, open("/f3")))).toHaveLength(32);
  });

  it("navigate pushes onto the tab's history and drops what was forward", () => {
    const nav = (path: string): PaneAction => ({ type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path } });
    let s = run(EMPTY, open("/a"), nav("/b"), nav("/c"));
    expect(activeTab(s, "s")!.history.map((e) => e.path)).toEqual(["/a", "/b", "/c"]);
    s = run(s, { type: "back", session: "s", paneTab: "t1" }, nav("/d"));
    const t = activeTab(s, "s")!;
    expect(t.history.map((e) => e.path)).toEqual(["/a", "/b", "/d"]);
    expect(t.index).toBe(2);
  });

  it("a history keeps its last 50 entries", () => {
    let s = run(EMPTY, open("/0"));
    for (let i = 1; i < 60; i++) s = run(s, { type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path: `/${i}` } });
    const t = activeTab(s, "s")!;
    expect(t.history).toHaveLength(50);
    expect(t.history[0]!.path).toBe("/10");
    expect(t.history[49]!.path).toBe("/59");
    expect(t.index).toBe(49);
  });

  it("back and forward stop at the ends", () => {
    const back: PaneAction = { type: "back", session: "s", paneTab: "t1" };
    const fwd: PaneAction = { type: "forward", session: "s", paneTab: "t1" };
    const one = run(EMPTY, open("/a"));
    expect(paneReducer(one, back)).toBe(one);
    expect(paneReducer(one, fwd)).toBe(one);
    const two = run(one, { type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path: "/b" } });
    expect(paneReducer(two, fwd)).toBe(two);
    const back1 = paneReducer(two, back);
    expect(activeTab(back1, "s")!.index).toBe(0);
    expect(paneReducer(back1, back)).toBe(back1);
    expect(activeTab(paneReducer(back1, fwd), "s")!.index).toBe(1);
  });

  it("setView changes only the current entry", () => {
    const nav: PaneAction = { type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path: "/b" } };
    let s = run(EMPTY, open("/a"), nav);
    s = run(s, { type: "setView", session: "s", paneTab: "t1", view: "source" });
    expect(activeTab(s, "s")!.history.map((e) => e.view)).toEqual(["rendered", "source"]);
    expect(paneReducer(s, { type: "setView", session: "s", paneTab: "t1", view: "source" })).toBe(s);
  });

  it("step wraps both ways and does nothing with no tabs", () => {
    expect(paneReducer(EMPTY, { type: "step", session: "s", by: 1 })).toBe(EMPTY);
    const s = run(EMPTY, open("/a"), open("/b"), open("/c"));
    expect(tabsOf(s, "s").active).toBe("t3");
    const next = paneReducer(s, { type: "step", session: "s", by: 1 });
    expect(tabsOf(next, "s").active).toBe("t1");
    expect(tabsOf(paneReducer(next, { type: "step", session: "s", by: -1 }), "s").active).toBe("t3");
  });

  it("closing the selected tab selects the one to its right, else the one to its left", () => {
    const sel = (paneTab: string): PaneAction => ({ type: "select", session: "s", paneTab });
    let s = run(EMPTY, open("/a"), open("/b"), open("/c"), sel("t2"), close("t2"));
    expect(tabsOf(s, "s").active).toBe("t3");
    s = run(s, close("t3"));
    expect(tabsOf(s, "s").active).toBe("t1");
    // Closing a tab that is not selected keeps the selection.
    s = run(EMPTY, open("/a"), open("/b"), close("t1"));
    expect(tabsOf(s, "s").active).toBe("t2");
  });

  it("closing the last tab removes the session's entry and leaves the pane open", () => {
    const s = run(EMPTY, open("/a"), close("t1"));
    expect("s" in s.sessions).toBe(false);
    expect(s.open).toBe(true);
    expect(tabsOf(s, "s")).toEqual({ tabs: [], active: null });
  });

  it("move puts the tab at the index and keeps the selection", () => {
    const mv = (paneTab: string, to: number): PaneAction => ({ type: "move", session: "s", paneTab, to });
    let s = run(EMPTY, open("/a"), open("/b"), open("/c"));
    s = run(s, mv("t3", 0));
    expect(ids(s)).toEqual(["t3", "t1", "t2"]);
    expect(tabsOf(s, "s").active).toBe("t3");
    expect(ids(run(s, mv("t3", 99)))).toEqual(["t1", "t2", "t3"]);
    expect(ids(run(s, mv("t1", -5)))).toEqual(["t1", "t3", "t2"]);
    expect(paneReducer(s, mv("t3", 0))).toBe(s);
  });

  it("toggle, show and hide set open", () => {
    const shown = paneReducer(EMPTY, { type: "show" });
    expect(shown.open).toBe(true);
    expect(paneReducer(shown, { type: "show" })).toBe(shown);
    expect(paneReducer(shown, { type: "hide" }).open).toBe(false);
    expect(paneReducer(EMPTY, { type: "hide" })).toBe(EMPTY);
    expect(paneReducer(EMPTY, { type: "toggle" }).open).toBe(true);
    expect(paneReducer(shown, { type: "toggle" }).open).toBe(false);
  });

  it("width keeps null and rounds a number", () => {
    expect(paneReducer(EMPTY, { type: "width", width: null })).toBe(EMPTY);
    expect(paneReducer(EMPTY, { type: "width", width: 400.6 }).width).toBe(401);
    expect(paneReducer(EMPTY, { type: "width", width: 100 }).width).toBe(320);
    expect(paneReducer(EMPTY, { type: "width", width: 9000 }).width).toBe(4096);
    const w = paneReducer(EMPTY, { type: "width", width: 500 });
    expect(paneReducer(w, { type: "width", width: 500 })).toBe(w);
    expect(paneReducer(w, { type: "width", width: Number.NaN })).toBe(w);
    expect(paneReducer(w, { type: "width", width: null }).width).toBeNull();
  });

  it("every open and navigate puts the file first in recent, once per project", () => {
    let s = run(EMPTY, open("/a"), open("/b"), open("/a"));
    expect(s.recent).toEqual([
      { project: "p", path: "/a" },
      { project: "p", path: "/b" },
    ]);
    s = run(s, open("/a", { project: "q" }));
    expect(s.recent).toHaveLength(3);
    s = run(s, { type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path: "/n" } });
    expect(s.recent[0]).toEqual({ project: "p", path: "/n" });
    let big = EMPTY;
    for (let i = 0; i < 210; i++) big = run(big, open(`/f${i}`, { session: `s${i % 10}` }));
    expect(big.recent).toHaveLength(200);
    expect(big.recent[0]!.path).toBe("/f209");
  });

  it("recentFor leaves out other projects and files open in the session, and stops at 10", () => {
    let s = EMPTY;
    for (let i = 0; i < 14; i++) s = run(s, open(`/f${i}`, { session: "other" }));
    s = run(s, open("/x", { project: "q", session: "other" }), open("/f13", { session: "s" }));
    const r = recentFor(s, "p", "s");
    expect(r).toHaveLength(10);
    expect(r).not.toContain("/f13");
    expect(r).not.toContain("/x");
    expect(r[0]).toBe("/f12");
  });

  it("tabTitles adds the folder only to tabs that share a name", () => {
    const s = run(EMPTY, open("docs/README.md"), open("api\\README.md"), open("/src/a.ts"));
    const titles = tabTitles(tabsOf(s, "s").tabs);
    expect(titles.get("t1")).toBe("README.md — docs");
    expect(titles.get("t2")).toBe("README.md — api");
    expect(titles.get("t3")).toBe("a.ts");
  });

  it("keptTabs keeps the active tab and the three shown before it", () => {
    expect(keptTabs([], null, ["a"])).toEqual([]);
    expect(keptTabs(["a", "b", "c", "d"], "e", ["a", "b", "c", "d", "e"])).toEqual(["e", "a", "b", "c"]);
    expect(keptTabs(["a", "b"], "b", ["a", "b"])).toEqual(["b", "a"]);
  });

  it("keptTabs forgets a tab that was closed", () => {
    expect(keptTabs(["a", "b", "c"], "d", ["a", "c", "d"])).toEqual(["d", "a", "c"]);
  });

  it("an action for a tab or session that isn't there changes nothing", () => {
    const s = run(EMPTY, open("/a"));
    const actions: PaneAction[] = [
      { type: "back", session: "s", paneTab: "zz" },
      { type: "forward", session: "nope", paneTab: "t1" },
      { type: "setView", session: "s", paneTab: "zz", view: "source" },
      { type: "select", session: "s", paneTab: "zz" },
      { type: "select", session: "nope", paneTab: "t1" },
      { type: "step", session: "nope", by: 1 },
      { type: "close", session: "s", paneTab: "zz" },
      { type: "close", session: "nope", paneTab: "t1" },
      { type: "move", session: "s", paneTab: "zz", to: 0 },
      { type: "navigate", session: "s", paneTab: "zz", project: "p", target: { path: "/b" } },
    ];
    for (const a of actions) expect(paneReducer(s, a)).toBe(s);
  });
});
