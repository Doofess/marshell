import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { SearchReply } from "../../generated/SearchReply";
import { SearchResults, searchKey, searchRows, searchSaid, type SearchRow } from "./SearchResults";

const reply: SearchReply = {
  root: "r",
  cut: null,
  files: [
    { path: "src/a.ts", lines: [{ line: 12, parts: [{ text: "let ", hit: false }, { text: "foo", hit: true }] }, { line: 20, parts: [{ text: "foo", hit: true }] }] },
    { path: "b.md", lines: [{ line: 1, parts: [{ text: "foo", hit: true }] }] },
  ],
};
const key = (k: string, over: object = {}) => ({ key: k, ctrlKey: false, altKey: false, metaKey: false, shiftKey: false, ...over });
const noop = () => {};

describe("SearchResults", () => {
  it("searchSaid counts lines and files, singular and plural", () => {
    expect(searchSaid(reply)).toBe("3 matching lines in 2 files.");
    expect(searchSaid({ root: "r", cut: null, files: [{ path: "a", lines: [{ line: 1, parts: [] }] }] })).toBe("1 matching line in 1 file.");
    expect(searchSaid({ root: "r", cut: null, files: [] })).toBe("No matches.");
  });

  it("results are a tree of files and their lines, hits in bold", () => {
    const out = renderToStaticMarkup(<SearchResults reply={reply} rtl={false} onOpen={noop} announce={noop} keyboardHere={() => false} />);
    expect(out).toContain('role="tree"');
    expect(out.match(/aria-level="1"/g)).toHaveLength(2);
    expect(out.match(/aria-level="2"/g)).toHaveLength(3);
    expect(out).toContain(">12<");
    expect(out).toContain("<b");
    expect(out.match(/tabindex="0"/g)).toHaveLength(1);
  });

  it("the results' keys move, open and close like the tree's", () => {
    const rows = searchRows(reply, new Set());
    expect(rows.map((r) => r.kind)).toEqual(["file", "line", "line", "file", "line"]);
    expect(searchRows(reply, new Set(["src/a.ts"])).map((r) => r.kind)).toEqual(["file", "file", "line"]);
    expect(searchKey(key("ArrowDown"), rows, 0, false)).toEqual({ type: "focus", index: 1 });
    expect(searchKey(key("ArrowUp"), rows, 1, false)).toEqual({ type: "focus", index: 0 });
    expect(searchKey(key("Home"), rows, 3, false)).toEqual({ type: "focus", index: 0 });
    expect(searchKey(key("End"), rows, 0, false)).toEqual({ type: "focus", index: 4 });
    expect(searchKey(key("ArrowLeft"), rows, 2, false)).toEqual({ type: "focus", index: 0 });
    expect(searchKey(key("ArrowLeft"), rows, 0, false)).toEqual({ type: "collapse", path: "src/a.ts" });
    expect(searchKey(key("ArrowRight"), rows, 0, false)).toEqual({ type: "focus", index: 1 });
    const closed: SearchRow[] = searchRows(reply, new Set(["src/a.ts"]));
    expect(searchKey(key("ArrowRight"), closed, 0, false)).toEqual({ type: "expand", path: "src/a.ts" });
    expect(searchKey(key("ArrowRight"), rows, 0, true)).toEqual({ type: "collapse", path: "src/a.ts" });
    expect(searchKey(key("Enter"), rows, 2, false)).toEqual({ type: "open", path: "src/a.ts", line: 20, keep: false });
    expect(searchKey(key("Enter"), rows, 0, false)).toEqual({ type: "open", path: "src/a.ts", keep: true });
    expect(searchKey(key("f", { ctrlKey: true }), rows, 0, false)).toEqual({ type: "filter" });
  });
});
