import { describe, expect, it } from "vitest";
import type { Appearance } from "../../generated/Appearance";
import { DEFAULT_APPEARANCE, contrastMoreOf, rootState } from "./rootState";

const a = (over: Partial<Appearance>): Appearance => ({ ...DEFAULT_APPEARANCE, ...over });
const off = { prefersContrastMore: false };

describe("rootState", () => {
  it("follow-system theme removes data-theme", () => {
    expect(rootState(a({ theme: "follow-system" }), off).attrs["data-theme"]).toBeNull();
  });
  it("a fixed theme sets data-theme", () => {
    expect(rootState(a({ theme: "dark" }), off).attrs["data-theme"]).toBe("dark");
    expect(rootState(a({ theme: "light" }), off).attrs["data-theme"]).toBe("light");
  });
  it("sets no accent: the root carries only the theme, contrast and text size", () => {
    const r = rootState(DEFAULT_APPEARANCE, off);
    expect(Object.keys(r.attrs).sort()).toEqual(["data-contrast", "data-text-size", "data-theme"]);
    expect(r).not.toHaveProperty("style");
  });
  it("contrast on sets data-contrast", () => {
    expect(rootState(a({ contrast: "on" }), off).attrs["data-contrast"]).toBe("more");
  });
  it("contrast off ignores the OS", () => {
    expect(rootState(a({ contrast: "off" }), { prefersContrastMore: true }).attrs["data-contrast"]).toBeNull();
  });
  it("follow-system contrast is on when the OS asks", () => {
    expect(rootState(a({ contrast: "follow-system" }), { prefersContrastMore: true }).attrs["data-contrast"]).toBe("more");
  });
  it("follow-system contrast is off when the OS does not ask", () => {
    expect(rootState(a({ contrast: "follow-system" }), off).attrs["data-contrast"]).toBeNull();
  });
  it("text size", () => {
    expect(rootState(a({ text_size: "default" }), off).attrs["data-text-size"]).toBeNull();
    expect(rootState(a({ text_size: "large" }), off).attrs["data-text-size"]).toBe("large");
    expect(rootState(a({ text_size: "larger" }), off).attrs["data-text-size"]).toBe("larger");
  });
});

describe("contrastMoreOf", () => {
  it("is on by the setting, whatever the system asks", () => {
    expect(contrastMoreOf("on", false)).toBe(true);
  });
  it("follows the system only when the setting says to", () => {
    expect(contrastMoreOf("follow-system", true)).toBe(true);
    expect(contrastMoreOf("follow-system", false)).toBe(false);
  });
  it("is off by the setting, whatever the system asks", () => {
    expect(contrastMoreOf("off", true)).toBe(false);
  });
});
