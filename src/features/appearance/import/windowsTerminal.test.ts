import { describe, expect, it } from "vitest";
import { NoReadableSchemes, parseWtSchemes, schemesInFiles, skippedNote, wtDefaultFont } from "./windowsTerminal";

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
    const { themes, skipped } = parseWtSchemes(text);
    expect(skipped).toEqual([]);
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
    const { themes } = parseWtSchemes(JSON.stringify(scheme("Solo")));
    expect(themes).toHaveLength(1);
    expect(themes[0]!.id).toBe("wt:solo");
  });

  it("gives a duplicate id a -2", () => {
    const { themes } = parseWtSchemes(JSON.stringify({ schemes: [scheme("Same"), scheme("Same")] }));
    expect(themes.map((t) => t.id)).toEqual(["wt:same", "wt:same-2"]);
  });

  it("missing brightPurple is rejected naming brightPurple", () => {
    const s = scheme("Broken");
    delete s.brightPurple;
    expect(() => parseWtSchemes(JSON.stringify(s))).toThrow(/brightPurple/);
  });

  it("one bad scheme is skipped and the others are kept", () => {
    const foo = scheme("Foo");
    delete foo.brightPurple;
    const bar = scheme("Bar", { red: "red" });
    const { themes, skipped } = parseWtSchemes(JSON.stringify({ schemes: [scheme("Good"), foo, bar, { red: "#ff0000" }] }));
    expect(themes.map((t) => t.name)).toEqual(["Good"]);
    expect(skipped).toEqual([
      { name: "Foo", reason: "is missing brightPurple" },
      { name: "Bar", reason: "has a red that isn't a colour" },
      { name: "A scheme", reason: "has no name" },
    ]);
    expect(skippedNote(skipped)).toBe("Skipped 3 schemes: Foo is missing brightPurple; Bar has a red that isn't a colour; A scheme has no name.");
    expect(skippedNote(skipped.slice(0, 1))).toBe("Skipped 1 scheme: Foo is missing brightPurple.");
  });

  it("throws when there are schemes but none can be read, with the first problem", () => {
    const foo = scheme("Foo");
    delete foo.brightPurple;
    const bar = scheme("Bar");
    delete bar.red;
    try {
      parseWtSchemes(JSON.stringify({ schemes: [foo, bar] }));
      expect.unreachable();
    } catch (e) {
      expect(e).toBeInstanceOf(NoReadableSchemes);
      expect((e as Error).message).toMatch(/brightPurple/);
      expect((e as NoReadableSchemes).skipped.map((s) => s.name)).toEqual(["Foo", "Bar"]);
    }
  });
});

describe("schemesInFiles", () => {
  it("lists the good schemes of every file and notes what was skipped", () => {
    const foo = scheme("Foo");
    delete foo.brightPurple;
    const a = JSON.stringify({ schemes: [scheme("One"), foo] });
    const b = JSON.stringify({ schemes: [scheme("Two")] });
    const out = schemesInFiles([a, b]);
    expect(out.themes.map((t) => t.name)).toEqual(["One", "Two"]);
    expect(out.note).toBe("Skipped 1 scheme: Foo is missing brightPurple.");
  });

  it("a file where nothing can be read still says which schemes were skipped", () => {
    const foo = scheme("Foo");
    delete foo.brightPurple;
    const out = schemesInFiles([JSON.stringify({ schemes: [foo] }), "{ not json", JSON.stringify({ profiles: {}, schemes: [] })]);
    expect(out.themes).toEqual([]);
    expect(out.note).toBe("Skipped 1 scheme: Foo is missing brightPurple.");
  });

  it("files with no schemes give no note", () => {
    expect(schemesInFiles(["{ not json"])).toEqual({ themes: [], note: null });
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
