import { describe, expect, it } from "vitest";
import type { PaneTab } from "../../generated/PaneTab";
import type { PaneView } from "../../generated/PaneView";
import type { SessionTabs } from "../../generated/SessionTabs";
import { type PanelId, PANELS, activeIn, onScreen, stackOf, stackPanels } from "./panels";
import { paneProblem } from "./paneRules";
import {
  type OpenTarget,
  type PaneAction,
  activeTab,
  currentEntry,
  placeOf,
  isBrowserTab,
  drawnTabs,
  keptTabs,
  openRefused,
  paneReducer,
  previewTab,
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
      { id: "t1", kind: "file", history: [{ path: "/a.md", line: null, col: null, view: "rendered" }], index: 0, title: null, opened_by: null, unseen: false },
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
    const tab = activeTab(s, "s", "files")!;
    expect(tab.history).toHaveLength(1);
    expect(tab.history[0]).toMatchObject({ line: 42, col: 7 });
  });

  it("a line or column too large to be one is left out, so the layout can still be saved", () => {
    // `app.log:1731234567890` reads as a path and a line; the core's layout holds a line as a u32 and refuses the whole save.
    const s = run(EMPTY, open("/app.log", { line: 1731234567890, col: 4294967296 }));
    expect(activeTab(s, "s", "files")!.history[0]).toMatchObject({ path: "/app.log", line: null, col: null });
    const top = run(EMPTY, open("/b.log", { line: 4294967295, col: 1 }));
    expect(activeTab(top, "s", "files")!.history[0]).toMatchObject({ line: 4294967295, col: 1 });
    const odd = run(EMPTY, open("/c.log", { line: -3, col: 1.5 }));
    expect(activeTab(odd, "s", "files")!.history[0]).toMatchObject({ line: null, col: null });
  });

  it("a second open with a line too large keeps the tab's place", () => {
    const s = run(EMPTY, open("/a.ts", { line: 7 }), open("/a.ts", { line: 1731234567890 }));
    expect(activeTab(s, "s", "files")!.history).toEqual([{ path: "/a.ts", line: 7, col: null, view: "rendered" }]);
    const col = run(EMPTY, open("/a.ts", { line: 7 }), open("/a.ts", { line: 9, col: 1731234567890 }));
    expect(activeTab(col, "s", "files")!.history[0]).toMatchObject({ line: 9, col: null });
  });

  it("a link inside a tab to a line too large moves the tab without the line", () => {
    const s = run(EMPTY, open("/a.md"), { type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path: "/b.log", line: 1731234567890 } });
    expect(activeTab(s, "s", "files")!.history[1]).toMatchObject({ path: "/b.log", line: null });
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
    expect(activeTab(s, "s", "files")!.history.map((e) => e.path)).toEqual(["/a", "/b", "/c"]);
    s = run(s, { type: "back", session: "s", paneTab: "t1" }, nav("/d"));
    const t = activeTab(s, "s", "files")!;
    expect(t.history.map((e) => e.path)).toEqual(["/a", "/b", "/d"]);
    expect(t.index).toBe(2);
  });

  it("a history keeps its last 50 entries", () => {
    let s = run(EMPTY, open("/0"));
    for (let i = 1; i < 60; i++) s = run(s, { type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path: `/${i}` } });
    const t = activeTab(s, "s", "files")!;
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
    expect(activeTab(back1, "s", "files")!.index).toBe(0);
    expect(paneReducer(back1, back)).toBe(back1);
    expect(activeTab(paneReducer(back1, fwd), "s", "files")!.index).toBe(1);
  });

  it("setView changes only the current entry", () => {
    const nav: PaneAction = { type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path: "/b" } };
    let s = run(EMPTY, open("/a"), nav);
    s = run(s, { type: "setView", session: "s", paneTab: "t1", view: "source" });
    expect(activeTab(s, "s", "files")!.history.map((e) => e.view)).toEqual(["rendered", "source"]);
    expect(paneReducer(s, { type: "setView", session: "s", paneTab: "t1", view: "source" })).toBe(s);
  });

  it("step wraps both ways and does nothing with no tabs", () => {
    expect(paneReducer(EMPTY, { type: "step", session: "s", panel: "files", by: 1 })).toBe(EMPTY);
    const s = run(EMPTY, open("/a"), open("/b"), open("/c"));
    expect(tabsOf(s, "s").active).toBe("t3");
    const next = paneReducer(s, { type: "step", session: "s", panel: "files", by: 1 });
    expect(tabsOf(next, "s").active).toBe("t1");
    expect(tabsOf(paneReducer(next, { type: "step", session: "s", panel: "files", by: -1 }), "s").active).toBe("t3");
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
    const shown = paneReducer(EMPTY, { type: "show", session: "s" });
    expect(shown.open).toBe(true);
    expect(paneReducer(shown, { type: "show", session: "s" })).toBe(shown);
    expect(paneReducer(shown, { type: "hide" }).open).toBe(false);
    expect(paneReducer(EMPTY, { type: "hide" })).toBe(EMPTY);
    expect(paneReducer(EMPTY, { type: "toggle", session: "s" }).open).toBe(true);
    expect(paneReducer(shown, { type: "toggle", session: "s" }).open).toBe(false);
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

  it("drawnTabs draws one tab for a file: the selected one, else the first that shows it", () => {
    // A link followed in t2, or Back, can bring it to the file t1 shows. One file has one document, one draft and one dot.
    let s = run(EMPTY, open("C:/p/readme.md"), open("C:/p/notes.md"), open("C:/p/other.md"));
    s = run(s, { type: "navigate", session: "s", paneTab: "t3", project: "p", target: { path: "c:/P/readme.md" } });
    const tabs = tabsOf(s, "s").tabs;
    expect(tabs.map((t) => t.id)).toEqual(["t1", "t2", "t3"]);
    expect(drawnTabs(tabs, ["t3", "t2", "t1"], "t3")).toEqual(["t2", "t3"]);
    expect(drawnTabs(tabs, ["t1", "t3", "t2"], "t1")).toEqual(["t1", "t2"]);
    expect(drawnTabs(tabs, ["t2", "t3", "t1"], "t2")).toEqual(["t1", "t2"]);
  });

  it("drawnTabs draws every kept tab when each shows its own file, and never drops a browser tab", () => {
    let s = run(EMPTY, open("/a"), open("/b"), { type: "openPage", session: "s", url: "http://localhost:3000/" });
    s = run(s, { type: "openPage", session: "s", url: "http://localhost:3000/x" });
    const tabs = tabsOf(s, "s").tabs;
    expect(drawnTabs(tabs, ["t4", "t3", "t2", "t1"], "t4")).toEqual(["t1", "t2", "t3", "t4"]);
    expect(drawnTabs(tabs, ["t1"], "t1")).toEqual(["t1"]);
  });

  it("an action for a tab or session that isn't there changes nothing", () => {
    const s = run(EMPTY, open("/a"));
    const actions: PaneAction[] = [
      { type: "back", session: "s", paneTab: "zz" },
      { type: "forward", session: "nope", paneTab: "t1" },
      { type: "setView", session: "s", paneTab: "zz", view: "source" },
      { type: "select", session: "s", paneTab: "zz" },
      { type: "select", session: "nope", paneTab: "t1" },
      { type: "step", session: "nope", panel: "files", by: 1 },
      { type: "close", session: "s", paneTab: "zz" },
      { type: "close", session: "nope", paneTab: "t1" },
      { type: "move", session: "s", paneTab: "zz", to: 0 },
      { type: "navigate", session: "s", paneTab: "zz", project: "p", target: { path: "/b" } },
    ];
    for (const a of actions) expect(paneReducer(s, a)).toBe(s);
  });
});

describe("browser tabs", () => {
  const page = (url: string, session = "s"): PaneAction => ({ type: "openPage", session, url });
  const ev = (paneTab: string, url: string, title: string | null, session = "s"): PaneAction => ({ type: "page", session, paneTab, url, title });

  it("openPage makes a browser tab and shows the pane", () => {
    const s = run(EMPTY, page("https://a.dev/"));
    expect(tabsOf(s, "s")).toEqual({
      tabs: [{ id: "t1", kind: "browser", history: [{ path: "https://a.dev/", line: null, col: null, view: "rendered" }], index: 0, title: null, opened_by: null, unseen: false }],
      active: "t1",
      active_browser: "t1",
    });
    expect(s.open).toBe(true);
    expect(isBrowserTab(activeTab(s, "s", "browser")!)).toBe(true);
  });

  it("openPage of an address already open selects its tab", () => {
    const s = run(EMPTY, page("https://a.dev/"), open("/f"), page("https://b.dev/"), page("https://A.dev"));
    expect(ids(s)).toEqual(["t1", "t2", "t3"]);
    expect(activeIn(tabsOf(s, "s"), "browser")).toBe("t1");
    const hidden = run(s, { type: "hide" }, page("https://b.dev/"));
    expect(hidden.open).toBe(true);
    expect(activeIn(tabsOf(hidden, "s"), "browser")).toBe("t3");
  });

  it("openPage of about:blank always makes a new tab", () => {
    const s = run(EMPTY, page("about:blank"), page("about:blank"));
    expect(ids(s)).toEqual(["t1", "t2"]);
    expect(activeIn(tabsOf(s, "s"), "browser")).toBe("t2");
  });

  it("openPage is refused at 32 tabs", () => {
    let s = EMPTY;
    for (let i = 0; i < 32; i++) s = run(s, page(`https://a.dev/${i}`));
    expect(ids(s)).toHaveLength(32);
    expect(openRefused(s, "s", "https://a.dev/extra")).toBe(true);
    expect(openRefused(s, "s", "https://A.dev/3")).toBe(false);
    expect(paneReducer(s, page("https://a.dev/extra"))).toBe(s);
    expect(paneReducer(s, page("about:blank"))).toBe(s);
    expect(activeIn(tabsOf(paneReducer(s, page("https://a.dev/3")), "s"), "browser")).toBe("t4");
  });

  it("openPage refuses an address that is not a page", () => {
    for (const url of ["file:///C:/x", "javascript:alert(1)", "http://u:p@a.dev/", "c:\\x", ""]) {
      expect(paneReducer(EMPTY, page(url))).toBe(EMPTY);
    }
  });

  it("openPage leaves the recent files alone", () => {
    const s = run(EMPTY, open("/a"), page("https://a.dev/"));
    expect(s.recent).toEqual([{ project: "p", path: "/a" }]);
    expect(run(EMPTY, page("https://a.dev/")).recent).toEqual([]);
    expect(recentFor(s, "p", "other")).toEqual(["/a"]);
    const withUrl = { ...EMPTY, recent: [{ project: "p", path: "https://a.dev/" }, { project: "p", path: "/a" }] };
    expect(recentFor(withUrl, "p", "s")).toEqual(["/a"]);
  });

  it("a page event moves the tab's address and title", () => {
    const s = run(EMPTY, page("https://a.dev/"), ev("t1", "https://a.dev/next", "Next"));
    const t = activeTab(s, "s", "browser")!;
    expect(t.history).toEqual([{ path: "https://a.dev/next", line: null, col: null, view: "rendered" }]);
    expect(t.index).toBe(0);
    expect(t.title).toBe("Next");
    const long = run(s, ev("t1", "https://a.dev/next", "x".repeat(300)));
    expect(activeTab(long, "s", "browser")!.title).toHaveLength(200);
    expect(activeTab(run(s, ev("t1", "https://a.dev/next", null)), "s", "browser")!.title).toBeNull();
  });

  it("a page event with the same address and title returns the same state", () => {
    const s = run(EMPTY, page("https://a.dev/"), ev("t1", "https://a.dev/", "A"));
    expect(paneReducer(s, ev("t1", "https://a.dev/", "A"))).toBe(s);
    expect(paneReducer(s, ev("t1", "https://a.dev/", "x".repeat(300)))).not.toBe(s);
  });

  it("a page event for a file tab is ignored", () => {
    const s = run(EMPTY, open("/a"));
    expect(paneReducer(s, ev("t1", "https://a.dev/", "A"))).toBe(s);
  });

  it("a page event with an address that is not a page, or for a tab that isn't there, is ignored", () => {
    const s = run(EMPTY, page("https://a.dev/"));
    for (const a of [ev("t1", "file:///x", "A"), ev("t1", "http://u@a.dev/", "A"), ev("t1", "", null), ev("zz", "https://b.dev/", "B"), ev("t1", "https://b.dev/", "B", "nope")]) {
      expect(paneReducer(s, a)).toBe(s);
    }
  });

  it("a browser tab's title is its page's, with no folder", () => {
    const s = run(EMPTY, page("https://a.dev/x/y"), page("https://b.dev/x/y"), open("docs/a.md"), open("api/a.md"), ev("t1", "https://a.dev/x/y", "Docs"));
    const titles = tabTitles(tabsOf(s, "s").tabs);
    expect(titles.get("t1")).toBe("Docs");
    expect(titles.get("t2")).toBe("b.dev");
    expect(titles.get("t3")).toBe("a.md — docs");
    expect(titles.get("t4")).toBe("a.md — api");
    expect(tabTitles(tabsOf(run(EMPTY, page("about:blank")), "s").tabs).get("t1")).toBe("New tab");
  });

  it("back and forward leave a browser tab alone", () => {
    const s = run(EMPTY, page("https://a.dev/"));
    expect(paneReducer(s, { type: "back", session: "s", paneTab: "t1" })).toBe(s);
    expect(paneReducer(s, { type: "forward", session: "s", paneTab: "t1" })).toBe(s);
    expect(paneReducer(s, { type: "setView", session: "s", paneTab: "t1", view: "source" })).toBe(s);
    const nav: PaneAction = { type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path: "/b" } };
    expect(paneReducer(s, nav)).toBe(s);
  });

  it("an open of a file path never selects a browser tab", () => {
    const s = run(EMPTY, page("https://a.dev/"), open("https://a.dev/"));
    expect(ids(s)).toEqual(["t1", "t2"]);
  });
});

describe("openBackground", () => {
  const page = (url: string, session = "s"): PaneAction => ({ type: "openPage", session, url });
  const bg = (target: string, o: { session?: string; kind?: "file" | "browser"; by?: string; onScreen?: boolean } = {}): PaneAction => ({
    type: "openBackground",
    session: o.session ?? "s",
    kind: o.kind ?? "file",
    target,
    by: o.by ?? "claude",
    project: "p",
    onScreen: o.onScreen ?? false,
  });
  const tab = (s: PaneView, id: string, session = "s") => tabsOf(s, session).tabs.find((t) => t.id === id)!;

  it("openBackground_adds_an_unselected_tab_and_keeps_the_pane_hidden", () => {
    const s = run(EMPTY, open("/a"), { type: "hide" }, bg("/b.md"), bg("https://a.dev/", { kind: "browser", by: "shell" }));
    expect(s.open).toBe(false);
    expect(tabsOf(s, "s").active).toBe("t1");
    expect(tab(s, "t2")).toMatchObject({ kind: "file", opened_by: "claude", unseen: true });
    expect(tab(s, "t3")).toMatchObject({ kind: "browser", opened_by: "shell", unseen: true });
    expect(s.recent[0]).toEqual({ project: "p", path: "/b.md" });
  });

  it("openBackground_in_an_empty_session_selects_it_and_still_keeps_the_pane_hidden", () => {
    const s = run(EMPTY, bg("/a.md"));
    expect(tabsOf(s, "s").active).toBe("t1");
    expect(s.open).toBe(false);
  });

  it("openBackground_marks_a_tab_already_open", () => {
    const base = run(EMPTY, open("/a"), open("/b"), page("https://a.dev/"));
    expect(tab(base, "t1").unseen).toBe(false);
    const s = run(base, bg("/a", { by: "codex" }), bg("HTTPS://A.dev", { kind: "browser", by: "codex" }));
    expect(ids(s)).toEqual(["t1", "t2", "t3"]);
    expect(tab(s, "t1")).toMatchObject({ opened_by: "codex", unseen: true });
    expect(tab(s, "t3")).toMatchObject({ opened_by: "codex", unseen: true });
    expect(tab(s, "t2").unseen).toBe(false);
    expect(s.open).toBe(base.open);
    expect(tabsOf(s, "s").active).toBe(tabsOf(base, "s").active);
  });

  it("openBackground_leaves_the_tab_on_screen_unmarked", () => {
    const base = run(EMPTY, open("/a"));
    expect(paneReducer(base, bg("/a", { onScreen: true }))).toBe(base);
    const marked = paneReducer(base, bg("/a", { onScreen: false }));
    expect(tab(marked, "t1").unseen).toBe(true);
  });

  it("openBackground_at_32_tabs_changes_nothing", () => {
    let s = EMPTY;
    for (let i = 0; i < 32; i++) s = run(s, open(`/f${i}`));
    expect(paneReducer(s, bg("/new"))).toBe(s);
    expect(paneReducer(s, bg("https://a.dev/", { kind: "browser" }))).toBe(s);
    expect(tab(paneReducer(s, bg("/f3")), "t4").unseen).toBe(true);
  });

  it("selecting_a_tab_clears_unseen_and_keeps_the_opener", () => {
    const s = run(EMPTY, open("/a"), bg("/b", { by: "codex" }), { type: "select", session: "s", paneTab: "t2" });
    expect(tab(s, "t2")).toMatchObject({ opened_by: "codex", unseen: false });
  });

  it("selecting_the_tab_on_screen_clears_its_mark", () => {
    const s = run(EMPTY, bg("/a", { by: "codex" }), { type: "select", session: "s", paneTab: "t1" });
    expect(tab(s, "t1")).toMatchObject({ opened_by: "codex", unseen: false });
  });

  it("stepping_onto_a_tab_clears_unseen", () => {
    const s = run(EMPTY, open("/a"), bg("/b"), { type: "step", session: "s", panel: "files", by: 1 });
    expect(tab(s, "t2").unseen).toBe(false);
    const back = run(EMPTY, open("/a"), bg("/b"), { type: "step", session: "s", panel: "files", by: -1 });
    expect(tab(back, "t2").unseen).toBe(false);
  });

  it("seen_clears_the_mark_on_the_active_tab_and_keeps_the_opener", () => {
    const s = run(EMPTY, bg("/a", { by: "codex" }), { type: "seen", session: "s", panel: "files" });
    expect(tab(s, "t1")).toMatchObject({ opened_by: "codex", unseen: false });
  });

  it("seen_leaves_other_tabs_marked", () => {
    const s = run(EMPTY, open("/a"), bg("/b", { by: "codex" }), { type: "seen", session: "s", panel: "files" });
    expect(tab(s, "t2")).toMatchObject({ opened_by: "codex", unseen: true });
  });

  it("seen_with_nothing_marked_returns_the_same_state", () => {
    const s = run(EMPTY, open("/a"));
    expect(paneReducer(s, { type: "seen", session: "s", panel: "files" })).toBe(s);
    expect(paneReducer(s, { type: "seen", session: "none", panel: "files" })).toBe(s);
  });

  it("opening_a_marked_tab_clears_unseen", () => {
    const s = run(EMPTY, open("/a"), bg("/b"), bg("https://a.dev/", { kind: "browser" }), open("/b"), page("https://a.dev/"));
    expect(tab(s, "t2").unseen).toBe(false);
    expect(tab(s, "t3").unseen).toBe(false);
  });

  it("closing_the_active_tab_clears_unseen_on_the_one_that_takes_over", () => {
    const s = run(EMPTY, open("/a"), bg("/b"), bg("/c"), close("t1"));
    expect(tabsOf(s, "s").active).toBe("t2");
    expect(tab(s, "t2").unseen).toBe(false);
    expect(tab(s, "t3").unseen).toBe(true);
    const last = run(EMPTY, open("/a"), bg("/b"), { type: "select", session: "s", paneTab: "t2" }, bg("/c"), close("t2"));
    expect(tabsOf(last, "s").active).toBe("t3");
    expect(tab(last, "t3").unseen).toBe(false);
  });
});

describe("a place in a PDF", () => {
  const at = (paneTab: string, page: number, top: number): PaneAction => ({ type: "place", session: "s", paneTab, place: { page, top } });
  const cur = (s: PaneView, id = "t1") => currentEntry(tabsOf(s, "s").tabs.find((t) => t.id === id)!);

  it("place records where the current entry is without pushing", () => {
    const s = run(EMPTY, open("/a.pdf"), at("t1", 3, 0.25));
    const t = tabsOf(s, "s").tabs[0]!;
    expect(t.history).toHaveLength(1);
    expect(cur(s)).toMatchObject({ page: 3, top: 2500 });
    expect(placeOf(cur(s))).toEqual({ page: 3, top: 0.25 });
  });

  it("place that changes nothing returns the same state", () => {
    const s = run(EMPTY, open("/a.pdf"), at("t1", 3, 0.25));
    expect(paneReducer(s, at("t1", 3, 0.25))).toBe(s);
    expect(paneReducer(s, at("nope", 3, 0.25))).toBe(s);
  });

  it("place leaves a browser tab alone", () => {
    const s = run(EMPTY, { type: "openPage", session: "s", url: "http://localhost:3000/" });
    expect(paneReducer(s, at("t1", 2, 0.5))).toBe(s);
  });

  it("navigate with a place pushes an entry with page and top", () => {
    const s = run(EMPTY, open("/a.pdf"), {
      type: "navigate",
      session: "s",
      paneTab: "t1",
      project: "p",
      target: { path: "/b.pdf", place: { page: 4, top: 0.5 } },
    });
    expect(tabsOf(s, "s").tabs[0]!.history).toHaveLength(2);
    expect(cur(s)).toMatchObject({ path: "/b.pdf", page: 4, top: 5000 });
  });

  it("back after a place and a navigate returns to the recorded place", () => {
    const s = run(
      EMPTY,
      open("/a.pdf"),
      at("t1", 7, 0.1),
      { type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path: "/a.pdf", place: { page: 2, top: 0 } } },
      { type: "back", session: "s", paneTab: "t1" },
    );
    expect(placeOf(cur(s))).toEqual({ page: 7, top: 0.1 });
  });

  it("placeOf of an entry without a page is null", () => {
    const s = run(EMPTY, open("/a.pdf"));
    expect(placeOf(cur(s))).toBeNull();
    expect("page" in cur(s)).toBe(false);
  });

  it("a place's top is clamped to 0..1 and its page to at least 1", () => {
    let s = run(EMPTY, open("/a.pdf"), at("t1", 0, 1.5));
    expect(cur(s)).toMatchObject({ page: 1, top: 10_000 });
    s = run(s, at("t1", -3, -0.2));
    expect(cur(s)).toMatchObject({ page: 1, top: 0 });
  });
});

describe("the Browser and Files panels", () => {
  const f = (id: string, path = `/${id}.md`): PaneTab => ({
    id,
    kind: "file",
    history: [{ path, line: null, col: null, view: "rendered" }],
    index: 0,
    title: null,
    opened_by: null,
    unseen: false,
  });
  const b = (id: string): PaneTab => ({ ...f(id, `https://${id}.test/`), kind: "browser" });
  const mk = (sessions: Record<string, SessionTabs>, extra: Partial<PaneView> = {}): PaneView => ({ open: true, width: null, sessions, recent: [], ...extra });
  const pg = (url: string, session = "s"): PaneAction => ({ type: "openPage", session, url });
  const bgPage = (url: string, session = "s"): PaneAction => ({ type: "openPage", session, url, background: true });
  const bg = (target: string, kind: "file" | "browser" = "file", session = "s"): PaneAction => ({
    type: "openBackground",
    session,
    kind,
    target,
    by: "claude",
    project: "p",
    onScreen: false,
  });
  const sel = (st: PaneView, panel: PanelId, session = "s") => activeIn(tabsOf(st, session), panel);
  const shown = (st: PaneView, session = "s") => stackOf(st, session).shown;
  const select = (paneTab: string, session = "s"): PaneAction => ({ type: "select", session, paneTab });
  // A pane saved by a build from before the panels: mixed tabs, and `active` on the Browser one.
  const OLD = (): PaneView => mk({ s: { tabs: [f("t1"), b("t2"), f("t3")], active: "t2" } });

  it("the old active tab stays its panel's selection after a select in the other panel", () => {
    const old = OLD();
    expect(sel(old, "browser")).toBe("t2");
    expect(sel(old, "files")).toBe("t1");
    const next = paneReducer(old, select("t3"));
    expect(tabsOf(next, "s")).toMatchObject({ active: "t3", active_browser: "t2" });
    expect(paneProblem(next)).toBeNull();
    expect(paneProblem(paneReducer(old, select("t1")))).toBeNull();
  });

  it("paneProblem refuses a viewport that is not a size, and takes the four sizes", () => {
    const withViewport = (viewport: string | undefined): PaneView => mk({ s: { tabs: [{ ...b("t1"), viewport }], active: "t1", active_browser: "t1" } });
    for (const ok of [undefined, "phone", "tablet", "laptop", "desktop"]) expect(paneProblem(withViewport(ok))).toBeNull();
    for (const bad of ["wide", "", "Phone"]) expect(paneProblem(withViewport(bad))).toBe("viewport");
  });

  it("sets and clears a Browser tab's viewport, and leaves a Files tab alone", () => {
    const withPage = mk({ s1: { tabs: [b("p1")], active: "p1", active_browser: "p1" } });
    const withTextFile = mk({ s1: { tabs: [f("f1")], active: "f1" } });
    const set = paneReducer(withPage, { type: "viewport", session: "s1", paneTab: "p1", viewport: "phone" });
    expect(tabsOf(set, "s1").tabs[0]!.viewport).toBe("phone");
    expect("viewport" in tabsOf(paneReducer(set, { type: "viewport", session: "s1", paneTab: "p1", viewport: null }), "s1").tabs[0]!).toBe(false);
    expect(paneReducer(withTextFile, { type: "viewport", session: "s1", paneTab: "f1", viewport: "phone" })).toBe(withTextFile);
  });

  it("the stack is the window's: another session shows the same panels", () => {
    const s = mk({ a: { tabs: [b("t1")], active: "t1" }, c: { tabs: [f("t1")], active: "t1" } });
    const withStack = paneReducer(s, { type: "pin", session: "a" });
    expect(stackOf(withStack, "c")).toEqual(stackOf(withStack, "a"));
    expect(shown(withStack, "c")).toEqual(["browser"]);
    const other = run(withStack, open("/x.md", { session: "c" }));
    expect(shown(other, "a")).toEqual(["browser", "files"]);
    expect(shown(other, "c")).toEqual(["browser", "files"]);
  });

  it("the stack's panels are the same with the pane hidden", () => {
    const s = run(EMPTY, pg("https://a.test/"), open("/a.md"));
    const before = stackPanels(stackOf(s, "s"));
    const hidden = paneReducer(s, { type: "hide" });
    expect(stackPanels(stackOf(hidden, "s"))).toEqual(before);
    expect(onScreen(hidden, "s")).toEqual([]);
    expect(onScreen(s, "s")).toEqual(["browser", "files"]);
    const one = run(s, { type: "panel", panel: "browser", session: "s" }, { type: "panel", panel: "files", session: "s" });
    expect(one.open).toBe(false);
    expect(stackPanels(stackOf(one, "s"))).toEqual(["files"]);
    expect(onScreen(one, "s")).toEqual([]);
  });

  it("a filled panel is the stack's only panel", () => {
    const s = run(EMPTY, pg("https://a.test/"), open("/a.md"), { type: "fill", panel: "files", session: "s" });
    expect(stackOf(s, "s").filled).toBe("files");
    expect(stackPanels(stackOf(s, "s"))).toEqual(["files"]);
    expect(onScreen(s, "s")).toEqual(["files"]);
    expect(stackOf(s, "s").shown).toEqual(["browser", "files"]);
  });

  it("an open selects the tab in its panel, shows the pane and adds the panel", () => {
    const s0 = run(EMPTY, pg("https://a.test/"));
    expect(shown(s0)).toEqual(["browser"]);
    expect(s0.panels).toEqual({ shown: ["browser"], weights: {}, filled: null });
    const s1 = run(s0, open("/a.md"));
    expect(shown(s1)).toEqual(["browser", "files"]);
    expect(sel(s1, "files")).toBe("t2");
    expect(sel(s1, "browser")).toBe("t1");
    expect(paneProblem(s1)).toBeNull();
    const s2 = run(s1, open("/a.html"));
    expect(sel(s2, "browser")).toBe("t3");
    expect(sel(s2, "files")).toBe("t2");
    expect(paneProblem(s2)).toBeNull();
  });

  it("an open into another panel ends a fill; into the filled panel keeps it", () => {
    const filled = run(EMPTY, pg("https://a.test/"), open("/a.md"), { type: "fill", panel: "browser", session: "s" });
    expect(stackOf(filled, "s").filled).toBe("browser");
    const files = run(filled, open("/b.md"));
    expect(stackOf(files, "s")).toMatchObject({ filled: null, shown: ["browser", "files"] });
    const same = run(filled, open("/b.html"));
    expect(stackOf(same, "s").filled).toBe("browser");
    const page = run(filled, pg("https://b.test/"));
    expect(stackOf(page, "s").filled).toBe("browser");
  });

  it("an open while the pane is hidden shows that panel alone", () => {
    const s = mk({ s: { tabs: [f("t1"), b("t2")], active: "t1", active_browser: "t2" } }, { open: false, panels: { shown: ["browser", "files"], weights: { browser: 3000 }, filled: "browser" } });
    const next = paneReducer(s, open("/new.md"));
    expect(next.open).toBe(true);
    expect(next.panels).toEqual({ shown: ["files"], weights: { browser: 3000 }, filled: null });
    const page = paneReducer(s, pg("https://z.test/"));
    expect(page.panels).toEqual({ shown: ["browser"], weights: { browser: 3000 }, filled: null });
  });

  it("an open of the tab already selected in a panel on screen returns the same state", () => {
    const s = run(EMPTY, pg("https://a.test/"), open("/a.md"));
    expect(paneReducer(s, open("/a.md"))).toBe(s);
    expect(paneReducer(s, pg("https://a.test/"))).toBe(s);
    const hidden = paneReducer(s, { type: "hide" });
    expect(paneReducer(hidden, open("/a.md"))).not.toBe(hidden);
    const off = run(s, { type: "panel", panel: "browser", session: "s" });
    expect(paneReducer(off, pg("https://a.test/"))).not.toBe(off);
    const notSelected = run(s, open("/b.md"));
    expect(paneReducer(notSelected, open("/a.md"))).not.toBe(notSelected);
  });

  it("the first page in a session with no tabs is active and active_browser", () => {
    const s = run(EMPTY, pg("https://a.test/"));
    expect(tabsOf(s, "s")).toMatchObject({ active: "t1", active_browser: "t1" });
    expect(paneProblem(s)).toBeNull();
    const file = run(EMPTY, open("/a.md"));
    expect("active_browser" in tabsOf(file, "s")).toBe(false);
    expect(paneProblem(file)).toBeNull();
    expect(paneProblem(run(EMPTY, open("/a.html")))).toBeNull();
  });

  it("the 33rd tab is refused whatever its panel", () => {
    let s = EMPTY;
    for (let i = 0; i < 16; i++) s = run(s, open(`/f${i}.md`), pg(`https://h${i}.test/`));
    expect(ids(s)).toHaveLength(32);
    expect(paneReducer(s, open("/extra.md"))).toBe(s);
    expect(paneReducer(s, open("/extra.html"))).toBe(s);
    expect(paneReducer(s, pg("https://extra.test/"))).toBe(s);
    expect(paneReducer(s, pg("about:blank"))).toBe(s);
    expect(paneReducer(s, bgPage("https://extra.test/"))).toBe(s);
    expect(paneReducer(s, bg("/extra.html"))).toBe(s);
  });

  it("a link to a file of the other panel opens a tab there", () => {
    const s = run(EMPTY, open("/a.md"));
    const link: PaneAction = { type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path: "/page.html" } };
    const next = paneReducer(s, link);
    expect(tabsOf(next, "s").tabs[0]).toBe(tabsOf(s, "s").tabs[0]);
    expect(next).toEqual(run(s, open("/page.html")));
    expect(sel(next, "browser")).toBe("t2");
    expect(shown(next)).toEqual(["browser", "files"]);
    expect(paneProblem(next)).toBeNull();
    let full = run(EMPTY, open("/a.md"));
    for (let i = 0; i < 31; i++) full = run(full, pg(`https://h${i}.test/`));
    expect(paneReducer(full, link)).toBe(full);
  });

  it("Back onto an entry of the other panel takes the tab there, selected, with its panel shown", () => {
    const moved: PaneTab = { ...f("t1", "/a.html"), history: [...f("t1", "/a.html").history, ...f("t1", "/b.md").history], index: 1 };
    const s = mk({ s: { tabs: [moved, f("t2")], active: "t1" } }, { panels: { shown: ["files"], weights: { files: 4000 }, filled: null } });
    const next = paneReducer(s, { type: "back", session: "s", paneTab: "t1" });
    expect(tabsOf(next, "s").tabs[0]!.index).toBe(0);
    expect(sel(next, "browser")).toBe("t1");
    expect(sel(next, "files")).toBe("t2");
    expect(next.panels).toEqual({ shown: ["browser", "files"], weights: { files: 4000 }, filled: null });
    expect(paneProblem(next)).toBeNull();
    const there = paneReducer(next, { type: "forward", session: "s", paneTab: "t1" });
    expect(sel(there, "files")).toBe("t1");
    expect(paneProblem(there)).toBeNull();
  });

  it("select sets its panel's selection only", () => {
    const s = mk({ s: { tabs: [f("t1"), b("t2"), f("t3"), b("t4")], active: "t1", active_browser: "t2" } });
    const files = paneReducer(s, select("t3"));
    expect(tabsOf(files, "s")).toMatchObject({ active: "t3", active_browser: "t2" });
    const browser = paneReducer(s, select("t4"));
    expect(tabsOf(browser, "s")).toMatchObject({ active: "t1", active_browser: "t4" });
    expect(paneReducer(s, select("t1"))).toBe(s);
  });

  it("step wraps inside the panel", () => {
    const s = mk({ s: { tabs: [f("t1"), b("t2"), f("t3"), b("t4")], active: "t1", active_browser: "t2" } });
    const step = (st: PaneView, panel: PanelId, by: 1 | -1) => paneReducer(st, { type: "step", session: "s", panel, by });
    const a = step(s, "files", 1);
    expect(sel(a, "files")).toBe("t3");
    expect(sel(step(a, "files", 1), "files")).toBe("t1");
    expect(sel(step(s, "browser", -1), "browser")).toBe("t4");
    expect(sel(step(s, "browser", 1), "files")).toBe("t1");
    const lone = mk({ s: { tabs: [f("t1")], active: "t1" } });
    expect(step(lone, "browser", 1)).toBe(lone);
  });

  it("move reorders inside the panel and leaves the other panel's tabs where they were", () => {
    const s = mk({ s: { tabs: [f("t1"), b("t2"), f("t3"), b("t4")], active: "t1", active_browser: "t2" } });
    const mv = (paneTab: string, to: number): PaneAction => ({ type: "move", session: "s", paneTab, to });
    expect(ids(paneReducer(s, mv("t3", 0)))).toEqual(["t3", "t2", "t1", "t4"]);
    expect(ids(paneReducer(s, mv("t2", 9)))).toEqual(["t1", "t4", "t3", "t2"]);
    expect(ids(paneReducer(s, mv("t4", 0)))).toEqual(["t1", "t4", "t3", "t2"]);
    expect(paneReducer(s, mv("t1", 0))).toBe(s);
  });

  it("closing a panel's selected tab selects its neighbour in that panel", () => {
    const s = mk({ s: { tabs: [f("t1"), b("t2"), f("t3"), f("t4"), b("t5")], active: "t3", active_browser: "t2" } });
    const a = paneReducer(s, close("t3"));
    expect(sel(a, "files")).toBe("t4");
    expect(sel(a, "browser")).toBe("t2");
    const b2 = paneReducer(s, close("t2"));
    expect(sel(b2, "browser")).toBe("t5");
    expect(sel(b2, "files")).toBe("t3");
    const last = paneReducer(a, close("t4"));
    expect(sel(last, "files")).toBe("t1");
    expect(paneProblem(last)).toBeNull();
  });

  it("closing the Files panel's last tab leaves the panel shown and active on the Browser panel's tab", () => {
    const s = run(EMPTY, open("/a.md"), pg("https://a.test/"), close("t1"));
    expect(shown(s)).toEqual(["browser", "files"]);
    expect(tabsOf(s, "s")).toMatchObject({ active: "t2", active_browser: "t2" });
    expect(paneProblem(s)).toBeNull();
  });

  it("closing the Browser panel's last tab removes active_browser", () => {
    const s = run(EMPTY, open("/a.md"), pg("https://a.test/"), close("t2"));
    expect(tabsOf(s, "s").active).toBe("t1");
    expect("active_browser" in tabsOf(s, "s")).toBe(false);
    expect(paneProblem(s)).toBeNull();
  });

  it("closing the session's last tab forgets the session", () => {
    const s = run(EMPTY, pg("https://a.test/"), close("t1"));
    expect("s" in s.sessions).toBe(false);
    expect(paneProblem(s)).toBeNull();
  });

  it("a background open never changes the stack", () => {
    const base = run(EMPTY, open("/a.md"), { type: "hide" });
    const files = paneReducer(base, bg("/b.md"));
    expect(files.open).toBe(false);
    expect(files.panels).toBe(base.panels);
    expect(sel(files, "files")).toBe("t1");
    const html = paneReducer(base, bg("/c.html"));
    expect(html.open).toBe(false);
    expect(html.panels).toBe(base.panels);
    // The Browser panel had no tab, so the tab is its selection.
    expect(sel(html, "browser")).toBe("t2");
    expect(sel(paneReducer(html, bg("https://x.test/", "browser")), "browser")).toBe("t2");
    const page = paneReducer(base, bgPage("https://x.test/"));
    expect(page.open).toBe(false);
    expect(page.panels).toBe(base.panels);
    expect(tabsOf(page, "s")).toMatchObject({ active: "t1", active_browser: "t2" });
    for (const empty of [paneReducer(EMPTY, bg("/a.md")), paneReducer(EMPTY, bg("https://x.test/", "browser")), paneReducer(EMPTY, bgPage("https://x.test/"))]) {
      expect(empty.open).toBe(false);
      expect(empty.panels).toBeUndefined();
      expect(paneProblem(empty)).toBeNull();
    }
  });

  it("seen clears the mark of that panel's selected tab only", () => {
    const marked = (t: PaneTab): PaneTab => ({ ...t, opened_by: "claude", unseen: true });
    const s = mk({ s: { tabs: [marked(f("t1")), marked(b("t2")), marked(f("t3"))], active: "t1", active_browser: "t2" } });
    const files = paneReducer(s, { type: "seen", session: "s", panel: "files" });
    expect(tabsOf(files, "s").tabs.map((t) => t.unseen)).toEqual([false, true, true]);
    const browser = paneReducer(s, { type: "seen", session: "s", panel: "browser" });
    expect(tabsOf(browser, "s").tabs.map((t) => t.unseen)).toEqual([true, false, true]);
    expect(paneReducer(files, { type: "seen", session: "s", panel: "files" })).toBe(files);
  });

  it("Ctrl+Shift+Q shows the remembered panels, and writes the stack down when none was saved", () => {
    const old = { ...OLD(), open: false };
    for (const type of ["toggle", "show"] as const) {
      const next = paneReducer(old, { type, session: "s" });
      expect(next.open).toBe(true);
      expect(next.panels).toEqual({ shown: ["browser", "files"], weights: {}, filled: null });
    }
    const none = paneReducer(old, { type: "show", session: null });
    expect(none.panels).toEqual({ shown: ["files"], weights: {}, filled: null });
    const saved = { ...old, panels: { shown: ["files"], weights: { files: 2 }, filled: null } };
    expect(paneReducer(saved, { type: "toggle", session: "s" }).panels).toBe(saved.panels);
    const open = paneReducer(old, { type: "show", session: "s" });
    expect(paneReducer(open, { type: "hide" }).panels).toBe(open.panels);
    expect(paneReducer(open, { type: "toggle", session: "s" }).panels).toBe(open.panels);
  });

  it("a panel's toggle", () => {
    const two = run(EMPTY, pg("https://a.test/"), open("/a.md"));
    const toggle = (st: PaneView, panel: PanelId) => paneReducer(st, { type: "panel", panel, session: "s" });
    // On screen with another: it leaves.
    const one = toggle(two, "browser");
    expect(one.panels).toEqual({ shown: ["files"], weights: {}, filled: null });
    expect(one.open).toBe(true);
    // The only one shown: the pane hides and the panel stays shown.
    const hidden = toggle(one, "files");
    expect(hidden.open).toBe(false);
    expect(hidden.panels?.shown).toEqual(["files"]);
    // Pane hidden: that panel alone.
    const back = toggle(hidden, "browser");
    expect(back.open).toBe(true);
    expect(back.panels).toEqual({ shown: ["browser"], weights: {}, filled: null });
    // Not on screen while the pane is open: added; a fill ends.
    expect(toggle(one, "browser").panels?.shown).toEqual(["browser", "files"]);
    const filled = run(two, { type: "fill", panel: "files", session: "s" });
    const added = toggle(filled, "browser");
    expect(added.panels).toEqual({ shown: ["browser", "files"], weights: {}, filled: null });
    // The filled one leaves; the other remains.
    const gone = toggle(filled, "files");
    expect(gone.panels).toEqual({ shown: ["browser"], weights: {}, filled: null });
    for (const st of [two, one, hidden, back, filled, added, gone]) expect(paneProblem(st)).toBeNull();
  });

  it("fill", () => {
    const two = run(EMPTY, pg("https://a.test/"), open("/a.md"));
    const fill = (st: PaneView, panel: PanelId) => paneReducer(st, { type: "fill", panel, session: "s" });
    const filled = fill(two, "browser");
    expect(filled.panels?.filled).toBe("browser");
    expect(fill(filled, "browser").panels?.filled).toBeNull();
    expect(fill(filled, "files").panels?.filled).toBe("files");
    const one = run(two, { type: "panel", panel: "browser", session: "s" });
    expect(fill(one, "files")).toBe(one);
    const hidden = paneReducer(two, { type: "hide" });
    expect(fill(hidden, "files")).toBe(hidden);
  });

  it("weights are whole, 1 to 100000; NaN, an infinite number and an unknown key are dropped", () => {
    const s = run(EMPTY, pg("https://a.test/"), open("/a.md"));
    const w = (weights: Record<string, number>) => paneReducer(s, { type: "weights", weights } as PaneAction);
    expect(w({ browser: 5000.4, files: 123456 }).panels?.weights).toEqual({ browser: 5000, files: 100000 });
    expect(w({ browser: 0.2 }).panels?.weights).toEqual({ browser: 1 });
    expect(w({ browser: Number.NaN, files: Number.POSITIVE_INFINITY, changes: 5 })).toBe(s);
    expect(w({ browser: Number.NaN, files: 7000, changes: 5 }).panels?.weights).toEqual({ files: 7000 });
    const merged = paneReducer(w({ browser: 3000 }), { type: "weights", weights: { files: 9000 } });
    expect(merged.panels?.weights).toEqual({ browser: 3000, files: 9000 });
    expect(paneProblem(merged)).toBeNull();
  });

  it("pin writes the stack once", () => {
    const s = OLD();
    const pinned = paneReducer(s, { type: "pin", session: "s" });
    expect(pinned.panels).toEqual({ shown: ["browser", "files"], weights: {}, filled: null });
    expect(paneReducer(pinned, { type: "pin", session: "s" })).toBe(pinned);
    expect(paneReducer(pinned, { type: "pin", session: "other" })).toBe(pinned);
  });

  /** A small seeded generator, so a failure repeats. */
  const mulberry32 = (seed: number) => () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };

  function walk(seed: number, start: PaneView): void {
    const rnd = mulberry32(seed);
    const int = (n: number) => Math.floor(rnd() * n);
    const pick = <T,>(list: readonly T[]): T => list[int(list.length)]!;
    const sessions = ["a", "b", "c"];
    let state = start;
    // A session the walk has not touched is as it was loaded: the core accepts an old layout that names no Browser selection.
    const loaded = new Set(Object.values(start.sessions));
    const checked = (st: PaneView): PaneView => ({ ...st, sessions: Object.fromEntries(Object.entries(st.sessions).filter(([, v]) => !loaded.has(v))) });
    for (let i = 0; i < 2000; i++) {
      const session = pick(sessions);
      const own = tabsOf(state, session).tabs;
      const tabId = () => (own.length > 0 && rnd() < 0.85 ? pick(own).id : pick(["t99", "zz"]));
      const path = () => pick([`/f${int(26)}.md`, `/f${int(26)}.html`, `C:\\p\\g${int(8)}.HTM`, `/d${int(8)}.pdf`, `/s${int(8)}.ts`]);
      const url = () => pick([`https://h${int(26)}.test/`, `http://localhost:${3000 + int(5)}/`, "about:blank", "javascript:x", "file:///x"]);
      const panel = () => pick(PANELS);
      const target = (): OpenTarget => ({ path: rnd() < 0.15 ? url() : path(), line: rnd() < 0.3 ? int(100) : undefined });
      const actions: PaneAction[] = [
        { type: "open", session, project: "p", target: target() },
        { type: "open", session, project: "p", target: target() },
        { type: "navigate", session, paneTab: tabId(), project: "p", target: target() },
        { type: "navigate", session, paneTab: tabId(), project: "p", target: target() },
        { type: "back", session, paneTab: tabId() },
        { type: "forward", session, paneTab: tabId() },
        { type: "setView", session, paneTab: tabId(), view: pick(["rendered", "source"] as const) },
        { type: "place", session, paneTab: tabId(), place: { page: int(5), top: rnd() } },
        { type: "select", session, paneTab: tabId() },
        { type: "step", session, panel: panel(), by: pick([1, -1] as const) },
        { type: "close", session, paneTab: tabId() },
        { type: "move", session, paneTab: tabId(), to: int(40) - 3 },
        { type: "openPage", session, url: url() },
        { type: "openPage", session, url: url() },
        { type: "openPage", session, url: url(), background: true },
        { type: "openBackground", session, kind: "file", target: path(), by: "claude", project: "p", onScreen: rnd() < 0.5 },
        { type: "openBackground", session, kind: "browser", target: url(), by: "shell", project: "p", onScreen: rnd() < 0.5 },
        { type: "seen", session, panel: panel() },
        { type: "page", session, paneTab: tabId(), url: url(), title: rnd() < 0.5 ? "T" : null },
        { type: "show", session: rnd() < 0.8 ? session : null },
        { type: "hide" },
        { type: "toggle", session: rnd() < 0.8 ? session : null },
        { type: "width", width: rnd() * 5000 },
        { type: "panel", panel: panel(), session: rnd() < 0.8 ? session : null },
        { type: "panel", panel: panel(), session },
        { type: "fill", panel: panel(), session: rnd() < 0.8 ? session : null },
        { type: "fill", panel: panel(), session },
        { type: "weights", weights: { [pick(["browser", "files", "changes"])]: pick([rnd() * 200000, -4, Number.NaN, Number.POSITIVE_INFINITY, 12.5]) } },
        { type: "pin", session },
      ];
      const action = pick(actions);
      state = paneReducer(state, action);
      const problem = paneProblem(checked(state));
      if (problem !== null) throw new Error(`seed ${seed}, action ${i} ${JSON.stringify(action)} broke ${problem}`);
    }
  }

  it("every state the reducer reaches is one the core accepts", { timeout: 60_000 }, () => {
    walk(1, EMPTY);
    walk(2, { ...EMPTY, open: true });
    walk(3, OLD());
    walk(4, { ...OLD(), open: false, panels: { shown: ["browser", "files"], weights: { browser: 3000 }, filled: "files" } });
    walk(5, mk({ a: { tabs: [b("t1")], active: "t1" }, b: { tabs: [f("t1", "/x.html"), f("t2")], active: "t2" } }));
  });
});

describe("a divider move with no stack saved", () => {
  it("writes the stack it merges into", () => {
    const next = paneReducer({ ...EMPTY, panels: undefined }, { type: "weights", weights: { browser: 3, files: 1 } });
    expect(next.panels?.weights).toMatchObject({ browser: 3, files: 1 });
    expect(next.panels?.shown).toEqual(stackOf({ ...EMPTY, panels: undefined }, null).shown);
  });

  it("keeps the session's panels when the move names the session", () => {
    const pane = run(EMPTY, open("/a.txt"), { type: "openPage", session: "s", url: "https://a.test/" });
    const bare: PaneView = { ...pane, panels: undefined };
    expect(stackOf(bare, "s").shown).toEqual(["browser", "files"]);
    const next = paneReducer(bare, { type: "weights", session: "s", weights: { browser: 3, files: 1 } });
    expect(next.panels?.shown).toEqual(stackOf(bare, "s").shown);
    expect(next.panels?.weights).toMatchObject({ browser: 3, files: 1 });
  });
});

describe("preview tabs and the tree", () => {
  const pv = (path: string, o: { session?: string } = {}): PaneAction => ({
    type: "open",
    session: o.session ?? "s",
    project: "p",
    target: { path },
    preview: true,
  });
  const tabOf = (s: PaneView, id: string) => tabsOf(s, "s").tabs.find((t) => t.id === id)!;
  const keep = (paneTab: string): PaneAction => ({ type: "keep", session: "s", paneTab });

  it("an open with preview adds a preview tab", () => {
    const s = run(EMPTY, pv("/a.ts"));
    expect(tabsOf(s, "s").tabs).toHaveLength(1);
    expect(tabOf(s, "t1").preview).toBe(true);
    expect(previewTab(tabsOf(s, "s"), "files")?.id).toBe("t1");
    expect(paneProblem(s)).toBeNull();
  });
  it("a second preview replaces the first in place, keeping its place in the strip", () => {
    const s = run(EMPTY, pv("/a.ts"), open("/k.ts"), pv("/b.ts"));
    const t = tabsOf(s, "s").tabs;
    expect(t.map((x) => currentEntry(x).path)).toEqual(["/b.ts", "/k.ts"]);
    expect(t[0]!.id).not.toBe("t1");
    expect(t[0]!.preview).toBe(true);
    expect(t[0]!.history).toHaveLength(1);
    expect(t[0]!.index).toBe(0);
    expect(tabsOf(s, "s").active).toBe(t[0]!.id);
    expect(paneProblem(s)).toBeNull();
  });
  it("a preview of a file already open selects that tab and keeps it as it is", () => {
    const s = run(EMPTY, open("/a.ts"), open("/b.ts"), pv("/a.ts"));
    expect(tabsOf(s, "s").tabs).toHaveLength(2);
    expect(tabOf(s, "t1").preview).toBeUndefined();
    expect(tabsOf(s, "s").active).toBe("t1");
    const s2 = run(EMPTY, pv("/a.ts"), open("/b.ts"), pv("/a.ts"));
    expect(tabsOf(s2, "s").tabs).toHaveLength(2);
    expect(tabOf(s2, "t1").preview).toBe(true);
    expect(tabsOf(s2, "s").active).toBe("t1");
  });
  it("an open without preview of the file in a preview tab keeps the tab", () => {
    const s = run(EMPTY, pv("/a.ts"), open("/a.ts"));
    expect(tabsOf(s, "s").tabs).toHaveLength(1);
    expect(tabOf(s, "t1").preview).toBeUndefined();
  });
  it("keep makes a preview tab an ordinary one", () => {
    const s = run(EMPTY, pv("/a.ts"), keep("t1"));
    expect(tabOf(s, "t1").preview).toBeUndefined();
    expect("preview" in tabOf(s, "t1")).toBe(false);
    expect(previewTab(tabsOf(s, "s"), "files")).toBeUndefined();
    expect(run(s, keep("t1"))).toBe(s);
  });
  it("a preview never replaces a kept tab, and each panel has at most one preview", () => {
    const s = run(EMPTY, open("/k.ts"), pv("/a.ts"), pv("/b.ts"));
    expect(tabsOf(s, "s").tabs).toHaveLength(2);
    expect(tabOf(s, "t1").preview).toBeUndefined();
    const both = run(EMPTY, pv("/a.html"), pv("/b.ts"));
    const t = tabsOf(both, "s").tabs;
    expect(t).toHaveLength(2);
    expect(t.every((x) => x.preview === true)).toBe(true);
    expect(previewTab(tabsOf(both, "s"), "browser")).toBeDefined();
    expect(previewTab(tabsOf(both, "s"), "files")).toBeDefined();
    expect(paneProblem(both)).toBeNull();
  });
  it("a navigation inside a preview tab keeps it a preview", () => {
    const s = run(EMPTY, pv("/a.ts"), { type: "navigate", session: "s", paneTab: "t1", project: "p", target: { path: "/b.ts" } });
    expect(tabOf(s, "t1").preview).toBe(true);
    expect(tabOf(s, "t1").history).toHaveLength(2);
  });
  it("marshell open and an agent's file never make a preview", () => {
    const bg = (target: string): PaneAction => ({ type: "openBackground", session: "s", kind: "file", target, by: "shell", project: "p", onScreen: false });
    const s = run(EMPTY, bg("/a.ts"));
    expect(tabOf(s, "t1").preview).toBeUndefined();
    const s2 = run(EMPTY, pv("/a.ts"), bg("/a.ts"));
    expect(tabsOf(s2, "s").tabs).toHaveLength(1);
    expect(tabOf(s2, "t1").preview).toBeUndefined();
  });
  it("tree sets the session's tree; treeView merges the window's", () => {
    const s = run(EMPTY, { type: "tree", session: "s", tree: { root: "/r", expanded: ["a"] } }, { type: "treeView", view: { width: 300 } }, { type: "treeView", view: { hidden: true } });
    expect(s.sessions.s!.tree).toEqual({ root: "/r", expanded: ["a"] });
    expect(s.tree).toEqual({ hidden: true, width: 300 });
    expect(paneProblem(s)).toBeNull();
  });
  it("tree keeps only relative folders and the first 500", () => {
    const many = Array.from({ length: 600 }, (_, i) => `d${i}`);
    const s = run(EMPTY, { type: "tree", session: "s", tree: { root: "/r", expanded: ["a/../b", "/x", "", "ok", ...many] } });
    const e = s.sessions.s!.tree!.expanded;
    expect(e).toHaveLength(500);
    expect(e[0]).toBe("ok");
    expect(e).not.toContain("/x");
    expect(paneProblem(s)).toBeNull();
  });
  it("treeView clamps the width to 160 to 2048", () => {
    const w = (width: number | null) => run(EMPTY, { type: "treeView", view: { width } }).tree?.width;
    expect(w(2500)).toBe(2048);
    expect(w(100)).toBe(160);
    expect(w(null)).toBeNull();
    expect(paneProblem(run(EMPTY, { type: "treeView", view: { width: 2500 } }))).toBeNull();
  });
  it("closing a session's last tab keeps its tree", () => {
    const tree = { root: "/r", expanded: ["a"] };
    const s = run(EMPTY, open("/a.ts"), { type: "tree", session: "s", tree }, close("t1"));
    expect(s.sessions.s).toEqual({ tabs: [], active: null, tree });
    expect(paneProblem(s)).toBeNull();
    expect(run(EMPTY, open("/a.ts"), close("t1")).sessions.s).toBeUndefined();
  });
  it("a session with a tree and no tabs opens its next tab normally", () => {
    const tree = { root: "/r", expanded: [] };
    const s = run(EMPTY, { type: "tree", session: "s", tree }, open("/a.ts"));
    expect(tabsOf(s, "s").tabs).toHaveLength(1);
    expect(tabsOf(s, "s").active).toBe("t1");
    expect(s.sessions.s!.tree).toEqual(tree);
    expect(paneProblem(s)).toBeNull();
  });
  it("at 32 tabs with no preview tab a preview open changes nothing", () => {
    const full = run(EMPTY, ...Array.from({ length: 32 }, (_, i) => open(`/f${i}.ts`)));
    const next = paneReducer(full, { type: "open", session: "s", project: "p", target: { path: "/new.ts" }, preview: true });
    expect(next).toBe(full);
  });
  it("a tree root over 4,096 characters is not kept", () => {
    const tree = (root: string) => run(EMPTY, { type: "tree", session: "s", tree: { root, expanded: [] } });
    expect(tabsOf(tree("a".repeat(4097)), "s").tree).toBeUndefined();
    expect(tabsOf(tree("a".repeat(4096)), "s").tree).toEqual({ root: "a".repeat(4096), expanded: [] });
  });
});
