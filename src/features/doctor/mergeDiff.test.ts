import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../../components/DiffView/diff";
import { unifiedDiff } from "../../lib/diff";
import { mergeDiffs } from "./mergeDiff";

const base = Array.from({ length: 20 }, (_, i) => `line ${i + 1}`).join("\n") + "\n";
const edit = (lineNo: number, text: string) => base.split("\n").map((l, i) => (i === lineNo - 1 ? text : l)).join("\n");
const add = (lineNo: number, text: string) => base.split("\n").flatMap((l, i) => (i === lineNo - 1 ? [l, text] : [l])).join("\n");

describe("mergeDiffs", () => {
  const a = unifiedDiff("f.json", base, edit(15, "late change"), 1);
  const b = unifiedDiff("f.json", base, add(3, "inserted early"), 1);
  const merged = mergeDiffs([a, b]);

  it("keeps one file header, not one per diff", () => {
    expect((merged.match(/^--- /gm) ?? []).length).toBe(1);
    expect((merged.match(/^\+\+\+ /gm) ?? []).length).toBe(1);
  });
  it("orders the hunks by where they are in the file, whatever order the diffs came in", () => {
    const starts = [...merged.matchAll(/^@@ -(\d+)/gm)].map((m) => Number(m[1]));
    expect(starts).toEqual([...starts].sort((x, y) => x - y));
    expect(starts).toHaveLength(2);
  });
  it("moves the new-side line numbers of a later hunk by what the earlier hunks added", () => {
    const hunks = [...merged.matchAll(/^@@ -(\d+),(\d+) \+(\d+),(\d+) @@/gm)].map((m) => m.slice(1, 5).map(Number));
    // The early hunk adds one line, so the later hunk sits one line further down on the new side.
    expect(hunks[1]![2]!).toBe(hunks[1]![0]! + 1);
  });
  it("parses as one diff with the counts of both", () => {
    const p = parseUnifiedDiff(merged);
    expect(p.added).toBe(2);
    expect(p.removed).toBe(1);
    expect(p.files).toEqual(["f.json"]);
  });
  it("is the diff itself for one diff, and empty for none", () => {
    expect(mergeDiffs([a])).toBe(a);
    expect(mergeDiffs([])).toBe("");
  });
});
