import { describe, expect, it } from "vitest";
import { MIN_CONTRAST, PALETTES } from "./palettes";
import { resolveScheme, resolveTerminalTheme, schemeFromComputed, schemeOfBackground, terminalOptions, themeFromColors } from "./terminalTheme";

describe("resolveScheme", () => {
  it("follows the app by default", () => {
    expect(resolveScheme("follow-app", "dark")).toBe("dark");
    expect(resolveScheme("follow-app", "light")).toBe("light");
  });
  it("can be fixed either way regardless of the app", () => {
    expect(resolveScheme("dark", "light")).toBe("dark");
    expect(resolveScheme("light", "dark")).toBe("light");
  });
});

describe("schemeFromComputed", () => {
  it("reads a forced scheme", () => {
    expect(schemeFromComputed("dark", false)).toBe("dark");
    expect(schemeFromComputed("light", true)).toBe("light");
  });
  it("falls back to the OS preference for 'light dark' or nothing", () => {
    expect(schemeFromComputed("light dark", true)).toBe("dark");
    expect(schemeFromComputed("light dark", false)).toBe("light");
    expect(schemeFromComputed("", true)).toBe("dark");
    expect(schemeFromComputed("normal", false)).toBe("light");
  });
});

describe("terminalOptions", () => {
  it("returns the palette with contrast protection on", () => {
    const o = terminalOptions("light");
    expect(o.theme).toBe(PALETTES.light);
    expect(o.minimumContrastRatio).toBe(MIN_CONTRAST);
  });
});

const SOLAR = {
  id: "wt:solarized-light",
  colors: {
    foreground: "#657b83",
    background: "#fdf6e3",
    cursor: "#586e75",
    selection: "#eee8d5",
    ansi: Array.from({ length: 16 }, (_, i) => `#0000${i.toString(16).padStart(2, "0")}`),
  },
};
const base = { setting: "follow-app", imported: [SOLAR], app: "dark" as const, protection: true };

describe("themeFromColors", () => {
  it("maps the 16 colours in xterm order", () => {
    const t = themeFromColors(SOLAR.colors);
    expect(t.black).toBe("#000000");
    expect(t.red).toBe("#000001");
    expect(t.brightRed).toBe("#000009");
    expect(t.brightWhite).toBe("#00000f");
    expect(t.selectionBackground).toBe("#eee8d5");
    expect(t.cursor).toBe("#586e75");
    expect(t.cursorAccent).toBe("#fdf6e3");
  });
  it("keeps the scrollbar hidden", () => {
    expect(themeFromColors(SOLAR.colors).scrollbarSliderBackground).toBe("#00000000");
  });
});

describe("schemeOfBackground", () => {
  it("reads dark and light backgrounds", () => {
    expect(schemeOfBackground("#002b36")).toBe("dark");
    expect(schemeOfBackground("#fdf6e3")).toBe("light");
  });
});

describe("resolveTerminalTheme", () => {
  it("follow-app takes the app scheme", () => {
    const r = resolveTerminalTheme(base);
    expect(r.scheme).toBe("dark");
    expect(r.theme).toBe(PALETTES.dark);
  });
  it("an imported theme resolves and reports its scheme", () => {
    const r = resolveTerminalTheme({ ...base, setting: SOLAR.id });
    expect(r.scheme).toBe("light");
    expect(r.theme.background).toBe("#fdf6e3");
  });
  it("the profile override wins over the setting", () => {
    expect(resolveTerminalTheme({ ...base, setting: SOLAR.id, override: "dark" }).theme).toBe(PALETTES.dark);
  });
  it("override_naming_missing_theme_falls_back to the setting", () => {
    expect(resolveTerminalTheme({ ...base, setting: "light", override: "wt:gone" }).theme).toBe(PALETTES.light);
  });
  it("a setting naming a missing theme falls back to follow-app", () => {
    expect(resolveTerminalTheme({ ...base, setting: "wt:gone", app: "light" }).theme).toBe(PALETTES.light);
  });
  it("contrast protection off gives ratio 1", () => {
    expect(resolveTerminalTheme({ ...base, protection: false }).minimumContrastRatio).toBe(1);
    expect(resolveTerminalTheme(base).minimumContrastRatio).toBe(MIN_CONTRAST);
  });
});
