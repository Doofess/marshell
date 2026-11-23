import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ChangesReply } from "../../generated/ChangesReply";
import { ChangesEmpty } from "./ChangesPanel";
import { DiffList, listKey } from "./DiffList";
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
  renderToStaticMarkup(<DiffList items={list} focus={o.focus ?? 0} range={o.range ?? null} wrap={o.wrap ?? false} onKey={() => {}} onClickItem={() => {}} onOpenItem={() => {}} />);

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

const reply = (o: Partial<ChangesReply>): ChangesReply => ({ kind: "listed", stamp: "s", folder: "C:\\repo", moved: false, branch: "main", detached: null, base: "origin/main", unrelated: false, ahead: 0, uncommitted: false, files: [], diff: "", truncated: false, more_new: 0, reason: null, ...o });
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
});
