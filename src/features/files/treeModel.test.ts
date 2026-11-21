import { describe, expect, it } from "vitest";
import type { TreeEntry } from "../../generated/TreeEntry";
import {
  absoluteOf,
  ancestors,
  childPath,
  nameParts,
  openFolders,
  relativeTo,
  treeKey,
  typeAhead,
  visibleRows,
  type Listing,
  type TreeRow,
} from "./treeModel";

const e = (name: string, kind: TreeEntry["kind"] = "file", ignored = false): TreeEntry => ({ name, kind, ignored });
const listing = (entries: TreeEntry[], more = 0, state: Listing["state"] = "listed"): Listing => ({
  entries,
  more,
  stamp: 1,
  state,
});
const key = (k: string, extra: Partial<Parameters<typeof treeKey>[0]> = {}) => ({
  key: k,
  ctrlKey: false,
  altKey: false,
  metaKey: false,
  shiftKey: false,
  ...extra,
});

const sample = () =>
  visibleRows(
    new Map([
      ["", listing([e("src", "folder"), e("lib", "folder"), e("a.ts"), e("l", "link")])],
      ["src", listing([e("x.ts"), e("y.ts")])],
    ]),
    new Set(["src"]),
  );

describe("path helpers", () => {
  it("childPath and ancestors", () => {
    expect(childPath("", "a")).toBe("a");
    expect(childPath("a", "b")).toBe("a/b");
    expect(ancestors("a/b/c.ts")).toEqual(["a", "a/b"]);
    expect(ancestors("c.ts")).toEqual([]);
  });

  it("relativeTo is case-insensitive on Windows and null outside the root", () => {
    expect(relativeTo("C:\\p", "c:\\P\\Src\\a.ts", true)).toBe("Src/a.ts");
    expect(relativeTo("C:\\p", "C:/p/src", true)).toBe("src");
    expect(relativeTo("C:\\p", "C:\\p", true)).toBe("");
    expect(relativeTo("C:\\p", "C:\\p2\\a", true)).toBeNull();
    expect(relativeTo("C:\\p", "D:\\p\\a", true)).toBeNull();
    expect(relativeTo("/p", "/P/a", false)).toBeNull();
    expect(relativeTo("/p", "/p/a/b", false)).toBe("a/b");
    expect(relativeTo("/p/", "/p/a", false)).toBe("a");
  });

  it("absoluteOf joins with the system's separator", () => {
    expect(absoluteOf("C:\\p", "src/a.ts", true)).toBe("C:\\p\\src\\a.ts");
    expect(absoluteOf("C:\\p", "", true)).toBe("C:\\p");
    expect(absoluteOf("C:\\p\\", "a", true)).toBe("C:\\p\\a");
    expect(absoluteOf("/p", "src/a.ts", false)).toBe("/p/src/a.ts");
    expect(absoluteOf("/p/", "a", false)).toBe("/p/a");
    expect(absoluteOf("/p", "", false)).toBe("/p");
  });
});

describe("visibleRows", () => {
  it("visible_rows_walk_open_folders_depth_first", () => {
    const rows = sample();
    expect(rows.map((r) => [r.path, r.level])).toEqual([
      ["src", 0],
      ["src/x.ts", 1],
      ["src/y.ts", 1],
      ["lib", 0],
      ["a.ts", 0],
      ["l", 0],
    ]);
    expect(rows[0]?.expanded).toBe(true);
    expect(rows[3]?.expanded).toBe(false);
    expect(rows[5]?.kind).toBe("link");
  });

  it("a folder asked but not yet listed shows as loading", () => {
    const rows = visibleRows(new Map([["", listing([e("src", "folder")])]]), new Set(["src"]));
    expect(rows).toHaveLength(1);
    expect(rows[0]?.loading).toBe(true);
    expect(rows[0]?.expanded).toBe(true);
  });

  it("visible_rows_put_a_more_row_after_a_cut_folder", () => {
    const rows = visibleRows(new Map([["", listing([e("a.ts")], 25000)]]), new Set());
    expect(rows.map((r) => r.kind)).toEqual(["file", "more"]);
    expect(rows[1]?.more).toBe(25000);
    expect(rows[1]?.level).toBe(0);
  });

  it("an open empty folder shows one empty row", () => {
    const rows = visibleRows(
      new Map([
        ["", listing([e("d", "folder")])],
        ["d", listing([])],
      ]),
      new Set(["d"]),
    );
    expect(rows.map((r) => [r.kind, r.level])).toEqual([
      ["folder", 0],
      ["empty", 1],
    ]);
  });

  it("set size and position count only real entries", () => {
    const rows = visibleRows(new Map([["", listing([e("a"), e("b"), e("c")], 9)]]), new Set());
    expect(rows.slice(0, 3).map((r) => [r.setSize, r.posInSet])).toEqual([
      [3, 1],
      [3, 2],
      [3, 3],
    ]);
  });

  it("openFolders lists only open folders whose parents are open, in row order", () => {
    const listings = new Map([
      ["", listing([e("a", "folder"), e("b", "folder")])],
      ["a", listing([e("c", "folder")])],
      ["a/c", listing([e("z.ts")])],
      ["b", listing([e("d", "folder")])],
    ]);
    // "b" is closed, so "b/d" (open in the set) is not drawn.
    const rows = visibleRows(listings, new Set(["a", "a/c", "b/d"]));
    expect(openFolders(rows)).toEqual(["a", "a/c"]);
  });
});

describe("treeKey", () => {
  const rows = sample(); // 0 src(open) 1 x 2 y 3 lib 4 a.ts 5 l

  it("Right opens a closed folder, then moves to its first child; Left closes, then moves to the parent", () => {
    expect(treeKey(key("ArrowRight"), rows, 3, false)).toEqual({ type: "expand", path: "lib" });
    expect(treeKey(key("ArrowRight"), rows, 0, false)).toEqual({ type: "focus", index: 1 });
    expect(treeKey(key("ArrowLeft"), rows, 0, false)).toEqual({ type: "collapse", path: "src" });
    expect(treeKey(key("ArrowLeft"), rows, 2, false)).toEqual({ type: "focus", index: 0 });
    expect(treeKey(key("ArrowLeft"), rows, 4, false)).toBeNull();
    expect(treeKey(key("ArrowRight"), rows, 4, false)).toBeNull();
    expect(treeKey(key("ArrowDown"), rows, 0, false)).toEqual({ type: "focus", index: 1 });
    expect(treeKey(key("ArrowUp"), rows, 0, false)).toBeNull();
    expect(treeKey(key("ArrowDown"), rows, 5, false)).toBeNull();
  });

  it("in right-to-left Left and Right swap", () => {
    expect(treeKey(key("ArrowLeft"), rows, 3, true)).toEqual({ type: "expand", path: "lib" });
    expect(treeKey(key("ArrowRight"), rows, 0, true)).toEqual({ type: "collapse", path: "src" });
    expect(treeKey(key("ArrowRight"), rows, 2, true)).toEqual({ type: "focus", index: 0 });
  });

  it("Enter opens a file kept and Space opens it as a preview; both toggle a folder", () => {
    expect(treeKey(key("Enter"), rows, 4, false)).toEqual({ type: "open", path: "a.ts", keep: true });
    expect(treeKey(key(" "), rows, 4, false)).toEqual({ type: "open", path: "a.ts", keep: false });
    expect(treeKey(key("Enter"), rows, 0, false)).toEqual({ type: "collapse", path: "src" });
    expect(treeKey(key(" "), rows, 3, false)).toEqual({ type: "expand", path: "lib" });
  });

  it("Home and End, Ctrl+F to the filter, Shift+F10 and the Menu key to the menu", () => {
    expect(treeKey(key("Home"), rows, 3, false)).toEqual({ type: "focus", index: 0 });
    expect(treeKey(key("End"), rows, 3, false)).toEqual({ type: "focus", index: 5 });
    expect(treeKey(key("f", { ctrlKey: true }), rows, 3, false)).toEqual({ type: "filter" });
    expect(treeKey(key("F10", { shiftKey: true }), rows, 3, false)).toEqual({ type: "menu" });
    expect(treeKey(key("ContextMenu"), rows, 3, false)).toEqual({ type: "menu" });
    expect(treeKey(key("F10"), rows, 3, false)).toBeNull();
  });

  it("a link row neither opens nor expands", () => {
    for (const k of ["Enter", " ", "ArrowRight"]) expect(treeKey(key(k), rows, 5, false)).toBeNull();
  });
});

describe("typeAhead and nameParts", () => {
  const rows: TreeRow[] = ["alpha", "beta", "Bravo", "alps"].map((name, i) => ({
    path: name,
    name,
    kind: "file",
    ignored: false,
    level: 0,
    expanded: false,
    loading: false,
    setSize: 4,
    posInSet: i + 1,
  }));

  it("type-ahead finds the next name that starts with the letters, wrapping", () => {
    expect(typeAhead(rows, 0, "b")).toBe(1);
    expect(typeAhead(rows, 1, "b")).toBe(2);
    expect(typeAhead(rows, 2, "a")).toBe(3);
    expect(typeAhead(rows, 3, "a")).toBe(0);
    expect(typeAhead(rows, 0, "alp")).toBe(0);
    expect(typeAhead(rows, 0, "z")).toBeNull();
  });

  it("nameParts marks every match ignoring case", () => {
    expect(nameParts("Banana", "AN")).toEqual([
      { text: "B", hit: false },
      { text: "an", hit: true },
      { text: "an", hit: true },
      { text: "a", hit: false },
    ]);
    expect(nameParts("abc", "")).toEqual([{ text: "abc", hit: false }]);
    expect(nameParts("abc", "x")).toEqual([{ text: "abc", hit: false }]);
  });
});
