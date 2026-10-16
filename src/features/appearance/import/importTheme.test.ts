import { describe, expect, it } from "vitest";
import { normalizeColor } from "./colors";
import { importThemeFile } from "./importTheme";

const WT_KEYS = [
  "black", "red", "green", "yellow", "blue", "purple", "cyan", "white",
  "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightPurple", "brightCyan", "brightWhite",
];

function wtScheme(color: string): string {
  const s: Record<string, string> = { name: "Short", foreground: color, background: color };
  for (const k of WT_KEYS) s[k] = color;
  return JSON.stringify(s);
}

describe("importThemeFile", () => {
  it("rejects files over 1 MB before parsing", () => {
    const r = importThemeFile("big.json", " ".repeat(1_000_001));
    expect(r).toEqual({ ok: false, reason: "This file is too big to be a theme (over 1 MB)." });
  });

  it("rejects unknown extensions with the exact copy", () => {
    const reason = "Marshell can read Windows Terminal .json, iTerm2 .itermcolors and VS Code theme .json files.";
    expect(importThemeFile("theme.txt", "{}")).toEqual({ ok: false, reason });
    // A .json that is neither a WT nor a VS Code theme.
    expect(importThemeFile("other.json", '{"a":1}')).toEqual({ ok: false, reason });
  });

  it("normalizes #abc and #aabbccdd", () => {
    expect(normalizeColor("#abc")).toBe("#aabbcc");
    expect(normalizeColor("#AABBCCDD")).toBe("#aabbcc");
    expect(normalizeColor("#aabbcc")).toBe("#aabbcc");
    expect(normalizeColor("blue")).toBeNull();
    expect(normalizeColor("#abcd1")).toBeNull();
    const r = importThemeFile("s.json", wtScheme("#abc"));
    expect(r.ok && r.themes[0]!.colors.ansi[0]).toBe("#aabbcc");
  });

  it("sends a WT settings file to the WT parser", () => {
    const r = importThemeFile("settings.json", `{"schemes":[${wtScheme("#111111")}]}`);
    expect(r.ok && r.themes.map((t) => t.id)).toEqual(["wt:short"]);
  });

  it("sends a VS Code theme to the VS Code parser", () => {
    const r = importThemeFile("t.json", '{"name":"Plain","colors":{}}');
    expect(r).toEqual({ ok: false, reason: "This VS Code theme has no terminal colours." });
  });

  it("returns a parse error as a reason, not a throw", () => {
    const r = importThemeFile("bad.json", "{nope");
    expect(!r.ok && r.reason).toMatch(/^Not valid JSON: /);
  });

  it("sends .itermcolors to the iTerm parser", () => {
    const r = importThemeFile("x.itermcolors", "<plist><dict></dict></plist>");
    expect(!r.ok && r.reason).toMatch(/Ansi 0 Color/);
  });
});
