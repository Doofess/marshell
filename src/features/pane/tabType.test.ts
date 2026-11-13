import { describe, expect, it } from "vitest";
import { splitName, tabKind } from "./tabType";

describe("tabKind", () => {
  it("is web for every browser tab, whatever the address ends in", () => {
    expect(tabKind(true, "https://example.com/a.pdf")).toBe("web");
  });
  it("is pdf, image or text for a file", () => {
    expect(tabKind(false, "C:\\a\\b.pdf")).toBe("pdf");
    expect(tabKind(false, "x.png")).toBe("image");
    for (const f of ["x.ts", "x.txt", "x.md", "x.js", "x.ps1", "x.html", "Makefile"]) expect(tabKind(false, f)).toBe("text");
  });
});

describe("splitName", () => {
  it("splits at the last dot when what follows is an extension", () => {
    expect(splitName("sample-long-name.txt")).toEqual({ head: "sample-long-name", tail: ".txt" });
    expect(splitName("archive.tar.gz")).toEqual({ head: "archive.tar", tail: ".gz" });
  });
  it("keeps the whole name as the head otherwise", () => {
    for (const n of [".gitignore", "README", "notes.", "a.backup-old"]) expect(splitName(n)).toEqual({ head: n, tail: "" });
  });
});
