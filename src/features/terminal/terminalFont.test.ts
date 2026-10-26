import { describe, expect, it } from "vitest";
import { familyOf, fontChoiceOf, fontStack, resolveTerminalFont, type FontChoice } from "./terminalFont";

describe("fontChoiceOf", () => {
  it("empty means match", () => {
    expect(fontChoiceOf("")).toEqual({ kind: "match" });
    expect(fontChoiceOf("  ")).toEqual({ kind: "match" });
    expect(fontChoiceOf(null)).toEqual({ kind: "match" });
    expect(fontChoiceOf(undefined)).toEqual({ kind: "match" });
  });
  it("bundled names match ignoring case and spaces", () => {
    expect(fontChoiceOf("cascadia mono ")).toEqual({ kind: "bundled", family: "Cascadia Mono" });
    expect(fontChoiceOf("JETBRAINS MONO")).toEqual({ kind: "bundled", family: "JetBrains Mono" });
  });
  it("anything else is another font", () => {
    expect(fontChoiceOf("FiraCode Nerd Font")).toEqual({ kind: "other", family: "FiraCode Nerd Font" });
  });
  it("familyOf round-trips", () => {
    const choices: FontChoice[] = [
      { kind: "match" },
      { kind: "bundled", family: "JetBrains Mono" },
      { kind: "bundled", family: "Cascadia Mono" },
      { kind: "bundled", family: "Atkinson Hyperlegible Mono" },
      { kind: "other", family: "FiraCode Nerd Font" },
    ];
    for (const c of choices) expect(fontChoiceOf(familyOf(c))).toEqual(c);
  });
});

describe("fontStack with a Windows Terminal family", () => {
  it("appends it after monospace", () => {
    expect(fontStack("Cascadia Mono", "CaskaydiaCove NF")).toBe('"Cascadia Mono", "JetBrains Mono", monospace, "CaskaydiaCove NF"');
    expect(fontStack("JetBrains Mono")).toBe('"JetBrains Mono", monospace');
  });
  it("strips quotes and backslashes in both names", () => {
    expect(fontStack('A\\"b', 'C"\\d')).toBe('"Ab", "JetBrains Mono", monospace, "Cd"');
  });
  it("does not repeat the chosen family", () => {
    expect(fontStack("Cascadia Mono", "cascadia mono")).toBe('"Cascadia Mono", "JetBrains Mono", monospace');
    expect(fontStack("JetBrains Mono", "JetBrains Mono")).toBe('"JetBrains Mono", monospace');
  });
});

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
