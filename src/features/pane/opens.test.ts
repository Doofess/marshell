import { describe, expect, it } from "vitest";
import type { PaneOpen } from "../../generated/PaneOpen";
import type { PaneView } from "../../generated/PaneView";
import { dropPlan, limitNews, takeOpens } from "./opens";
import { paneReducer, tabsOf, type PaneAction } from "./paneState";
import { PANELS, panelTabs } from "./panels";

const EMPTY: PaneView = { open: false, width: null, sessions: {}, recent: [] };
const sessions = new Map([
  ["s1", { project: "p1" }],
  ["s2", { project: "p2" }],
]);
const ctx = (o: Partial<Parameters<typeof takeOpens>[3]> = {}): Parameters<typeof takeOpens>[3] => ({ sessions, shown: "s1", onScreen: [], refuses: () => false, ...o });
const req = (id: number, target: string, o: Partial<PaneOpen> = {}): PaneOpen => ({ id, tab_id: "s1", kind: "file", target, by: "claude", auto: false, ...o });
const none = new Set<number>();
const open = (path: string): PaneAction => ({ type: "open", session: "s1", project: "p1", target: { path } });
const titles = (v: PaneView, session = "s1") => tabsOf(v, session).tabs.map((t) => t.history[0]!.path);

describe("takeOpens", () => {
  it("takeOpens_applies_each_request_once", () => {
    const r = takeOpens(EMPTY, [req(1, "/a.md"), req(2, "https://a.dev/", { kind: "browser" })], none, ctx());
    expect(titles(r.view)).toEqual(["/a.md", "https://a.dev/"]);
    expect(r.done).toEqual([
      { id: 1, refused: null },
      { id: 2, refused: null },
    ]);
    expect(r.view.open).toBe(false);
    expect(r.view.recent).toEqual([{ project: "p1", path: "/a.md" }]);
    // The actions are what the window's reducer replays to get the same view.
    expect(r.actions.reduce(paneReducer, EMPTY)).toEqual(r.view);
  });

  it("takeOpens_skips_ids_already_seen", () => {
    const r = takeOpens(EMPTY, [req(1, "/a.md"), req(2, "/b.md")], new Set([1]), ctx());
    expect(titles(r.view)).toEqual(["/b.md"]);
    expect(r.done).toEqual([{ id: 2, refused: null }]);
    const again = takeOpens(EMPTY, [req(1, "/a.md")], new Set([1]), ctx());
    expect(again.view).toBe(EMPTY);
    expect(again.done).toEqual([]);
    expect(again.said).toEqual([]);
  });

  it("takeOpens_refuses_an_address_the_shell_would", () => {
    const asked: string[] = [];
    const refuses = (u: string) => (asked.push(u), u.includes("bad"));
    const r = takeOpens(EMPTY, [req(1, "https://bad.dev/", { kind: "browser" }), req(2, "http://u@a.dev/", { kind: "browser" }), req(3, "ftp://a.dev/", { kind: "browser" }), req(4, "https://ok.dev/", { kind: "browser" })], none, ctx({ refuses }));
    expect(r.done).toEqual([
      { id: 1, refused: "address" },
      { id: 2, refused: "address" },
      { id: 3, refused: "address" },
      { id: 4, refused: null },
    ]);
    expect(titles(r.view)).toEqual(["https://ok.dev/"]);
    expect(asked).toContain("https://bad.dev/");
  });

  it("takeOpens_refuses_an_address_too_long_for_the_layout_once_it_is_written_out", () => {
    // 2000 characters as the core counted them; each "é" is six once the address is written the way a tab keeps it.
    const long = `https://ok.dev/${"é".repeat(2000)}`;
    const r = takeOpens(EMPTY, [req(1, long, { kind: "browser" }), req(2, `https://ok.dev/${"a".repeat(2000)}`, { kind: "browser" })], none, ctx());
    expect(r.done).toEqual([
      { id: 1, refused: "address" },
      { id: 2, refused: null },
    ]);
    expect(titles(r.view)).toHaveLength(1);
  });

  it("takeOpens_stores_a_typed_address_the_way_the_url_bar_does", () => {
    const first = takeOpens(EMPTY, [req(1, "HTTP://EXAMPLE.COM/", { kind: "browser" })], none, ctx());
    expect(titles(first.view)).toEqual(["http://example.com/"]);
    const again = takeOpens(first.view, [req(2, "http://Example.com", { kind: "browser" })], none, ctx({ shown: "s2" }));
    expect(titles(again.view)).toEqual(["http://example.com/"]);
  });

  it("takeOpens_refuses_the_33rd_tab", () => {
    let view = EMPTY;
    for (let i = 0; i < 32; i++) view = paneReducer(view, open(`/f${i}`));
    const r = takeOpens(view, [req(1, "/new.md"), req(2, "https://a.dev/", { kind: "browser" }), req(3, "/f2")], none, ctx());
    expect(r.done).toEqual([
      { id: 1, refused: "limit" },
      { id: 2, refused: "limit" },
      { id: 3, refused: null },
    ]);
    expect(tabsOf(r.view, "s1").tabs).toHaveLength(32);
  });

  it("takeOpens_reports_a_closed_session_done", () => {
    const r = takeOpens(EMPTY, [req(1, "/a.md", { tab_id: "gone" })], none, ctx());
    expect(r.done).toEqual([{ id: 1, refused: "session" }]);
    expect(r.view).toBe(EMPTY);
    expect(r.said).toEqual([]);
    expect(r.news).toEqual([]);
  });

  it("takeOpens_says_who_opened_what", () => {
    const r = takeOpens(
      EMPTY,
      [req(1, "C:\\x\\a.md"), req(2, "https://a.dev/page", { kind: "browser", by: "codex" }), req(3, "/b.md", { by: "shell" }), req(4, "/c.md", { by: "mystery-cli" })],
      none,
      ctx(),
    );
    expect(r.said).toEqual(["Claude Code opened a.md in Files.", "Codex opened a.dev in Browser.", "Opened b.md in Files.", "mystery-cli opened c.md in Files."]);
  });

  it("takeOpens_lists_news_only_for_a_hidden_pane_or_another_session", () => {
    const one = [req(1, "/a.md")];
    const f = { session: "s1", panel: "files" };
    expect(takeOpens(EMPTY, one, none, ctx({ onScreen: [], shown: "s1" })).news).toEqual([f]);
    expect(takeOpens(EMPTY, one, none, ctx({ onScreen: PANELS, shown: "s2" })).news).toEqual([f]);
    expect(takeOpens(EMPTY, one, none, ctx({ onScreen: PANELS, shown: null })).news).toEqual([f]);
    expect(takeOpens(EMPTY, one, none, ctx({ onScreen: PANELS, shown: "s1" })).news).toEqual([]);
    const two = takeOpens(EMPTY, [req(1, "/a.md"), req(2, "/b.md"), req(3, "/c.md", { tab_id: "s2" })], none, ctx());
    expect(two.news).toEqual([f, { session: "s2", panel: "files" }]);
  });

  it("news names the panel", () => {
    const r = takeOpens(EMPTY, [req(1, "/a.html"), req(2, "/b.md"), req(3, "https://a.dev/", { kind: "browser" })], none, ctx());
    expect(r.news).toEqual([
      { session: "s1", panel: "browser" },
      { session: "s1", panel: "files" },
    ]);
  });

  it("a request for a panel in front of the person makes no news, one for the other panel does", () => {
    const r = takeOpens(EMPTY, [req(1, "/a.md"), req(2, "/b.html")], none, ctx({ onScreen: ["files"], shown: "s1" }));
    expect(r.news).toEqual([{ session: "s1", panel: "browser" }]);
  });

  it("takeOpens_leaves_the_tab_on_screen_unmarked_and_unannounced", () => {
    const view = paneReducer(EMPTY, open("/a.md"));
    const r = takeOpens(view, [req(1, "/a.md")], none, ctx({ onScreen: PANELS, shown: "s1" }));
    expect(r.view).toBe(view);
    expect(r.done).toEqual([{ id: 1, refused: null }]);
    expect(r.said).toEqual([]);
    expect(r.news).toEqual([]);
  });

  it("takeOpens_twice_in_one_frame_makes_one_tab", () => {
    const r = takeOpens(EMPTY, [req(1, "https://a.dev/", { kind: "browser" }), req(2, "https://A.dev", { kind: "browser" })], none, ctx());
    expect(tabsOf(r.view, "s1").tabs).toHaveLength(1);
    expect(r.done).toEqual([
      { id: 1, refused: null },
      { id: 2, refused: null },
    ]);
    expect(r.said).toHaveLength(1);
  });

  it("takeOpens_never_opens_the_pane_or_moves_the_selection", () => {
    const view = paneReducer(paneReducer(EMPTY, open("/a.md")), { type: "hide" });
    const r = takeOpens(view, [req(1, "/b.md")], none, ctx());
    expect(r.view.open).toBe(false);
    expect(tabsOf(r.view, "s1").active).toBe("t1");
  });
});

const full32 = (): PaneView => {
  let view = EMPTY;
  for (let i = 0; i < 32; i++) view = paneReducer(view, open(`/f${i}`));
  return view;
};

describe("auto opens", () => {
  it("an agent's own write reloads an open file in place and marks it, unless it is in front", () => {
    const view = paneReducer(EMPTY, open("/a.html"));
    const hidden = takeOpens(view, [req(1, "/a.html", { auto: true })], none, ctx({ onScreen: [], shown: "s1" }));
    expect(tabsOf(hidden.view, "s1").tabs).toHaveLength(1);
    expect(hidden.done).toEqual([{ id: 1, refused: null }]);
    expect(hidden.news).toEqual([{ session: "s1", panel: "browser" }]);
    const front = takeOpens(view, [req(1, "/a.html", { auto: true })], none, ctx({ onScreen: PANELS, shown: "s1" }));
    expect(front.view).toBe(view);
    expect(front.said).toEqual([]);
    expect(front.news).toEqual([]);
  });

  it("lists a session whose auto open met the limit, once, and answers every request", () => {
    const taken = takeOpens(full32(), [req(1, "/a.html", { auto: true }), req(2, "/b.html", { auto: true }), req(3, "/c.html")], none, ctx());
    expect(taken.full).toEqual(["s1"]);
    expect(taken.done).toEqual([
      { id: 1, refused: "limit" },
      { id: 2, refused: "limit" },
      { id: 3, refused: "limit" },
    ]);
    expect(taken.said).toEqual([]);
  });

  it("does not list a session for an open by hand that met the limit", () => {
    expect(takeOpens(full32(), [req(1, "/c.html")], none, ctx()).full).toEqual([]);
  });

  it("says the limit once per session until it has had room", () => {
    const fullView = full32();
    let roomyView = EMPTY;
    for (let i = 0; i < 31; i++) roomyView = paneReducer(roomyView, open(`/f${i}`));
    const first = limitNews(new Set(), ["s1"], fullView);
    expect(first.say).toEqual(["s1"]);
    expect(limitNews(first.told, ["s1"], fullView).say).toEqual([]);
    expect(limitNews(first.told, [], fullView).told.has("s1")).toBe(true);
    const roomy = limitNews(first.told, [], roomyView);
    expect(roomy.told.has("s1")).toBe(false);
    expect(limitNews(roomy.told, ["s1"], fullView).say).toEqual(["s1"]);
  });

  it("marks as told only the session the line is said for, so another session hears it when it is on screen", () => {
    const fullView = full32();
    const onS2 = (s: string): boolean => s === "s2";
    const first = limitNews(new Set(), ["s1"], fullView, onS2);
    expect(first.say).toEqual([]);
    expect(first.told.has("s1")).toBe(false);
    expect(limitNews(first.told, ["s1"], fullView, (s) => s === "s1").say).toEqual(["s1"]);
    expect(limitNews(new Set(), ["s1", "s2"], fullView, onS2).say).toEqual(["s2"]);
  });
});

describe("dropPlan", () => {
  it("a drop of 3 files at 31 tabs opens one and says the limit", () => {
    let view = EMPTY;
    for (let i = 0; i < 31; i++) view = paneReducer(view, open(`/f${i}`));
    const plan = dropPlan(view, "s1", "p1", [
      { path: "/a.md", ok: true },
      { path: "/b.md", ok: true },
      { path: "/c.md", ok: true },
    ]);
    expect(plan.actions).toEqual([open("/a.md")]);
    expect(plan.last).toBe("/a.md");
    expect(plan.refused).toBe(true);
    expect(tabsOf(plan.actions.reduce(paneReducer, view), "s1").tabs).toHaveLength(32);
  });

  it("a drop of an HTML file and a text file opens each in its panel and reports the last", () => {
    const plan = dropPlan(EMPTY, "s1", "p1", [
      { path: "/site/index.html", ok: true },
      { path: "/notes/a.md", ok: true },
    ]);
    expect(plan.actions).toEqual([open("/site/index.html"), open("/notes/a.md")]);
    expect(plan.last).toBe("/notes/a.md");
    expect(plan.refused).toBe(false);
    const after = plan.actions.reduce(paneReducer, EMPTY);
    expect(panelTabs(after, "s1", "browser").map((t) => t.history[0]!.path)).toEqual(["/site/index.html"]);
    expect(panelTabs(after, "s1", "files").map((t) => t.history[0]!.path)).toEqual(["/notes/a.md"]);
  });

  it("a folder alone opens nothing", () => {
    const plan = dropPlan(EMPTY, "s1", "p1", [{ path: "/folder", ok: false }]);
    expect(plan).toEqual({ actions: [], last: null, refused: false });
  });
});
