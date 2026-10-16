import { describe, expect, it } from "vitest";
import { sectionNote, splitDiff } from "./splitDiff";

const A = "diff --git a/src/a.ts b/src/a.ts\nindex 111..222 100644\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1,1 +1,1 @@\n-one\n+two\n";
const C = "diff --git a/new dir/c.txt b/new dir/c.txt\nnew file mode 100644\n--- /dev/null\n+++ b/new dir/c.txt\n@@ -0,0 +1,1 @@\n+hello\n";

describe("splitDiff", () => {
  it("gives nothing for an empty diff", () => {
    expect(splitDiff("")).toEqual([]);
  });
  it("splits a two-file diff, keeping each section whole", () => {
    expect(splitDiff(A + C)).toEqual([
      { path: "src/a.ts", diff: A },
      { path: "new dir/c.txt", diff: C },
    ]);
  });
  it("keeps a path with a space", () => {
    expect(splitDiff(C)[0]?.path).toBe("new dir/c.txt");
  });
  it("reads a path that itself contains ' b/'", () => {
    const d = "diff --git a/x b/y.txt b/x b/y.txt\nnew file mode 100644\n--- /dev/null\n+++ b/x b/y.txt\n@@ -0,0 +1 @@\n+z\n";
    expect(splitDiff(d)[0]?.path).toBe("x b/y.txt");
  });
  it("reads a binary or large section, which has no +++ line", () => {
    const bin = "diff --git a/img/logo.png b/img/logo.png\nnew file mode 100644\nBinary file img/logo.png\n";
    const big = "diff --git a/big.log b/big.log\nnew file mode 100644\nLarge file big.log\n";
    const parts = splitDiff(A + bin + big);
    expect(parts.map((p) => p.path)).toEqual(["src/a.ts", "img/logo.png", "big.log"]);
    expect(parts[1]?.diff).toBe(bin);
  });
  it("takes a renamed file's path from its new name", () => {
    const d = "diff --git a/old name.ts b/new name.ts\nsimilarity index 90%\nrename from old name.ts\nrename to new name.ts\nindex 1..2 100644\n--- a/old name.ts\n+++ b/new name.ts\n@@ -1 +1 @@\n-a\n+b\n";
    expect(splitDiff(d)).toEqual([{ path: "new name.ts", diff: d }]);
  });
  it("does not mistake a diffed line that starts with 'diff --git' for a new section", () => {
    const d = "diff --git a/s.md b/s.md\n--- a/s.md\n+++ b/s.md\n@@ -1 +1 @@\n- diff --git a/q b/q\n+ diff --git a/r b/r\n";
    expect(splitDiff(d)).toHaveLength(1);
  });
  it("handles CRLF line ends", () => {
    expect(splitDiff(A.replace(/\n/g, "\r\n") + C.replace(/\n/g, "\r\n")).map((p) => p.path)).toEqual(["src/a.ts", "new dir/c.txt"]);
  });
});

describe("sectionNote", () => {
  it("names a binary and a large file, and nothing else", () => {
    expect(sectionNote("diff --git a/i.png b/i.png\nnew file mode 100644\nBinary file i.png\n")).toBe("binary");
    expect(sectionNote("diff --git a/i.png b/i.png\nindex 1..2 100644\nBinary files a/i.png and b/i.png differ\n")).toBe("binary");
    expect(sectionNote("diff --git a/b.log b/b.log\nnew file mode 100644\nLarge file b.log\n")).toBe("large");
    expect(sectionNote(A)).toBeNull();
  });
  it("does not read a diffed line as a note", () => {
    expect(sectionNote("diff --git a/s b/s\n--- a/s\n+++ b/s\n@@ -1 +1 @@\n+Binary file x\n")).toBeNull();
  });
});
