import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./OsBanner.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("OsBanner.css", () => {
  it("is a raised fill with the menu shadow and no border", () => {
    expect(css).toMatch(/\.os-banner\s*\{[^}]*background:\s*var\(--bg-overlay\)/);
    expect(css).toMatch(/\.os-banner\s*\{[^}]*box-shadow:\s*var\(--shadow-menu\)/);
    expect(css).not.toMatch(/border(?!-radius)/);
  });
  it("rounds more on macOS than on Windows", () => {
    const radius = (sel: string) => Number(new RegExp(`${sel}\\s*\\{[^}]*border-radius:\\s*(\\d+)px`).exec(css)?.[1]);
    expect(radius('\\.os-banner\\[data-os="mac"\\]')).toBeGreaterThan(radius('\\.os-banner\\[data-os="windows"\\]'));
  });
  it("keeps an outline in forced colours", () => {
    expect(css).toMatch(/@media \(forced-colors: active\)[\s\S]*?\.os-banner[^{]*\{[^}]*outline:\s*1px solid CanvasText/);
  });
  it("wraps long words instead of scrolling", () => {
    expect(css).toMatch(/overflow-wrap:\s*anywhere/);
    expect(css).not.toMatch(/overflow(-x|-y)?:\s*(auto|scroll)/);
  });
});
