import { describe, expect, it } from "vitest";
import { parseWtSchemes, wtDefaultFont } from "./windowsTerminal";

const ANSI = [
  "black", "red", "green", "yellow", "blue", "purple", "cyan", "white",
  "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightPurple", "brightCyan", "brightWhite",
];

/** A full WT scheme object; the colour of ANSI slot i is the hex digit i repeated six times. */
function scheme(name: string, extra: Record<string, string> = {}): Record<string, string> {
  const out: Record<string, string> = { name, foreground: "#cccccc", background: "#0c0c0c" };
  ANSI.forEach((key, i) => {
    const h = i.toString(16);
    out[key] = `#${h.repeat(6)}`;
  });
  return { ...out, ...extra };
}

describe("parseWtSchemes", () => {
  it("reads schemes from settings.json", () => {
    const text = `﻿{
      // Windows Terminal settings
      "profiles": { "defaults": {} },
      "schemes": [
        ${JSON.stringify(scheme("One Half Dark", { cursorColor: "#ffffff", selectionBackground: "#222222" }))},
        /* second */
        ${JSON.stringify(scheme("Campbell"))},
      ],
    }`;
    const themes = parseWtSchemes(text);
    expect(themes.map((t) => t.id)).toEqual(["wt:one-half-dark", "wt:campbell"]);
    expect(themes[0]).toMatchObject({
      name: "One Half Dark",
      source: "windows-terminal",
      colors: { foreground: "#cccccc", background: "#0c0c0c", cursor: "#ffffff", selection: "#222222" },
    });
    expect(themes[0]!.colors.ansi).toHaveLength(16);
    expect(themes[0]!.colors.ansi[1]).toBe("#111111");
    // Cursor and selection fall back to the foreground and mid grey.
    expect(themes[1]!.colors.cursor).toBe("#cccccc");
    expect(themes[1]!.colors.selection).toBe("#808080");
  });

  it("reads a single scheme object", () => {
    const themes = parseWtSchemes(JSON.stringify(scheme("Solo")));
    expect(themes).toHaveLength(1);
    expect(themes[0]!.id).toBe("wt:solo");
  });

  it("gives a duplicate id a -2", () => {
    const themes = parseWtSchemes(JSON.stringify({ schemes: [scheme("Same"), scheme("Same")] }));
    expect(themes.map((t) => t.id)).toEqual(["wt:same", "wt:same-2"]);
  });

  it("missing brightPurple is rejected naming brightPurple", () => {
    const s = scheme("Broken");
    delete s.brightPurple;
    expect(() => parseWtSchemes(JSON.stringify(s))).toThrow(/brightPurple/);
  });
});

describe("wtDefaultFont", () => {
  it("reads new and legacy keys", () => {
    const next = '{"profiles":{"defaults":{"font":{"face":"Cascadia Code","size":11}}}}';
    expect(wtDefaultFont(next)).toEqual({ family: "Cascadia Code", size: 11 });
    const legacy = '// c\n{"profiles":{"defaults":{"fontFace":"Consolas","fontSize":12,},},}';
    expect(wtDefaultFont(legacy)).toEqual({ family: "Consolas", size: 12 });
    expect(wtDefaultFont('{"profiles":{"defaults":{}}}')).toEqual({});
  });
});
