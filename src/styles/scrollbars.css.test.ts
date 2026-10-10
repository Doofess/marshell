import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { PALETTES } from "../features/terminal/palettes";

const css = readFileSync(new URL("./scrollbars.css", import.meta.url), "utf8");
const block = (sel: string) => {
  const i = css.indexOf(`${sel} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("scrollbars.css", () => {
  it("draws a quiet pill thumb on a transparent track, with room around it", () => {
    expect(block("*::-webkit-scrollbar")).toMatch(/inline-size|width:\s*12px/);
    expect(block("*::-webkit-scrollbar-track")).toContain("background: transparent");
    const thumb = block("*::-webkit-scrollbar-thumb");
    expect(thumb).toContain("border-radius: 999px");
    expect(thumb).toContain("background-clip: content-box");
    expect(thumb).toMatch(/border:\s*3px solid transparent/);
  });
  it("hides the arrow buttons and the corner", () => {
    expect(block("*::-webkit-scrollbar-button")).toContain("display: none");
    expect(block("*::-webkit-scrollbar-corner")).toContain("background: transparent");
  });
  it("strengthens the thumb on hover and while dragging", () => {
    expect(css).toContain("*::-webkit-scrollbar-thumb:hover");
    expect(css).toContain("*::-webkit-scrollbar-thumb:active");
  });
  it("takes its colours from the theme, so both schemes and forced colours follow", () => {
    expect(css).toContain("var(--text-1)");
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).toContain("forced-colors: active");
  });
  it("falls back to the standard thin scrollbar only where the pill cannot be drawn (Chromium ignores the pill once scrollbar-color is set)", () => {
    expect(css).toMatch(/@supports not selector\(::-webkit-scrollbar\)\s*\{[^@]*scrollbar-width:\s*thin[^@]*scrollbar-color/);
  });
});

describe("terminal scrollbar", () => {
  for (const scheme of ["dark", "light"] as const)
    it(`styles xterm's own scrollbar in the ${scheme} palette, stronger on hover and drag`, () => {
      const p = PALETTES[scheme];
      for (const k of ["scrollbarSliderBackground", "scrollbarSliderHoverBackground", "scrollbarSliderActiveBackground"] as const)
        expect(p[k], k).toMatch(/^#[0-9a-f]{8}$/);
      const alpha = (c: string) => parseInt(c.slice(7), 16);
      expect(alpha(p.scrollbarSliderHoverBackground!)).toBeGreaterThan(alpha(p.scrollbarSliderBackground!));
      expect(alpha(p.scrollbarSliderActiveBackground!)).toBeGreaterThan(alpha(p.scrollbarSliderHoverBackground!));
    });
});
