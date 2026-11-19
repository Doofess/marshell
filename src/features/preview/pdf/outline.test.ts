import type { PDFDocumentProxy } from "pdfjs-dist";
import { describe, expect, it } from "vitest";
import { ancestorsOf, loadOutline, MAX_ENTRIES, pickSays, sectionAt, treeKey, visibleRows, type OutlineNode, type Row } from "./outline";

type Raw = { title?: unknown; dest?: unknown; url?: unknown; unsafeUrl?: unknown; action?: unknown; count?: number; items?: Raw[] };

/** A document with `pages` pages of 100 by 200 whose destinations are `[pageIndex, { name: "XYZ" }, 0, 100, null]`, and this outline. */
function fakeDoc(outline: unknown, pages = 20, extra: Partial<{ getDestination: (n: string) => Promise<unknown>; log: { live: number; peak: number } }> = {}): PDFDocumentProxy {
  const log = extra.log;
  return {
    numPages: pages,
    getOutline: async () => outline,
    getDestination: extra.getDestination ?? (async () => null),
    getPageIndex: async () => 0,
    getPage: async () => {
      if (log) {
        log.live++;
        log.peak = Math.max(log.peak, log.live);
        await Promise.resolve();
        await Promise.resolve();
        log.live--;
      }
      return { getViewport: () => ({ width: 100, height: 200, convertToViewportPoint: (_x: number, y: number) => [0, 200 - y] }) };
    },
  } as unknown as PDFDocumentProxy;
}
const at = (page: number) => [page - 1, { name: "XYZ" }, 0, 100, null];

describe("loadOutline", () => {
  it("loadOutline_nests_entries_with_index_path_ids_and_pages", async () => {
    const doc = fakeDoc([
      { title: "One", dest: at(1), count: 1, items: [{ title: "One.one", dest: at(3) }, { title: "One.two", dest: at(4), items: [{ title: "Deep", dest: at(5) }] }] },
      { title: "Two", dest: at(12), count: -2 },
    ]);
    const out = await loadOutline(doc);
    expect(out.map((n) => [n.id, n.title, n.page, n.open])).toEqual([
      ["0", "One", 1, true],
      ["1", "Two", 12, false],
    ]);
    expect(out[0]!.children.map((n) => n.id)).toEqual(["0.0", "0.1"]);
    expect(out[0]!.children[1]!.children[0]).toMatchObject({ id: "0.1.0", title: "Deep", page: 5, goal: { kind: "dest" } });
  });

  it("loadOutline_trims_titles_drops_control_characters_and_reads_an_empty_one_as_untitled", async () => {
    const out = await loadOutline(fakeDoc([{ title: "  Intro\u0000\u0007 \n", dest: at(1) }, { title: " \t\u0001 ", dest: at(1) }, { title: undefined, dest: at(1) }, { title: 7, dest: at(1) }, { title: "A\tB", dest: at(1) }]));
    expect(out.map((n) => n.title)).toEqual(["Intro", "Untitled", "Untitled", "Untitled", "AB"]);
  });

  it("loadOutline_caps_the_walk_at_ten_thousand_entries_in_document_order", async () => {
    const items: Raw[] = Array.from({ length: 6000 }, (_, i) => ({ title: `a${i}`, items: [{ title: `b${i}` }] }));
    const out = await loadOutline(fakeDoc(items));
    let n = 0;
    const count = (list: OutlineNode[]) => list.forEach((x) => (n++, count(x.children)));
    count(out);
    expect(MAX_ENTRIES).toBe(10_000);
    expect(n).toBe(10_000);
    // The first 5000 pairs are whole; the cut falls between entries in document order.
    expect(out[4999]!.children.map((c) => c.title)).toEqual(["b4999"]);
    expect(out.length).toBe(5000);
  });

  it("loadOutline_walks_a_very_deep_outline_without_overflowing_the_stack", async () => {
    let item: Raw = { title: "leaf" };
    for (let i = 0; i < 9000; i++) item = { title: `n${i}`, items: [item] };
    const out = await loadOutline(fakeDoc([item]));
    expect(out).toHaveLength(1);
    expect(out[0]!.title).toBe("n8999");
  });

  it("loadOutline_keeps_an_entry_whose_destination_is_missing_without_a_page", async () => {
    const doc = fakeDoc([{ title: "Gone", dest: "no-such-name" }, { title: "Bad", dest: [] }, { title: "Far", dest: [999, { name: "Fit" }] }]);
    const out = await loadOutline(doc);
    expect(out.map((n) => [n.title, n.page, n.goal?.kind])).toEqual([
      ["Gone", null, "dest"],
      ["Bad", null, "dest"],
      ["Far", null, "dest"],
    ]);
  });

  it("loadOutline_gives_a_url_entry_the_link_goal_and_no_page", async () => {
    const out = await loadOutline(fakeDoc([{ title: "Site", url: "https://example.com/a" }, { title: "Mail", url: "mailto:a@b.c" }, { title: "Nothing" }]));
    expect(out[0]).toMatchObject({ goal: { kind: "web", url: "https://example.com/a" }, page: null });
    expect(out[1]!.goal).toEqual({ kind: "other" });
    expect(out[2]!.goal).toBeNull();
  });

  // pdf.js gives an entry a named action, or an address it will not open as `unsafeUrl`, instead of a destination.
  it("loadOutline_reads_an_entry_action_and_an_unsafe_address", async () => {
    const out = await loadOutline(fakeDoc([{ title: "Last", action: "LastPage" }, { title: "Script", unsafeUrl: "javascript:alert(1)" }, { title: "Nothing" }]));
    expect(out.map((n) => n.goal)).toEqual([{ kind: "action", action: "LastPage" }, { kind: "other" }, null]);
  });

  it("pickSays_an_entry_with_no_destination_leads_nowhere", async () => {
    const [none, some] = await loadOutline(fakeDoc([{ title: "Nothing" }, { title: "Last", action: "LastPage" }]));
    expect(pickSays(none!)).toBe("This link leads nowhere in the file.");
    expect(pickSays(some!)).toBeNull();
  });

  it("loadOutline_reads_a_failure_or_no_outline_as_empty", async () => {
    const failing = { getOutline: async () => Promise.reject(new Error("x")) } as unknown as PDFDocumentProxy;
    expect(await loadOutline(failing)).toEqual([]);
    expect(await loadOutline(fakeDoc(null))).toEqual([]);
  });

  it("loadOutline_resolves_pages_a_few_at_a_time", async () => {
    const log = { live: 0, peak: 0 };
    const items: Raw[] = Array.from({ length: 200 }, (_, i) => ({ title: `t${i}`, dest: at((i % 20) + 1) }));
    const out = await loadOutline(fakeDoc(items, 20, { log }));
    expect(out).toHaveLength(200);
    expect(out[199]!.page).toBe(20);
    expect(log.peak).toBeGreaterThan(0);
    expect(log.peak).toBeLessThanOrEqual(8);
  });
});

const node = (id: string, page: number | null, children: OutlineNode[] = []): OutlineNode => ({ id, title: `t${id}`, goal: null, page, open: false, children });

describe("sectionAt", () => {
  const tree = [node("0", 1, [node("0.0", 3), node("0.1", 6)]), node("1", 10), node("2", null), node("3", 20)];
  it("sectionAt_is_null_before_the_first_entry", () => {
    expect(sectionAt([node("0", 4)], 3)).toBeNull();
    expect(sectionAt([], 3)).toBeNull();
  });
  it("sectionAt_takes_the_last_entry_in_order_that_starts_at_or_before_the_page", () => {
    expect(sectionAt(tree, 1)).toBe("0");
    expect(sectionAt(tree, 5)).toBe("0.0");
    expect(sectionAt(tree, 6)).toBe("0.1");
    expect(sectionAt(tree, 12)).toBe("1");
    expect(sectionAt(tree, 99)).toBe("3");
  });
  it("sectionAt_goes_into_nested_entries_and_skips_those_with_no_page", () => {
    expect(sectionAt([node("0", null, [node("0.0", 2)])], 5)).toBe("0.0");
    expect(sectionAt([node("0", null)], 5)).toBeNull();
  });
});

describe("visibleRows", () => {
  const tree = [node("0", 1, [node("0.0", 2, [node("0.0.0", 3)]), node("0.1", 4)]), node("1", 5)];
  it("visibleRows_shows_the_top_level_when_nothing_is_open", () => {
    const rows = visibleRows(tree, new Set());
    expect(rows.map((r) => [r.id, r.level, r.hasChildren, r.expanded, r.parent, r.posinset, r.setsize])).toEqual([
      ["0", 1, true, false, null, 1, 2],
      ["1", 1, false, false, null, 2, 2],
    ]);
  });
  it("visibleRows_shows_the_children_of_open_entries_in_document_order", () => {
    const rows = visibleRows(tree, new Set(["0", "0.0"]));
    expect(rows.map((r) => r.id)).toEqual(["0", "0.0", "0.0.0", "0.1", "1"]);
    expect(rows[2]).toMatchObject({ level: 3, parent: "0.0", posinset: 1, setsize: 1 });
    expect(rows[1]).toMatchObject({ level: 2, parent: "0", expanded: true, posinset: 1, setsize: 2 });
  });
  it("visibleRows_hides_the_children_of_an_entry_whose_parent_is_closed", () => {
    expect(visibleRows(tree, new Set(["0.0"])).map((r) => r.id)).toEqual(["0", "1"]);
  });
  it("visibleRows_ignores_an_open_mark_on_a_leaf", () => {
    expect(visibleRows(tree, new Set(["1"])).find((r) => r.id === "1")!.expanded).toBe(false);
  });
});

describe("ancestorsOf", () => {
  it("ancestorsOf_lists_the_ids_above_an_entry_outermost_first", () => {
    expect(ancestorsOf("0")).toEqual([]);
    expect(ancestorsOf("3.12")).toEqual(["3"]);
    expect(ancestorsOf("0.2.1")).toEqual(["0", "0.2"]);
  });
});

describe("treeKey", () => {
  const row = (id: string, level: number, parent: string | null, hasChildren: boolean, expanded: boolean): Row => ({ id, level, title: id, parent, hasChildren, expanded, posinset: 1, setsize: 1 });
  const rows = [row("0", 1, null, true, true), row("0.0", 2, "0", false, false), row("0.1", 2, "0", true, false), row("1", 1, null, false, false)];
  it("treeKey_moves_down_and_up_and_stops_at_the_ends", () => {
    expect(treeKey(rows, "0", "ArrowDown")).toEqual({ focus: "0.0" });
    expect(treeKey(rows, "0.1", "ArrowUp")).toEqual({ focus: "0.0" });
    expect(treeKey(rows, "1", "ArrowDown")).toBeNull();
    expect(treeKey(rows, "0", "ArrowUp")).toBeNull();
  });
  it("treeKey_goes_to_the_first_and_last_rows", () => {
    expect(treeKey(rows, "0.1", "Home")).toEqual({ focus: "0" });
    expect(treeKey(rows, "0.1", "End")).toEqual({ focus: "1" });
    expect(treeKey(rows, "0", "Home")).toBeNull();
    expect(treeKey(rows, "1", "End")).toBeNull();
  });
  it("treeKey_opens_a_closed_entry_and_goes_to_the_first_child_of_an_open_one", () => {
    expect(treeKey(rows, "0.1", "ArrowRight")).toEqual({ expand: "0.1" });
    expect(treeKey(rows, "0", "ArrowRight")).toEqual({ focus: "0.0" });
  });
  it("treeKey_does_nothing_on_Right_at_a_leaf", () => {
    expect(treeKey(rows, "0.0", "ArrowRight")).toBeNull();
    expect(treeKey(rows, "1", "ArrowRight")).toBeNull();
  });
  it("treeKey_closes_an_open_entry_and_otherwise_goes_to_the_parent", () => {
    expect(treeKey(rows, "0", "ArrowLeft")).toEqual({ collapse: "0" });
    expect(treeKey(rows, "0.0", "ArrowLeft")).toEqual({ focus: "0" });
    expect(treeKey(rows, "0.1", "ArrowLeft")).toEqual({ focus: "0" });
  });
  it("treeKey_does_nothing_on_Left_at_a_top_level_leaf_or_closed_entry", () => {
    expect(treeKey(rows, "1", "ArrowLeft")).toBeNull();
    expect(treeKey([row("0", 1, null, true, false)], "0", "ArrowLeft")).toBeNull();
  });
  it("treeKey_activates_on_Enter_and_ignores_other_keys_and_unknown_rows", () => {
    expect(treeKey(rows, "0.1", "Enter")).toEqual({ activate: "0.1" });
    expect(treeKey(rows, "0.1", "a")).toBeNull();
    expect(treeKey(rows, "nope", "ArrowDown")).toBeNull();
  });
});
