import { describe, expect, it } from "vitest";
import type { Appearance } from "../../generated/Appearance";
import { DEFAULT_APPEARANCE, rootState } from "./rootState";

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
  it("a preset accent sets data-accent and clears the inline accent", () => {
    const r = rootState(a({ accent: "teal" }), off);
    expect(r.attrs["data-accent"]).toBe("teal");
    expect(r.style["--accent"]).toBeNull();
    expect(r.style["--accent-ink"]).toBeNull();
  });
  it("a custom accent sets the hex and its ink inline", () => {
    const r = rootState(a({ accent: "#DB2777" }), off);
    expect(r.attrs["data-accent"]).toBe("custom");
    expect(r.style["--accent"]).toBe("#db2777");
    expect(r.style["--accent-ink"]).toBe("#ffffff");
  });
  it("a custom accent that fails the check falls back to the default preset", () => {
    const r = rootState(a({ accent: "#ffffff" }), off);
    expect(r.attrs["data-accent"]).toBe("amber");
    expect(r.style["--accent"]).toBeNull();
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
