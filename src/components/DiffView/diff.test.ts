import { describe, expect, it } from "vitest";
import { collapse, parseUnifiedDiff } from "./diff";

const SAMPLE = [
  "--- a/settings.json",
  "+++ b/settings.json",
  "@@ -3,4 +3,6 @@",
  '   "model": "opus",',
  '-  "cleanupPeriodDays": 30,',
  '+  "cleanupPeriodDays": 365,',
  '+  "hooks": {},',
  "   \"theme\": \"dark\"",
].join("\n");

describe("parseUnifiedDiff", () => {
  it("counts additions and removals and finds the file", () => {
    const d = parseUnifiedDiff(SAMPLE);
    expect(d.added).toBe(2);
    expect(d.removed).toBe(1);
    expect(d.files).toEqual(["settings.json"]);
  });
  it("numbers lines from the hunk header on both sides", () => {
    const l = parseUnifiedDiff(SAMPLE).lines;
    const ctx = l.find((x) => x.kind === "context")!;
    expect([ctx.oldNo, ctx.newNo]).toEqual([3, 3]);
    const del = l.find((x) => x.kind === "del")!;
    expect(del.oldNo).toBe(4);
    const add = l.find((x) => x.kind === "add")!;
    expect(add.newNo).toBe(4);
  });
  it("treats a removed line that starts with dashes as a removal inside a hunk", () => {
    const d = parseUnifiedDiff(["--- a/x", "+++ b/x", "@@ -1,2 +1,1 @@", "--- not a header", " keep"].join("\n"));
    expect(d.removed).toBe(1);
  });
  it("reads two files that follow each other without a 'diff ' line between them", () => {
    const two = [
      "--- a/a.json", "+++ b/a.json", "@@ -1,2 +1,2 @@", "-old a", "+new a", " same",
      "--- /dev/null", "+++ b/b.json", "@@ -1,0 +1,1 @@", "+new b",
    ].join("\n");
    const d = parseUnifiedDiff(two);
    expect(d.files).toEqual(["a.json", "b.json"]);
    expect([d.added, d.removed]).toEqual([2, 1]);
  });
  it("reads CRLF input and an empty diff", () => {
    expect(parseUnifiedDiff(SAMPLE.replace(/\n/g, "\r\n")).added).toBe(2);
    expect(parseUnifiedDiff("")).toEqual({ lines: [], added: 0, removed: 0, files: [] });
  });
});

describe("collapse", () => {
  it("keeps the first lines and says how many are hidden", () => {
    const lines = Array.from({ length: 200 }, (_, i) => ({ kind: "context" as const, text: `l${i}` }));
    const c = collapse(lines, 40);
    expect(c.shown).toHaveLength(40);
    expect(c.hidden).toBe(160);
    expect(collapse(lines.slice(0, 10), 40).hidden).toBe(0);
  });
});
