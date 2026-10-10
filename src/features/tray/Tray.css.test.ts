import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Tray.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("Tray.css", () => {
  it("is a raised menu on the overlay surface with the menu shadow, and no border", () => {
    expect(css).toMatch(/\.tray-menu\s*\{[^}]*background:\s*var\(--bg-overlay\)/);
    expect(css).toMatch(/\.tray-menu\s*\{[^}]*box-shadow:\s*var\(--shadow-menu\)/);
    expect(css).not.toMatch(/border(?!-radius)/);
  });
  it("fills an item on hover only for a real pointer, and dims a disabled item with the third text colour", () => {
    expect(css).toMatch(/@media \(hover: hover\) and \(pointer: fine\)\s*\{[^}]*\.tray-menu__item:hover[^}]*var\(--hover\)/);
    expect(css).toMatch(/\.tray-menu__item\[aria-disabled="true"\][^{]*\{[^}]*color:\s*var\(--text-3\)/);
  });
  it("draws a separator as a 1 px fill, not a border", () => {
    expect(css).toMatch(/\.tray-menu__sep\s*\{[^}]*block-size:\s*1px/);
  });
  it("keeps an outline in forced colours", () => {
    expect(css).toMatch(/@media \(forced-colors: active\)[\s\S]*?\.tray-menu[^{]*\{[^}]*outline:\s*1px solid CanvasText/);
  });
  it("shows the icon on a light and a dark panel with the same raised fill", () => {
    expect(css).toMatch(/\.tray-panel\[data-panel="light"\]\s*\{[^}]*color-scheme:\s*light/);
    expect(css).toMatch(/\.tray-panel\[data-panel="dark"\]\s*\{[^}]*color-scheme:\s*dark/);
    expect(css).toMatch(/\.tray-panel\s*\{[^}]*background:\s*var\(--bg-raised\)/);
  });
  it("never scrolls", () => {
    expect(css).not.toMatch(/overflow(-x|-y)?:\s*(auto|scroll)/);
  });
});
