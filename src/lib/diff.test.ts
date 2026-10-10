import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../components/DiffView/diff";
import { unifiedDiff } from "./diff";

describe("unifiedDiff", () => {
  it("is empty when nothing changed", () => {
    expect(unifiedDiff("a.json", "{}\n", "{}\n")).toBe("");
  });
  it("names the file with a/ and b/ prefixes and counts the change", () => {
    const d = unifiedDiff("C:/Users/Home Office/.claude/settings.json", '{\n  "a": 1\n}\n', '{\n  "a": 2,\n  "b": 3\n}\n');
    expect(d.split("\n").slice(0, 2)).toEqual(["--- a/C:/Users/Home Office/.claude/settings.json", "+++ b/C:/Users/Home Office/.claude/settings.json"]);
    const p = parseUnifiedDiff(d);
    expect(p.added).toBe(2);
    expect(p.removed).toBe(1);
  });
  it("writes a new file's hunk header as git does, @@ -0,0 +1,N @@, and an emptied file's as @@ -1,N +0,0 @@", () => {
    const made = unifiedDiff("f.json", "", "a\nb\n");
    expect(made).toContain("@@ -0,0 +1,2 @@");
    expect(parseUnifiedDiff(made)).toMatchObject({ added: 2, removed: 0, files: ["f.json"] });
    expect(unifiedDiff("f.json", "a\nb\n", "")).toContain("@@ -1,2 +0,0 @@");
  });
  it("keeps three lines of context around a change in a long file", () => {
    const before = Array.from({ length: 40 }, (_, i) => `line ${i}`).join("\n") + "\n";
    const after = before.replace("line 20", "LINE 20");
    const ctx = parseUnifiedDiff(unifiedDiff("f.txt", before, after)).lines.filter((l) => l.kind === "context");
    expect(ctx).toHaveLength(6);
  });
});
