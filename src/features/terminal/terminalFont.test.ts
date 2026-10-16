import { describe, expect, it } from "vitest";
import { fontStack, resolveTerminalFont } from "./terminalFont";

describe("resolveTerminalFont", () => {
  it("yours wins", () => {
    expect(resolveTerminalFont({ family: "Fira Code", size: 15 }, { family: "Cascadia Mono", size: 12 })).toEqual({ family: "Fira Code", size: 15, source: "yours" });
  });
  it("windows terminal next, at your size", () => {
    expect(resolveTerminalFont({ family: null, size: 14 }, { family: "Cascadia Mono", size: 12 })).toEqual({ family: "Cascadia Mono", size: 14, source: "windows-terminal" });
  });
  it("bundled last", () => {
    expect(resolveTerminalFont({ family: null, size: 13 }, null)).toEqual({ family: "JetBrains Mono", size: 13, source: "bundled" });
    expect(resolveTerminalFont({ family: "  ", size: 13 }, { size: 11 }).source).toBe("bundled");
  });
});

describe("fontStack", () => {
  it("family stack always ends in JetBrains Mono, monospace", () => {
    expect(fontStack("Cascadia Mono")).toBe('"Cascadia Mono", "JetBrains Mono", monospace');
    expect(fontStack("JetBrains Mono")).toBe('"JetBrains Mono", monospace');
  });
  it("strips quotes a family name can't hold", () => {
    expect(fontStack('Bad"Name')).toBe('"BadName", "JetBrains Mono", monospace');
  });
});
