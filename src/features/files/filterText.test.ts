import { describe, expect, it } from "vitest";
import { filterMode, namesNote, searchNote, SHORT_NOTE } from "./filterText";

describe("filterText", () => {
  it("filterMode reads names, ? with text, and ? alone", () => {
    expect(filterMode("abc")).toEqual({ kind: "names", query: "abc" });
    expect(filterMode("?ab")).toEqual({ kind: "search", query: "ab" });
    expect(filterMode("?a")).toEqual({ kind: "short" });
    expect(filterMode("?")).toEqual({ kind: "short" });
    expect(filterMode("")).toEqual({ kind: "none" });
    expect(filterMode("  ")).toEqual({ kind: "none" });
  });

  it("the notes read as the plan says", () => {
    expect(SHORT_NOTE).toBe("Type at least 2 characters to search.");
    const reply = (files: number, cut: "lines" | "time" | null) => ({
      root: "r",
      cut,
      files: Array.from({ length: files }, (_, i) => ({ path: `f${i}`, lines: [{ line: 1, parts: [] }] })),
    });
    expect(searchNote(reply(0, null), "zz", false)).toBe("No file has “zz”.");
    expect(searchNote(reply(1, "lines"), "zz", false)).toBe("Showing the first 1,000 matching lines.");
    expect(searchNote(reply(1, "time"), "zz", false)).toBe("The search stopped after 3 seconds. Showing what it found.");
    expect(searchNote(reply(1, null), "zz", false)).toBeNull();
    expect(searchNote(null, "zz", true)).toBe("The search failed. Try again.");
    expect(searchNote(null, "zz", false)).toBeNull();
    expect(namesNote({ matches: 0, capped: false, listCut: false }, "q")).toBe("No file or folder name has “q”.");
    expect(namesNote({ matches: 500, capped: true, listCut: false }, "q")).toBe("Showing 500 matches. Type more to narrow them.");
    expect(namesNote({ matches: 3, capped: false, listCut: true }, "q")).toBe("Only the first 20,000 files are filtered.");
    expect(namesNote({ matches: 3, capped: false, listCut: false }, "q")).toBeNull();
  });
});
