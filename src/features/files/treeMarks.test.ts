import { describe, expect, it } from "vitest";
import { MARK_LETTER, MARK_WORD, marksOf, withMarks } from "./treeMarks";
import type { TreeRow } from "./treeModel";

const row = (path: string, kind: TreeRow["kind"] = "file"): TreeRow => ({ path, name: path.split("/").pop() ?? path, kind, ignored: false, level: 0, expanded: false, loading: false, setSize: 1, posInSet: 1 });

describe("marksOf", () => {
  it("marks every ancestor folder of a change", () => {
    expect([...marksOf([{ path: "a/b/c.ts", status: "modified" }]).folders]).toEqual(["a", "a/b"]);
  });
  it("keeps each file's status", () => {
    expect(marksOf([{ path: "x.ts", status: "added" }]).files.get("x.ts")).toBe("added");
  });
});

describe("the mark copy", () => {
  it("has a letter and a word for every status", () => {
    expect(MARK_LETTER).toEqual({ modified: "M", added: "A", untracked: "A", renamed: "R", deleted: "D" });
    expect(MARK_WORD).toEqual({ modified: ", modified", added: ", new", untracked: ", new", renamed: ", renamed", deleted: ", deleted" });
  });
});

describe("withMarks", () => {
  it("puts the status on files and holdsChanges on folders, and returns the same rows with no changes", () => {
    const rows = [row("a", "folder"), row("a/c.ts"), row("z.ts")];
    expect(withMarks(rows, [])).toBe(rows);
    const out = withMarks(rows, [{ path: "a/c.ts", status: "modified" }]);
    expect(out[0]?.holdsChanges).toBe(true);
    expect(out[1]?.change).toBe("modified");
    expect(out[2]?.change).toBeUndefined();
  });
});
