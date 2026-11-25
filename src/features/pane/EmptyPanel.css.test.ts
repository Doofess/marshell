import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("./EmptyPanel.css", import.meta.url), "utf8");
/** The text of every block opened by `@<at> <query>`, braces balanced. */
const blocks = (at: string) => {
  const out: string[] = [];
  let from = 0;
  for (;;) {
    const i = css.indexOf(at, from);
    if (i < 0) return out;
    let depth = 0;
    let j = css.indexOf("{", i);
    for (; j < css.length; j++) {
      if (css[j] === "{") depth++;
      else if (css[j] === "}" && --depth === 0) break;
    }
    out.push(css.slice(i, j + 1));
    from = j + 1;
  }
};
/** The text from `selector {` to the next `}`. */
const rule = (selector: string) => {
  const i = css.indexOf(`${selector} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};
const without =(parts: string[]) => parts.reduce((text, part) => text.replace(part, ""), css);
const SYSTEM = ["Canvas", "CanvasText", "ButtonFace", "ButtonText", "Highlight", "HighlightText", "GrayText", "none", "transparent"];

describe("EmptyPanel.css", () => {
  it("has no hex colour", () => {
    expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
  });
  it("names only system colours in the forced-colors blocks", () => {
    const fc = blocks("@media (forced-colors: active)");
    expect(fc.length).toBeGreaterThan(0);
    for (const block of fc) {
      expect(block).not.toMatch(/var\(/);
      const colours = [...block.matchAll(/(?:color|background|fill|stroke):\s*([^;]+);/g)].map((m) => m[1]!.trim());
      for (const c of colours) expect(SYSTEM).toContain(c);
    }
    const all = fc.join("\n");
    // The filled button is the system's own selection pair, and the plain one its button pair.
    expect(all).toMatch(/\.empty-panel__button--primary \{[^}]*background: Highlight;[^}]*color: HighlightText;/s);
    expect(all).toMatch(/\.empty-panel__button \{[^}]*background: ButtonFace;[^}]*color: ButtonText;/s);
  });
  it("puts :hover only inside (hover: hover) and (pointer: fine)", () => {
    const hover = blocks("@media (hover: hover) and (pointer: fine)").concat(blocks("@media (forced-colors: active) and (hover: hover) and (pointer: fine)"));
    expect(hover.join("\n")).toContain(":hover");
    expect(without(hover)).not.toContain(":hover");
  });
  it("tells its buttons apart by fill: no border but the reset, and an outline only for focus and forced colours", () => {
    const borders = [...css.matchAll(/\bborder(?:-[a-z-]+)?:\s*([^;]+);/g)].filter((m) => !m[0].startsWith("border-radius")).map((m) => m[1]!.trim());
    for (const b of borders) expect(b).toBe("0");
    const fc = blocks("@media (forced-colors: active)");
    const rest = without(fc);
    const rules = [...rest.matchAll(/([^{}]+)\{([^{}]*)\}/g)].filter((m) => /\boutline(?:-[a-z]+)?:/.test(m[2]!));
    expect(rules.length).toBeGreaterThan(0);
    for (const r of rules) for (const selector of r[1]!.split(",")) expect(selector).toContain(":focus-visible");
  });
  it("never removes a focus ring", () => {
    expect(css).not.toMatch(/outline:\s*(?:none|0)\b/);
  });
  it("gives up the mark and the help line at level 1 and the list at level 2, never the heading or a button", () => {
    expect(css).toMatch(/\.empty-panel\[data-level="1"\] \.empty-panel__icon,\s*\.empty-panel\[data-level="2"\] \.empty-panel__icon,\s*\.empty-panel\[data-level="1"\] \.empty-panel__line,\s*\.empty-panel\[data-level="2"\] \.empty-panel__line \{\s*display: none;/);
    expect(css).toMatch(/\.empty-panel\[data-level="2"\] \.empty-panel__group \{\s*display: none;/);
    expect(css).not.toMatch(/data-level[^{]*__(title|actions|button)[^{]*\{\s*display: none/);
    expect(css).not.toContain("@container");
  });
  it("shows no scrollbar", () => {
    expect(rule(".empty-panel")).toContain("scrollbar-width: none");
    expect(css).toContain(".empty-panel::-webkit-scrollbar {");
  });
  it("moves a pressed button only where motion is welcome", () => {
    const motion = blocks("@media (prefers-reduced-motion: no-preference)");
    expect(motion.join("\n")).toContain("transition: scale");
    expect(without(motion)).not.toMatch(/\btransition:/);
    expect(css).not.toMatch(/transition:\s*all\b/);
  });
});
