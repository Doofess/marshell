import { describe, expect, it } from "vitest";
import { parseVsCodeTheme } from "./vscode";

const NAMES = ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White"];

/** terminal.ansi* colours; slot i is the digit i repeated six times (bright slots 8 to 15 use 8 and up). */
function ansiColors(): Record<string, string> {
  const out: Record<string, string> = {};
  NAMES.forEach((n, i) => {
    out[`terminal.ansi${n}`] = `#${i.toString(16).repeat(6)}`;
    out[`terminal.ansiBright${n}`] = `#${(i + 8).toString(16).repeat(6)}`;
  });
  return out;
}

describe("parseVsCodeTheme", () => {
  it("reads terminal colours", () => {
    const text = JSON.stringify({
      name: "Night Owl",
      colors: {
        ...ansiColors(),
        "terminal.foreground": "#d6deeb",
        "terminal.background": "#011627",
        "terminalCursor.foreground": "#80a4c2",
        "terminal.selectionBackground": "#1d3b53",
      },
    });
    const t = parseVsCodeTheme(text, "night.json");
    expect(t).toMatchObject({
      id: "vscode:night-owl",
      name: "Night Owl",
      source: "vscode",
      colors: { foreground: "#d6deeb", background: "#011627", cursor: "#80a4c2", selection: "#1d3b53" },
    });
    expect(t.colors.ansi).toHaveLength(16);
    expect(t.colors.ansi[5]).toBe("#555555");
    expect(t.colors.ansi[13]).toBe("#dddddd");
  });

  it("falls back to editor colours for fg/bg", () => {
    const text = `// theme
    {"colors": ${JSON.stringify({ ...ansiColors(), "editor.foreground": "#abc", "editor.background": "#112233ff" })},}`;
    const t = parseVsCodeTheme(text, "my-theme.json");
    expect(t.colors.foreground).toBe("#aabbcc");
    expect(t.colors.background).toBe("#112233");
    expect(t.colors.cursor).toBe("#aabbcc");
    expect(t.colors.selection).toBe("#808080");
    // No name in the file: the file name without its extension.
    expect(t.name).toBe("my-theme");
    expect(t.id).toBe("vscode:my-theme");
  });

  it("no terminal colours gives the exact reason", () => {
    const text = JSON.stringify({ name: "Plain", colors: { "editor.foreground": "#fff", "editor.background": "#000" } });
    expect(() => parseVsCodeTheme(text, "plain.json")).toThrow("This VS Code theme has no terminal colours.");
  });

  it("names the missing key when only some ANSI colours are set", () => {
    const colors = ansiColors();
    delete colors["terminal.ansiBrightMagenta"];
    const text = JSON.stringify({ colors: { ...colors, "terminal.foreground": "#fff", "terminal.background": "#000" } });
    expect(() => parseVsCodeTheme(text, "x.json")).toThrow(/terminal\.ansiBrightMagenta/);
  });
});
