import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./LayoutMenu.css", import.meta.url), "utf8");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("LayoutMenu.css: the Automatic checkbox", () => {
  it("draws the tick with a mask in the accent's own ink, so it reads on every vendor colour in both themes", () => {
    const tick = rule('.layout-menu__auto[aria-checked="true"] .layout-menu__box::after');
    expect(tick).toContain("mask-image");
    expect(tick).toContain("var(--accent-ink)");
    expect(css).not.toMatch(/black|stroke='#|stroke=.#000/i);
  });
  it("fills the checked box with the accent", () => {
    expect(rule('.layout-menu__auto[aria-checked="true"] .layout-menu__box')).toContain("var(--accent)");
  });
  it("gives the empty box a 3:1 edge (--text-3), as its own shape rather than a control outline", () => {
    const box = rule(".layout-menu__box");
    expect(box).toMatch(/box-shadow:\s*inset 0 0 0 1\.5px var\(--text-3\)/);
    expect(css).toMatch(/\/\*[^*]*checkbox[^*]*(exception|its own)[^*]*\*\//i);
  });
});
