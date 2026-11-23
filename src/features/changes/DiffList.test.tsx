import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ChangesReply } from "../../generated/ChangesReply";
import { ChangesEmpty, emptyNotices, noticesOf } from "./ChangesPanel";
import { DiffList, chunksOf, keyFocuses, listKey, refocusOf } from "./DiffList";
import { copy } from "./copy";
import { itemsOf, type Item, type Row, type Section } from "./model";

const none = { shift: false, alt: false };
const rows = (kinds: Row["kind"][]): Row[] => kinds.map((kind, i) => ({ kind, text: kind === "hunk" ? "@@ -1 +1 @@" : `line ${i}`, ...(kind === "del" ? { oldNo: i + 1 } : kind === "hunk" ? {} : { newNo: i + 42 }) }));
const file = (path: string, kinds: Row["kind"][], folded = false): Section => ({ path, status: "modified", added: 1, removed: 0, rows: rows(kinds), folded });
const A = file("a.ts", ["hunk", "add", "add", "context", "hunk", "add"]);
const B = file("b.ts", ["hunk", "del", "add"]);
// 0 file a, 1 hunk, 2 add, 3 add, 4 context, 5 hunk, 6 add, 7 file b, 8 hunk, 9 del, 10 add
const items: Item[] = itemsOf([A, B], [], new Map());

const draw = (list: Item[], o: { focus?: number; range?: { from: number; to: number } | null; wrap?: boolean } = {}) =>
  renderToStaticMarkup(<DiffList items={list} focus={o.focus ?? 0} range={o.range ?? null} wrap={o.wrap ?? false} onKey={() => {}} onClickItem={() => {}} onOpenItem={() => {}} onMenuItem={() => {}} />);

describe("DiffList", () => {
  it("is one tree with one tab stop", () => {
    const html = draw(items, { focus: 3 });
    expect(html).toContain('role="tree"');
    expect(html).toContain('aria-label="Changes"');
    expect(html).toContain('aria-multiselectable="true"');
    expect(html.match(/tabindex="0"/g)).toHaveLength(1);
    expect(html.match(/data-changes-item/g)).toHaveLength(items.length);
    expect(html).toContain('aria-level="1"');
    expect(html).toContain('aria-level="2"');
  });
  it("labels an added row", () => {
    expect(draw(items)).toContain('aria-label="Added, line 43: line 1"');
  });
  it("Alt+Down jumps to the next file", () => {
    expect(listKey(items, 3, "ArrowDown", { shift: false, alt: true }, null).focus).toBe(7);
    expect(listKey(items, 3, "ArrowUp", { shift: false, alt: true }, null).focus).toBe(0);
    expect(listKey(items, 9, "ArrowDown", { shift: false, alt: true }, null).focus).toBe(9);
  });
  it("Shift+Down stops at the hunk's end", () => {
    const first = listKey(items, 2, "ArrowDown", { shift: true, alt: false }, null);
    expect(first).toMatchObject({ focus: 3, range: { from: 2, to: 3 } });
    const second = listKey(items, 3, "ArrowDown", { shift: true, alt: false }, first.range ?? null);
    expect(second).toMatchObject({ focus: 4, range: { from: 2, to: 4 } });
    const third = listKey(items, 4, "ArrowDown", { shift: true, alt: false }, second.range ?? null);
    expect(third.focus).toBe(4);
    expect(third.range).toBeUndefined();
  });
  it("Shift+Down never takes in a hunk row", () => {
    expect(listKey(items, 1, "ArrowDown", { shift: true, alt: false }, null).range).toBeUndefined();
    expect(listKey(items, 4, "ArrowDown", { shift: true, alt: false }, null).focus).toBe(4);
    expect(listKey(items, 6, "ArrowUp", { shift: true, alt: false }, null).focus).toBe(6);
  });
  it("Shift+Up back to the first row drops the range", () => {
    expect(listKey(items, 3, "ArrowUp", { shift: true, alt: false }, { from: 2, to: 3 })).toMatchObject({ focus: 2, range: null });
  });
  it("Left on a row goes to its file, Right on a folded file unfolds it", () => {
    expect(listKey(items, 4, "ArrowLeft", none, null).focus).toBe(0);
    expect(listKey(items, 10, "ArrowLeft", none, null).focus).toBe(7);
    expect(listKey(items, 0, "ArrowLeft", none, null).fold).toBe("a.ts");
    const folded = itemsOf([file("a.ts", ["hunk", "add"], true)], [], new Map());
    expect(listKey(folded, 0, "ArrowRight", none, null).unfold).toBe("a.ts");
    expect(listKey(folded, 1, "Enter", none, null).unfold).toBe("a.ts");
    expect(listKey(folded, 0, "Enter", none, null).unfold).toBe("a.ts");
    expect(listKey(items, 0, "Enter", none, null).fold).toBe("a.ts");
  });
  it("Home and End reach the first and last item", () => {
    expect(listKey(items, 5, "Home", none, null).focus).toBe(0);
    expect(listKey(items, 5, "End", none, null).focus).toBe(items.length - 1);
  });
  it("Up and Down move one item", () => {
    expect(listKey(items, 5, "ArrowDown", none, null)).toMatchObject({ focus: 6, range: null });
    expect(listKey(items, 0, "ArrowUp", none, null).focus).toBe(0);
  });
  it("Enter on a row opens the comment box; Shift+F10 and Menu open the menu", () => {
    expect(listKey(items, 2, "Enter", none, null).open).toBe("comment");
    expect(listKey(items, 2, "F10", { shift: true, alt: false }, null).open).toBe("menu");
    expect(listKey(items, 2, "ContextMenu", none, null).open).toBe("menu");
  });
  it("Delete removes a comment and does nothing on a row", () => {
    const a = file("a.ts", ["hunk", "add"]);
    const c = { id: "c", path: "a.ts", side: "new" as const, start: 43, end: 43, rows: 1, first: "f", hash: "h", quote: [], text: "t" };
    const list = itemsOf([a], [c], new Map([["c", { at: 1, changed: false }]]));
    expect(list[3]!.kind).toBe("comment");
    expect(listKey(list, 3, "Delete", none, null).remove).toBe(true);
    expect(listKey(list, 2, "Delete", none, null).remove).toBeUndefined();
    expect(listKey(list, 3, "Enter", none, null).open).toBe("comment");
  });
  it("Esc leaves the list", () => expect(listKey(items, 3, "Escape", none, null).escape).toBe(true));
  it("a folded file shows its Show row", () => {
    const html = draw(itemsOf([file("a.ts", ["hunk", "add", "add"], true)], [], new Map()));
    expect(html).toContain("Show 3 lines");
    expect(html).toContain('aria-expanded="false"');
    expect(html).not.toContain("line 1");
  });
  it("draws the file header with its status and counts", () => {
    const html = draw(items);
    expect(html).toContain("a.ts");
    expect(html).toContain(">M<");
    expect(html).toContain("+1");
    expect(html).toContain("\u22120");
  });
  it("always draws the sign column", () => {
    const html = draw(items);
    expect(html).toContain('<span class="changes__sign" aria-hidden="true">+</span>');
    expect(html).toContain('<span class="changes__sign" aria-hidden="true">\u2212</span>');
    expect(html).toContain('<span class="changes__sign" aria-hidden="true"> </span>');
  });
  it("draws the range with SelectedMark", () => {
    const html = draw(items, { focus: 3, range: { from: 2, to: 3 } });
    const selected = html.match(/<div[^>]*aria-selected="true"[^>]*>/g) ?? [];
    expect(selected).toHaveLength(2);
    for (const el of selected) expect(el).toContain("is-selected-mark");
    expect(selected.join("")).toContain('data-changes-index="2"');
    expect(selected.join("")).toContain('data-changes-index="3"');
  });
  it("wraps long lines only when asked", () => {
    expect(draw(items, { wrap: true })).toContain('data-wrap="true"');
    expect(draw(items)).not.toContain('data-wrap="true"');
  });
});

describe("DiffList blocks", () => {
  // A redraw keyed by the items' places would rebuild every block below a file that grew, and drop the keyboard.
  it("a file's blocks keep their keys when rows are added to a file above it", () => {
    const big = file("c.ts", ["hunk", ...Array<Row["kind"]>(450).fill("add")]);
    const grown = file("a.ts", ["hunk", "add", "add", "add", "context", "hunk", "add", "add"]);
    const before = chunksOf(itemsOf([A, big], [], new Map()));
    const after = chunksOf(itemsOf([grown, big], [], new Map()));
    expect(after[1]!.chunks.map((c) => c.key)).toEqual(before[1]!.chunks.map((c) => c.key));
    expect(after[1]!.chunks.map((c) => c.indices.length)).toEqual([200, 200, 52]);
    expect(after[1]!.chunks[0]!.indices[0]).toBe(9);
  });
});

describe("keyFocuses", () => {
  it("puts the keyboard on the item a key moved to, soon after the key", () => {
    expect(keyFocuses(50, "key", false)).toBe(true);
    expect(keyFocuses(200, "key", false)).toBe(false);
    expect(keyFocuses(200, "click", false)).toBe(false);
  });
  it("leaves the keyboard in the comment box the same key opened", () => {
    // Enter on a row opens a box; a draft kept above that row moves the row's index in the same draw.
    expect(keyFocuses(50, "key", true)).toBe(false);
  });
  it("takes the keyboard out of the comment box for a shift-click on a row", () => {
    // The click itself moves no focus (its mousedown is cancelled), so the range it widened would be left without the keyboard.
    expect(keyFocuses(50, "click", true)).toBe(true);
  });
});

describe("refocusOf", () => {
  const body = {} as Element;
  const other = {} as Element;
  it("puts the keyboard back where it was when a redraw dropped it on the page", () => {
    expect(refocusOf("item", body, body)).toBe("item");
    expect(refocusOf("box", body, body)).toBe("box");
    expect(refocusOf("box", null, body)).toBe("box");
  });
  it("leaves the focus alone when it was elsewhere, or is somewhere now", () => {
    expect(refocusOf(null, body, body)).toBeNull();
    expect(refocusOf("item", other, body)).toBeNull();
  });
});

const reply = (o: Partial<ChangesReply>): ChangesReply => ({ kind: "listed", stamp: "s", folder: "C:\\repo", moved: false, branch: "main", detached: null, base: "origin/main", unrelated: false, ahead: 0, uncommitted: false, files: [], diff: "", truncated: false, more_new: 0, new_cut: false, reason: null, head: null, pr_base_unfetched: null, ...o });
const empty = (r: ChangesReply | null, noFolder = false) => renderToStaticMarkup(<ChangesEmpty reply={r} noFolder={noFolder} onRetry={() => {}} />);

describe("ChangesEmpty", () => {
  it("each empty state shows its copy", () => {
    const repo = copy.emptyRepo("C:\\repo");
    expect(empty(reply({ kind: "no_repo" }))).toContain(repo.title);
    expect(empty(reply({ kind: "no_repo" }))).toContain("C:\\repo isn&#x27;t in a git checkout.");
    expect(empty(reply({ kind: "no_repo" }))).not.toContain("<button");
    expect(empty(reply({}))).toContain("No changes");
    expect(empty(reply({}))).toContain("Nothing differs from origin/main.");
    const failed = empty(reply({ kind: "failed", reason: "git timed out" }));
    expect(failed).toContain("Can&#x27;t read the changes");
    expect(failed).toContain("git timed out");
    expect(failed).toContain("Try again");
    const refused = empty(reply({ kind: "refused", reason: "C:\\x is on a network share." }));
    expect(refused).toContain("Can&#x27;t read the changes");
    expect(refused).toContain("network share");
    expect(refused).not.toContain("<button");
    const noFolder = empty(null, true);
    expect(noFolder).toContain("This session has no folder the tree can show.");
    expect(noFolder).not.toContain("<button");
    expect(noFolder).toContain('data-icon="panel-changes"');
  });
  it("is blank before the first reply, with the mark focusPane waits on", () => {
    expect(empty(null)).toBe('<div data-changes-blank=""></div>');
  });
});

describe("emptyNotices", () => {
  const c = { id: "1", path: "a.ts", side: "new" as const, start: 1, end: 1, rows: 1, first: "f", hash: "h", quote: ["x"], text: "t" };
  it("keeps the comments on another checkout in sight beside an empty diff, so they can still be sent or discarded", () => {
    const drafts = { checkout: "C:\\other", base: "main", comments: [c], picked: [], sent: [], folds: {} };
    expect(emptyNotices(reply({ folder: "r", base: null }), drafts)).toEqual([copy.noBase, "Send or discard the 1 comment on other first."]);
  });
  it("is a bare empty state with no comments elsewhere, or no list", () => {
    expect(emptyNotices(reply({ folder: "r" }), undefined)).toBeNull();
    expect(emptyNotices(reply({ folder: "r" }), { checkout: "r", base: "main", comments: [c], picked: [], sent: [], folds: {} })).toBeNull();
    expect(emptyNotices(null, { checkout: "C:\\other", base: "main", comments: [c], picked: [], sent: [], folds: {} })).toBeNull();
  });
});

describe("noticesOf", () => {
  const f = (path: string) => ({ path, old_path: null, status: "modified" as const, note: null });
  const part = (path: string) => `diff --git a/${path} b/${path}\nindex 1..2 100644\n--- a/${path}\n+++ b/${path}\n@@ -1 +1 @@\n-a\n+b\n`;
  it("says how many files a cut diff leaves out", () => {
    const r = reply({ files: [f("a.ts"), f("b.ts"), f("c.ts")], diff: part("a.ts"), truncated: true });
    expect(noticesOf(r)).toEqual(["The diff is cut at 4 MiB. 2 more files aren't shown."]);
  });
  it("says nothing of a whole diff with a base", () => expect(noticesOf(reply({ files: [f("a.ts")], diff: part("a.ts") }))).toEqual([]));
  it("names a missing base and the new files past 200", () => expect(noticesOf(reply({ base: null, more_new: 3 }))).toEqual([copy.noBase, copy.moreNew(3)]));
  it("says when a pull request's base isn't fetched", () =>
    expect(noticesOf(reply({ pr_base_unfetched: "develop" }))).toEqual(["This pull request targets develop, which isn't fetched here."]));
  it("says when the look for new files was cut", () =>
    expect(noticesOf(reply({ new_cut: true }))).toEqual(["Looking for new files took too long, so some may not be shown."]));
});
