import { describe, expect, it } from "vitest";
import type { PaneOpen } from "../../generated/PaneOpen";
import type { PaneView } from "../../generated/PaneView";
import { takeOpens } from "./opens";
import { paneReducer, tabsOf, type PaneAction } from "./paneState";
import { PANELS } from "./panels";

const EMPTY: PaneView = { open: false, width: null, sessions: {}, recent: [] };
const sessions = new Map([
  ["s1", { project: "p1" }],
  ["s2", { project: "p2" }],
]);
const ctx = (o: Partial<Parameters<typeof takeOpens>[3]> = {}): Parameters<typeof takeOpens>[3] => ({ sessions, shown: "s1", onScreen: [], refuses: () => false, ...o });
const req = (id: number, target: string, o: Partial<PaneOpen> = {}): PaneOpen => ({ id, tab_id: "s1", kind: "file", target, by: "claude", ...o });
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
