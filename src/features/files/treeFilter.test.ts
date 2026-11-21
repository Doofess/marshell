import { describe, expect, it } from "vitest";
import { filterRows } from "./treeFilter";

describe("filterRows", () => {
  it("the filter keeps matching files inside their folders, opened", () => {
    const { rows, matches, capped } = filterRows(["src/a/Button.tsx", "src/b.ts"], "butt", new Set());
    expect(rows.map((r) => [r.path, r.kind, r.level, r.expanded])).toEqual([
      ["src", "folder", 0, true],
      ["src/a", "folder", 1, true],
      ["src/a/Button.tsx", "file", 2, false],
    ]);
    expect(rows[2]?.parts).toEqual([
      { text: "Butt", hit: true },
      { text: "on.tsx", hit: false },
    ]);
    expect(matches).toBe(1);
    expect(capped).toBe(false);
  });

  it("a matched folder shows closed, and opened lists every file under it", () => {
    const files = ["docs/a.md", "docs/sub/b.md", "x.ts"];
    const closed = filterRows(files, "docs", new Set());
    expect(closed.rows.map((r) => [r.path, r.expanded])).toEqual([["docs", false]]);
    const open = filterRows(files, "docs", new Set(["docs"]));
    expect(open.rows.map((r) => r.path)).toEqual(["docs", "docs/sub", "docs/sub/b.md", "docs/a.md"]);
    expect(open.rows[0]?.expanded).toBe(true);
  });

  it("the filter stops at 500 matches and says it was capped", () => {
    const files = Array.from({ length: 600 }, (_, i) => `d/file${i}.ts`);
    const r = filterRows(files, "file", new Set());
    expect(r.capped).toBe(true);
    expect(r.matches).toBe(500);
    expect(r.rows.filter((x) => x.kind === "file")).toHaveLength(500);
  });

  it("an empty query or no match gives no rows", () => {
    expect(filterRows(["a.ts"], "", new Set()).rows).toEqual([]);
    expect(filterRows(["a.ts"], "zz", new Set())).toEqual({ rows: [], matches: 0, capped: false });
  });
});
