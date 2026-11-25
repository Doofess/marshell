import { afterEach, describe, expect, it } from "vitest";
import type { PaneTab } from "../../generated/PaneTab";
import type { PaneView } from "../../generated/PaneView";
import { paneReducer, tabsOf } from "../pane/paneState";
import { canLoad, pageReducer, setLoadable, subscribePages, type PageEvent, type PageState } from "./pageStore";
import { PAGE_REFUSED, SHELL_TAB_LIMIT, TAB_LIMIT } from "../pane/paneCommands";
import { openKeys, owners, pageAction, popupActions, popupNews, popupFold, popupOutcome, reloadPlan, unload } from "./useBrowserPages";
import { nextLive } from "./pages";
import { slotStep } from "./usePageSlot";

const empty: PaneView = { open: false, width: null, sessions: {}, recent: [] };
const file = (s: PaneView, session: string, path: string) => paneReducer(s, { type: "open", session, project: "p", target: { path } });
const page = (s: PaneView, session: string, url: string) => paneReducer(s, { type: "openPage", session, url });

describe("openKeys", () => {
  it("counts every browser tab of every session, and no file tab", () => {
    let s = file(empty, "a", "C:\\x\\a.md");
    s = page(s, "a", "https://example.com/");
    s = page(s, "b", "http://localhost:3000/");
    s = page(s, "b", "about:blank");
    s = file(s, "b", "C:\\x\\b.md");
    expect(openKeys(s, new Set(["a", "b"]), null)).toEqual(new Set(["a/t2", "b/t1", "b/t2"]));
  });
  it("is empty with no tabs", () => {
    expect(openKeys(empty, new Set(["a"]), null).size).toBe(0);
  });
  // Found in review: the pane keeps a closed session's tabs (a reopened session gets them back), and its pages stayed loaded
  // with no way to them: a video kept playing.
  it("leaves out the tabs of a session that is closed, so its pages unload", () => {
    const s = page(page(empty, "a", "https://example.com/"), "b", "http://localhost:3000/");
    expect(openKeys(s, new Set(["a"]), null)).toEqual(new Set(["a/t1"]));
    expect(nextLive(["b/t1", "a/t1"], "a/t1", openKeys(s, new Set(["a"]), null))).toEqual({ live: ["a/t1"], close: ["b/t1"] });
  });
  it("counts every session while the session list isn't known, so a core that is starting again unloads nothing", () => {
    const s = page(page(empty, "a", "https://example.com/"), "b", "http://localhost:3000/");
    expect(openKeys(s, null, null)).toEqual(new Set(["a/t1", "b/t1"]));
  });
});

// The shell panel's tabs are the pane's "shell" set. Their pages are keyed by the panel shell's tab id, so that a handover (the
// panel's shell becoming a session) renames the set and keeps every page.
describe("the shell's browser tabs", () => {
  const tab = (id: string, kind: "file" | "browser", path: string): PaneTab => ({ id, kind, history: [{ path, line: null, col: null, view: "rendered" }], index: 0, title: null, opened_by: null, unseen: false });
  const pane: PaneView = {
    open: true,
    width: null,
    sessions: { shell: { tabs: [tab("p1", "browser", "https://shell.example/"), tab("p2", "file", "/x.md")], active: "p1", active_browser: "p1" } },
    recent: [],
  };
  it("the shell's tabs are keyed by the panel's tab id", () => {
    const o = owners(pane, new Set(["s1"]), "t9");
    expect([...o.keys()]).toEqual(["t9/p1"]);
    expect(o.get("t9/p1")).toEqual({ session: "shell", paneTab: "p1" });
  });
  it("with no panel shell the shell's pages can't load", () => {
    const o = owners(pane, new Set(["s1"]), null);
    expect([...o.keys()].filter((k) => k.endsWith("/p1"))).toEqual([]);
  });
  it("a handover keeps every key", () => {
    const before = openKeys(pane, new Set(["s1"]), "t9");
    const after = openKeys(paneReducer(pane, { type: "handOver", from: "shell", to: "t9" }), new Set(["s1", "t9"]), null);
    expect(before.size).toBe(1);
    expect(after).toEqual(before);
  });
  it("a page report of a shell page moves the shell's tab", () => {
    const owner = owners(pane, new Set(["s1"]), "t9").get("t9/p1");
    expect(pageAction(owner, { url: "https://shell.example/next", title: "N", loading: false })).toEqual({ type: "page", session: "shell", paneTab: "p1", url: "https://shell.example/next", title: "N" });
  });
  it("a popup in a shell page opens in the shell's set", () => {
    expect(popupOutcome(pane, "shell", "https://example.com/", true, []).actions[0]).toMatchObject({ type: "openPage", session: "shell" });
  });
  it("a popup in a full shell set says the shell's limit", () => {
    let full = empty;
    for (let i = 0; i < 32; i++) full = page(full, "shell", `https://example.com/${i}`);
    expect(popupOutcome(full, "shell", "https://example.com/", true, [])).toEqual({ actions: [], say: SHELL_TAB_LIMIT });
  });
  it("reloadPlan finds a shell page", () => {
    expect(reloadPlan(pane, ["t9/p1"], "t9")).toEqual([{ key: "t9/p1", url: "https://shell.example/" }]);
  });
});

describe("reloadPlan", () => {
  it("loads every loaded page again, each at its tab's address, and counts them", () => {
    const s = page(page(empty, "s1", "https://a.example/"), "s2", "https://b.example/x");
    expect(reloadPlan(s, ["s1/t1", "s2/t1", "s3/t1"], null)).toEqual([
      { key: "s1/t1", url: "https://a.example/" },
      { key: "s2/t1", url: "https://b.example/x" },
      { key: "s3/t1", url: null },
    ]);
  });
});

describe("owners", () => {
  it("names the session and the pane tab of each key", () => {
    const s = page(page(empty, "a", "https://example.com/"), "b", "https://example.org/");
    expect(owners(s, new Set(["a", "b"]), null).get("b/t1")).toEqual({ session: "b", paneTab: "t1" });
    expect(owners(s, new Set(["a"]), null).has("b/t1")).toBe(false);
  });
});

// Found in the live re-check: a hide keeps drawing the pane's last node until its motion ends (MainWindow), so a closed session's
// shown tab was still drawn, as active as it last was, when its page was unloaded. It opened the page again (the test site got a
// second request), and nothing unloaded that one. No prop reaches a kept node, so the tab asks the store.
describe("canLoad", () => {
  afterEach(() => setLoadable(null));
  it("a tab whose session is closed opens no page, however active its last drawing was", () => {
    const s = page(page(empty, "a", "https://example.com/"), "b", "http://localhost:3000/");
    setLoadable(openKeys(s, new Set(["a"]), null));
    expect(canLoad("b/t1")).toBe(false);
    expect(slotStep(undefined, true, false, canLoad("b/t1"))).toBe("none");
    expect(canLoad("a/t1")).toBe(true);
    expect(slotStep(undefined, true, false, canLoad("a/t1"))).toBe("open");
  });
  it("tells the tabs that are drawn when the keys change, and only then", () => {
    let calls = 0;
    const stop = subscribePages(() => calls++);
    setLoadable(new Set(["a/t1"]));
    setLoadable(new Set(["a/t1"]));
    expect(calls).toBe(1);
    setLoadable(new Set());
    expect(calls).toBe(2);
    stop();
  });
  it("any page may load before the app has said which", () => {
    expect(canLoad("a/t1")).toBe(true);
  });
});

describe("popupActions", () => {
  // openPage shows the pane, so the closed pane is made by hiding it after.
  const open = page(page(empty, "a", "https://one.example/"), "a", "https://two.example/");
  const base = paneReducer(open, { type: "hide" });
  it("opens a tab in the popup's own session and keeps it selected when that session is on screen", () => {
    expect(popupActions(base, "a", "https://pop.example/", true, [])).toEqual([{ type: "openPage", session: "a", url: "https://pop.example/" }]);
  });
  it("a popup from a session that isn't on screen opens in the background", () => {
    expect(popupActions(base, "a", "https://pop.example/", false, [])).toEqual([{ type: "openPage", session: "a", url: "https://pop.example/", background: true }]);
  });
  it("a_popup_to_an_own_origin_opens_no_tab", () => {
    const own = [["localhost", 1420], ["127.0.0.1", 49152]] as const;
    for (const onScreen of [true, false]) {
      for (const url of ["http://localhost:1420/", "http://127.0.0.1:49152/", "http://tauri.localhost/", "file:///C:/x"]) expect(popupActions(base, "a", url, onScreen, own), url).toEqual([]);
    }
    expect(popupActions(base, "a", "http://localhost:3000/", true, own)).toEqual([{ type: "openPage", session: "a", url: "http://localhost:3000/" }]);
  });
  it("a background popup is the same whether the pane was open or hidden", () => {
    expect(popupActions(open, "a", "https://pop.example/", false, [])).toEqual(popupActions(base, "a", "https://pop.example/", false, []));
  });
});

describe("popupNews", () => {
  it("a popup opened in the background reports news for its session", () => {
    const base = paneReducer(page(empty, "a", "https://one.example/"), { type: "hide" });
    expect(popupNews(popupActions(base, "a", "https://pop.example/", false, []), base)).toEqual(["a"]);
    expect(popupNews(popupActions(base, "a", "https://pop.example/", true, []), base)).toEqual([]);
    expect(popupNews(popupActions(base, "a", "file:///C:/x", false, []), base)).toEqual([]);
  });
  it("reports nothing for an address its session already has open: no tab is added", () => {
    const base = paneReducer(page(empty, "a", "https://one.example/"), { type: "hide" });
    expect(popupNews(popupActions(base, "a", "https://one.example/", false, []), base)).toEqual([]);
    // The same address in another session is a new tab there.
    expect(popupNews(popupActions(base, "b", "https://one.example/", false, []), base)).toEqual(["b"]);
  });
  it("reports nothing when its session is at the tab limit: no tab is added", () => {
    let full = empty;
    for (let i = 0; i < 32; i++) full = page(full, "a", `https://example.com/${i}`);
    full = paneReducer(full, { type: "hide" });
    expect(full.sessions.a?.tabs.length).toBe(32);
    expect(popupNews(popupActions(full, "a", "https://pop.example/", false, []), full)).toEqual([]);
  });
});

describe("popupOutcome", () => {
  it("says the limit line for a popup in a session with 32 tabs", () => {
    let full = empty;
    for (let i = 0; i < 32; i++) full = page(full, "s1", `https://example.com/${i}`);
    expect(popupOutcome(full, "s1", "https://example.com/", true, [])).toEqual({ actions: [], say: TAB_LIMIT });
  });
  it("keeps the actions for an address already open in a session with 32 tabs", () => {
    let full = empty;
    for (let i = 0; i < 32; i++) full = page(full, "s1", `https://example.com/${i}`);
    const r = popupOutcome(full, "s1", "https://example.com/5", true, []);
    expect(r.say).toBeNull();
    expect(r.actions).toEqual([{ type: "openPage", session: "s1", url: "https://example.com/5" }]);
  });
  it("says the refusal for an address the shell would not open", () => {
    expect(popupOutcome(page(empty, "s1", "https://one.example/"), "s1", "file:///c:/x.html", true, []).say).toBe(PAGE_REFUSED);
  });
  it("says nothing for a popup that opens", () => {
    const r = popupOutcome(page(empty, "s1", "https://one.example/"), "s1", "https://pop.example/", true, []);
    expect(r.say).toBeNull();
    expect(r.actions).toHaveLength(1);
  });
});

describe("pageAction", () => {
  const owner = { session: "a", paneTab: "t1" };
  it("gives the tab its page's address and title, and no title for an empty one", () => {
    expect(pageAction(owner, { url: "https://a.test/", title: "A", loading: true })).toEqual({ type: "page", session: "a", paneTab: "t1", url: "https://a.test/", title: "A" });
    expect(pageAction(owner, { url: "https://a.test/", title: "", loading: false })).toMatchObject({ title: null });
  });
  // Found in review: a page reports about:blank before its first address has loaded, and that replaced a restored tab's saved
  // address and title, for good when the app closed before the page got there.
  it("says nothing for a blank page that is still loading, so the tab keeps its saved address", () => {
    expect(pageAction(owner, { url: "about:blank", title: "", loading: true })).toBeNull();
  });
  it("a blank page that has loaded is the tab's address", () => {
    expect(pageAction(owner, { url: "about:blank", title: "", loading: false })).toMatchObject({ type: "page", url: "about:blank", title: null });
  });
  it("says nothing for a page no tab holds", () => {
    expect(pageAction(undefined, { url: "https://a.test/", title: "A", loading: false })).toBeNull();
  });
});

// The ninth page unloads the one shown longest ago. Its tab must open it again when shown (found in the 6q-3a live check: the
// store kept the closed page, so the tab asked the shell to show a page that no longer existed, and stayed blank).
describe("unload", () => {
  const loaded = (key: string): ReadonlyMap<string, PageState> =>
    pageReducer(pageReducer(new Map(), { type: "opening", key }), { type: "state", key, url: "http://localhost:3000/", title: "T", canBack: false, canForward: false, loading: false });

  it("closes each page in the shell and forgets it, so its tab opens it again when shown", async () => {
    let pages = loaded("a/t1");
    const closed: string[] = [];
    expect(slotStep(pages.get("a/t1"), true, false)).toBe("show");
    await unload(
      ["a/t1"],
      async (key) => void closed.push(key),
      (e: PageEvent) => (pages = pageReducer(pages, e)),
    );
    expect(closed).toEqual(["a/t1"]);
    expect(slotStep(pages.get("a/t1"), true, false)).toBe("open");
  });

  it("forgets a page whose close failed: the shell has no such page either way", async () => {
    let pages = loaded("a/t1");
    await unload(
      ["a/t1"],
      () => Promise.reject(new Error("The page isn't open.")),
      (e: PageEvent) => (pages = pageReducer(pages, e)),
    );
    expect(pages.has("a/t1")).toBe(false);
  });
});

describe("popupFold", () => {
  it("a second popup in the same tick sees the first one's tab", () => {
    const pane = page(empty, "s1", "https://one.example/");
    const first = popupOutcome(pane, "s1", "https://a.example/", false, []).actions;
    const folded = popupFold(pane, first);
    expect(popupNews(popupOutcome(folded, "s1", "https://b.example/", false, []).actions, folded)).toEqual(["s1"]);
    expect(tabsOf(folded, "s1").tabs.length).toBe(tabsOf(pane, "s1").tabs.length + 1);
  });
});
