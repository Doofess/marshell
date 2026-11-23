import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { clickMove, extendTyped, FileTree, scrollsToRow, visibleRange } from "./FileTree";
import type { TreeRow } from "./treeModel";

const row = (over: Partial<TreeRow>): TreeRow => ({
  path: "a.txt",
  name: "a.txt",
  kind: "file",
  ignored: false,
  level: 0,
  expanded: false,
  loading: false,
  setSize: 1,
  posInSet: 1,
  ...over,
});
const noop = () => {};
const render = (rows: TreeRow[], over: Partial<Parameters<typeof FileTree>[0]> = {}) =>
  renderToStaticMarkup(
    <FileTree rows={rows} rootName="Airport" noFolder={false} selectedPath={null} focusedPath={null} rtl={false} onMove={noop} onFocusRow={noop} onMenu={noop} onDoubleOpen={noop} {...over} />,
  );

describe("FileTree", () => {
  it("is a tree named for its root, with levels, sizes and positions", () => {
    const out = render([
      row({ path: "src", name: "src", kind: "folder", expanded: true, setSize: 2, posInSet: 1 }),
      row({ path: "src/main.ts", name: "main.ts", level: 1, setSize: 1, posInSet: 1 }),
      row({ path: "README.md", name: "README.md", setSize: 2, posInSet: 2 }),
    ]);
    expect(out).toContain('role="tree"');
    expect(out).toContain('aria-label="Files in Airport"');
    expect(out).toContain('aria-level="1"');
    expect(out).toContain('aria-level="2"');
    expect(out).toContain('aria-setsize="2"');
    expect(out).toContain('aria-posinset="2"');
    expect(out.match(/role="treeitem"/g)).toHaveLength(3);
    // aria-expanded sits on the folder only.
    expect(out.match(/aria-expanded=/g)).toHaveLength(1);
    expect(out).toContain('aria-expanded="true"');
  });

  it("a closed folder says it is collapsed", () => {
    expect(render([row({ path: "src", name: "src", kind: "folder" })])).toContain('aria-expanded="false"');
  });

  it("an ignored row is dimmed and read as ignored", () => {
    const out = render([row({ path: "dist", name: "dist", kind: "folder", ignored: true })]);
    expect(out).toContain('data-ignored="true"');
    expect(out).toContain("dist, ignored");
  });

  it("a link row is read as a link", () => {
    const out = render([row({ path: "current", name: "current", kind: "link" })]);
    expect(out).toContain("current, link");
  });

  it("an ignored link reads both", () => {
    expect(render([row({ name: "x", kind: "link", ignored: true })])).toContain("x, ignored, link");
  });

  it("the open file is marked selected", () => {
    const out = render([row({ path: "a.txt" }), row({ path: "b.txt", name: "b.txt" })], { selectedPath: "b.txt" });
    expect(out.match(/aria-selected="true"/g)).toHaveLength(1);
    expect(out.match(/aria-selected="false"/g)).toHaveLength(1);
    expect(out.indexOf('aria-selected="true"')).toBeGreaterThan(out.indexOf('aria-selected="false"'));
  });

  it("has one tab stop: the row with the keyboard, or the first", () => {
    const rows = [row({ path: "a.txt" }), row({ path: "b.txt", name: "b.txt" })];
    expect(render(rows, { focusedPath: "b.txt" }).match(/tabindex="0"/g)).toHaveLength(1);
    expect(render(rows, { focusedPath: "b.txt" }).indexOf('tabindex="0"')).toBeGreaterThan(render(rows).indexOf('tabindex="0"'));
    expect(render(rows).match(/tabindex="0"/g)).toHaveLength(1);
  });

  it("a more row reads And 25,000 more", () => {
    const out = render([row({ path: "\u0000more", name: "", kind: "more", more: 25000 })]);
    expect(out).toContain("And 25,000 more");
    expect(out).toContain('aria-disabled="true"');
  });

  it("a note row's selected state comes from selectedProps", () => {
    const out = render([row({ path: "\u0000more", name: "", kind: "more", more: 3 })]);
    expect(out).toContain("is-selected-mark");
    expect(out).toContain('aria-selected="false"');
    expect(readFileSync(new URL("./FileTree.tsx", import.meta.url), "utf8")).not.toContain("aria-selected={false}");
  });

  it("an empty row is disabled and says so", () => {
    const out = render([row({ path: "a/\u0000empty", name: "", kind: "empty", level: 1 })]);
    expect(out).toContain("Empty");
    expect(out).toContain('aria-disabled="true"');
  });

  it("no folder reads This session has no folder the tree can show.", () => {
    const out = render([], { noFolder: true });
    expect(out).toContain("This session has no folder the tree can show.");
    expect(out).not.toContain('role="tree"');
  });

  it("gives a row its indent, its tooltip and a name that shortens in the middle", () => {
    const out = render([row({ path: "src/deep/main.ts", name: "main.ts", level: 2 })]);
    expect(out).toContain('title="src/deep/main.ts"');
    expect(out).toContain("--level:2");
    expect(out).toContain('<span class="file-tree__head">main</span><span class="file-tree__tail">.ts</span>');
  });

  it("bolds the letters a name filter matched", () => {
    const out = render([row({ name: "main.ts", parts: [{ text: "ma", hit: true }, { text: "in.ts", hit: false }] })]);
    expect(out).toContain('<b class="file-tree__hit">ma</b>');
  });

  it("draws an icon that is hidden from a screen reader", () => {
    const out = render([row({})]);
    expect(out).toContain('aria-hidden="true"');
    expect(out).toContain('stroke="currentColor"');
  });

  it("mirrors in a right-to-left layout", () => {
    expect(render([row({})], { rtl: true })).toContain('data-rtl="true"');
  });
});

describe("visibleRange", () => {
  it("shows the rows in view plus 10 above and below, between spacers", () => {
    const r = visibleRange({ count: 1000, scrollTop: 24 * 100, viewport: 24 * 20, focused: 105 });
    expect(r.start).toBe(90);
    expect(r.end).toBe(130);
    expect(r.extra).toBeNull();
  });
  it("starts at the top and ends at the last row", () => {
    expect(visibleRange({ count: 5, scrollTop: 0, viewport: 240, focused: 0 })).toEqual({ start: 0, end: 5, extra: null });
  });
  it("always keeps the row with the keyboard", () => {
    expect(visibleRange({ count: 1000, scrollTop: 0, viewport: 240, focused: 700 }).extra).toBe(700);
  });
  it("copes with no rows", () => {
    expect(visibleRange({ count: 0, scrollTop: 0, viewport: 240, focused: 0 })).toEqual({ start: 0, end: 0, extra: null });
  });
});

describe("extendTyped", () => {
  it("extends the letters typed within 500 ms and starts over after", () => {
    const a = extendTyped({ text: "", at: 0 }, "s", 1000);
    expect(a).toEqual({ text: "s", at: 1000 });
    expect(extendTyped(a, "r", 1400)).toEqual({ text: "sr", at: 1400 });
    expect(extendTyped(a, "r", 1501)).toEqual({ text: "r", at: 1501 });
  });
});

describe("clickMove", () => {
  it("a click opens a file as a preview, toggles a folder, and does nothing on a link, a more or an empty row", () => {
    expect(clickMove(row({ path: "a.ts", kind: "file" }))).toEqual({ type: "open", path: "a.ts", keep: false });
    expect(clickMove(row({ path: "src", kind: "folder", expanded: false }))).toEqual({ type: "expand", path: "src" });
    expect(clickMove(row({ path: "src", kind: "folder", expanded: true }))).toEqual({ type: "collapse", path: "src" });
    expect(clickMove(row({ path: "current", kind: "link" }))).toBeNull();
    expect(clickMove(row({ path: "d/more", kind: "more" }))).toBeNull();
    expect(clickMove(row({ path: "d/empty", kind: "empty" }))).toBeNull();
  });
});

describe("scrollsToRow", () => {
  // PR review of 6r-2: a file an agent added above the row with the keyboard moved its index, and the tree scrolled back to it
  // from wherever the person had scrolled to read.
  it("scrolls when the row becomes the one with the keyboard, or first appears, or after a key move", () => {
    expect(scrollsToRow("src/a.ts", null, false)).toBe(true);
    expect(scrollsToRow("src/b.ts", "src/a.ts", false)).toBe(true);
    expect(scrollsToRow("src/a.ts", "src/a.ts", true)).toBe(true);
  });
  it("leaves the view where it is when the same row only moved", () => {
    expect(scrollsToRow("src/a.ts", "src/a.ts", false)).toBe(false);
  });
  it("reads a modified file aloud", () => {
    const out = render([row({ path: "a/c.ts", name: "c.ts", change: "modified" })]);
    expect(out).toContain('aria-label="c.ts, modified"');
    expect(out).toContain('<span class="file-tree__mark" aria-hidden="true">M</span>');
  });
  it("a folder holding changes says so", () => {
    const out = render([row({ path: "a/b", name: "b", kind: "folder", holdsChanges: true })]);
    expect(out).toContain('aria-label="b, has changes"');
    expect(out).toContain('data-changes=""');
  });
  it("an unmarked row has no mark", () => {
    expect(render([row({})])).not.toContain("file-tree__mark");
  });
  it("is what the tree's scroll effect asks, and a row that leaves the tree is shown again when it comes back", () => {
    const tree = readFileSync(new URL("./FileTree.tsx", import.meta.url), "utf8");
    expect(tree).toContain("if (!scrollsToRow(p.focusedPath, shown.current, key)) return;");
    expect(tree).toContain("shown.current = null;");
  });
});
