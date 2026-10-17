import { describe, expect, it } from "vitest";
import { findPathCandidates } from "./pathCandidates";

const texts = (s: string) => findPathCandidates(s).map((c) => c.text);

describe("findPathCandidates", () => {
  it("finds a relative path with line and column, without the trailing dot", () => {
    const s = "Edited src/main.rs:42:7.";
    const [c, ...rest] = findPathCandidates(s);
    expect(rest).toEqual([]);
    expect(c!).toEqual({ text: "src/main.rs", start: 7, end: 23, line: 42, col: 7 });
    // The link covers the path and its suffix, not the sentence's dot.
    expect(s.slice(c!.start, c!.end)).toBe("src/main.rs:42:7");
  });

  it("reads a Windows path with a (line,col) or (line) suffix", () => {
    expect(findPathCandidates("C:\\dev\\x\\a.ts(12,3)")).toEqual([{ text: "C:\\dev\\x\\a.ts", start: 0, end: 19, line: 12, col: 3 }]);
    expect(findPathCandidates("C:/dev/a.ts(9)")).toEqual([{ text: "C:/dev/a.ts", start: 0, end: 14, line: 9 }]);
  });

  it("finds ./ and ../ paths", () => {
    expect(texts("see ./b/c.md and ../d.txt")).toEqual(["./b/c.md", "../d.txt"]);
  });

  it("finds absolute, home and bare-name paths", () => {
    expect(texts("/home/u/a.py")).toEqual(["/home/u/a.py"]);
    expect(texts("~/x/y.json")).toEqual(["~/x/y.json"]);
    expect(texts("error in main.rs now")).toEqual(["main.rs"]);
    expect(texts("src\\main.rs")).toEqual(["src\\main.rs"]);
  });

  it("is not fooled by a URL", () => {
    expect(texts("https://x.com/a.js")).toEqual([]);
    expect(texts("(https://x.com/a.js)")).toEqual([]);
  });

  it("drops wrappers and trailing punctuation", () => {
    for (const s of ["(src/a.rs)", '"src/a.rs"', "`src/a.rs`", "[src/a.rs]", "<src/a.rs>", "src/a.rs,", "(src/a.rs).", "'src/a.rs';"]) {
      expect(texts(s), s).toEqual(["src/a.rs"]);
    }
    const c = findPathCandidates("(src/a.rs:3)")[0]!;
    expect(c).toEqual({ text: "src/a.rs", start: 1, end: 11, line: 3 });
  });

  it("ignores plain words, version numbers, durations and a bare slash", () => {
    for (const s of ["hello world", "1.2.3", "v1.2.3", "took 1.5s", "12.3ms", "/", "a / b", "2026/10/17"]) {
      expect(texts(s), s).toEqual([]);
    }
  });

  it("never offers a UNC or device path", () => {
    for (const s of ["\\\\srv\\share\\a.txt", "//srv/share/a.txt", '"\\\\srv\\share\\a.txt"', "\\\\?\\C:\\a.txt", "\\\\.\\pipe\\x"]) {
      expect(texts(s), s).toEqual([]);
    }
  });

  it("skips a candidate longer than the core accepts", () => {
    expect(texts(`${"a/".repeat(600)}x.rs`)).toEqual([]);
  });

  it("counts offsets in UTF-16 units", () => {
    const s = "😀 src/a.rs";
    const c = findPathCandidates(s)[0]!;
    expect(s.slice(c.start, c.end)).toBe("src/a.rs");
  });
});
