import { describe, expect, it } from "vitest";
import { MIN_CONTRAST, PALETTES } from "./palettes";
import { resolveScheme, schemeFromComputed, terminalOptions } from "./terminalTheme";

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
