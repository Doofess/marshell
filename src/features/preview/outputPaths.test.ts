import { describe, expect, it, vi } from "vitest";
import { outputPaths, seenPaths, type OutputBuffer } from "./outputPaths";

/** A buffer of text rows; a row in `wrapped` continues the one before it. */
function buffer(rows: string[], wrapped: number[] = []): OutputBuffer {
  return {
    length: rows.length,
    getLine: (y) => {
      const row = rows[y];
      if (row === undefined) return undefined;
      return { isWrapped: wrapped.includes(y), length: row.length, getCell: (x) => (x < row.length ? { getChars: () => row[x]!, getWidth: () => 1 } : undefined) };
    },
  };
}

describe("outputPaths", () => {
  it("reads the newest lines first", () => {
    expect(outputPaths(buffer(["old/a.ts", "mid/b.ts", "new/c.ts"]))).toEqual(["new/c.ts", "mid/b.ts", "old/a.ts"]);
  });
  it("scans only the last 2000 lines", () => {
    const rows = ["first/gone.ts", ...Array.from({ length: 2000 }, () => "plain words")];
    expect(outputPaths(buffer(rows))).toEqual([]);
    expect(outputPaths(buffer(["first/kept.ts", ...Array.from({ length: 1999 }, () => "plain")]))).toEqual(["first/kept.ts"]);
  });
  it("drops duplicates", () => {
    expect(outputPaths(buffer(["a/x.ts a/x.ts", "a/x.ts:3", "b/y.ts"]))).toEqual(["b/y.ts", "a/x.ts"]);
  });
  it("sends at most 64 candidates", () => {
    const rows = Array.from({ length: 100 }, (_, i) => `d/f${i}.ts`);
    const got = outputPaths(buffer(rows));
    expect(got).toHaveLength(64);
    expect(got[0]).toBe("d/f99.ts");
  });
  it("reads a wrapped line once, as one path", () => {
    expect(outputPaths(buffer(["see src/deep/", "er/file.ts now"], [1]))).toEqual(["src/deep/er/file.ts"]);
  });
});

describe("seenPaths", () => {
  it("keeps the resolved paths in order, once each", async () => {
    const resolve = vi.fn(async () => ["/r/b.ts", null, "/r/b.ts", "/r/a.ts"]);
    const got = await seenPaths("tab", buffer(["a/1.ts", "a/2.ts", "a/3.ts", "a/4.ts"]), resolve);
    expect(got).toEqual(["/r/b.ts", "/r/a.ts"]);
    expect(resolve).toHaveBeenCalledTimes(1);
  });
  it("does not ask when nothing looks like a path", async () => {
    const resolve = vi.fn(async () => []);
    expect(await seenPaths("tab", buffer(["hello"]), resolve)).toEqual([]);
    expect(resolve).not.toHaveBeenCalled();
  });
});
