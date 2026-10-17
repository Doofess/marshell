import { describe, expect, it } from "vitest";
import { displayPath, openPath, rankFiles, splitPath } from "./quickOpen";

describe("rankFiles: a path outside the root", () => {
  const seen = ["C:/Users/Home Office/AppData/Local/Temp/claude/C--dev-Airport/pv/page.html"];
  it("scores on its parent folder and name, so a short query does not match scattered letters", () => {
    const r = rankFiles("carg", ["Cargo.toml"], seen, 10);
    expect(r.map((h) => h.path)).toEqual(["Cargo.toml"]);
  });
  it("still matches on its name, with the ranges inside the full path", () => {
    const r = rankFiles("page", ["Cargo.toml"], seen, 10);
    expect(r.map((h) => h.path)).toEqual(seen);
    const [a, b] = r[0]!.ranges[0]!;
    expect(seen[0]!.slice(a, b)).toBe("page");
  });
});

describe("rankFiles", () => {
  it("ranks a basename match above a match scattered through the folders", () => {
    const r = rankFiles("mainrs", ["docs/maintenance/rs.md", "src/main.rs"], [], 10);
    expect(r.map((h) => h.path)).toEqual(["src/main.rs", "docs/maintenance/rs.md"]);
  });
  it("ranks a seen path above the same score from the list", () => {
    const r = rankFiles("a.ts", ["x/a.ts"], ["y/a.ts"], 10);
    expect(r.map((h) => h.path)).toEqual(["y/a.ts", "x/a.ts"]);
  });
  it("respects the limit", () => {
    const files = Array.from({ length: 50 }, (_, i) => `src/f${i}.ts`);
    expect(rankFiles("ts", files, [], 20)).toHaveLength(20);
  });
  it("returns only the seen paths, in order, for an empty query", () => {
    expect(rankFiles("", ["a.ts"], ["z.ts", "b.ts"], 10).map((h) => h.path)).toEqual(["z.ts", "b.ts"]);
    expect(rankFiles("  ", ["a.ts"], ["z.ts", "b.ts"], 1).map((h) => h.path)).toEqual(["z.ts"]);
  });
  it("gives the matched ranges", () => {
    expect(rankFiles("ma", ["main.rs"], [], 5)[0]!.ranges).toEqual([[0, 2]]);
  });
});

describe("paths", () => {
  it("shows a path inside the root relative to it, and keeps one outside absolute", () => {
    expect(displayPath("C:\\p\\Marshell\\src\\a.ts", "C:\\p\\Marshell")).toBe("src/a.ts");
    expect(displayPath("c:/p/marshell/a.ts", "C:\\p\\Marshell")).toBe("a.ts");
    expect(displayPath("/home/u/x.ts", "/home/me/proj")).toBe("/home/u/x.ts");
    expect(displayPath("/home/me/projx/x.ts", "/home/me/proj")).toBe("/home/me/projx/x.ts");
  });
  it("opens a listed path under the root with the root's separator", () => {
    expect(openPath("src/a.ts", "C:\\p\\M")).toBe("C:\\p\\M\\src\\a.ts");
    expect(openPath("src/a.ts", "/home/m/")).toBe("/home/m/src/a.ts");
  });
  it("splits a display path into name and folder", () => {
    expect(splitPath("src/a/b.ts")).toEqual({ name: "b.ts", dir: "src/a" });
    expect(splitPath("b.ts")).toEqual({ name: "b.ts", dir: "" });
  });
});
