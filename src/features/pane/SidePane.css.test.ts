import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("./SidePane.css", import.meta.url), "utf8");
const rule = (selector: string) => {
  const i = css.indexOf(`${selector} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};
/** The text of the block opened by the first `@media <query>`, braces balanced. */
const media = (query: string, from = 0) => {
  const i = css.indexOf(`@media ${query}`, from);
  if (i < 0) return "";
  let depth = 0;
  for (let j = css.indexOf("{", i); j < css.length; j++) {
    if (css[j] === "{") depth++;
    else if (css[j] === "}" && --depth === 0) return css.slice(i, j + 1);
  }
  return "";
};

describe("SidePane.css", () => {
  it("has no hex colour", () => {
    expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
  });
  it("names only system colours in the forced-colors block", () => {
    const fc = media("(forced-colors: active)");
    expect(fc).not.toBe("");
    expect(fc).toMatch(/Highlight/);
    expect(fc).not.toMatch(/var\(/);
    const colours = [...fc.matchAll(/(?:color|background):\s*([^;]+);/g)].map((m) => m[1]!.trim());
    for (const c of colours) expect(["Highlight", "HighlightText", "CanvasText", "ButtonText"]).toContain(c);
  });
  it("puts :hover only inside (hover: hover) and (pointer: fine)", () => {
    const hover = media("(hover: hover) and (pointer: fine)");
    const outside = css.replace(hover, "");
    expect(hover).toContain(":hover");
    expect(outside).not.toContain(":hover");
  });
  it("keeps the selected tab's title readable in forced colours: no backplate under HighlightText", () => {
    // Without this the system paints a Canvas backplate behind the title, and HighlightText on Canvas can be white on white.
    const fc = media("(forced-colors: active)");
    const i = fc.indexOf('.side-pane__tab[aria-selected="true"] {');
    const selected = fc.slice(i, fc.indexOf("}", i));
    expect(selected).toContain("background: Highlight");
    expect(selected).toContain("color: HighlightText");
    expect(selected).toContain("forced-color-adjust: none");
  });
  it("rings a focused selected tab, and its close button, in HighlightText: CanvasText on Highlight may not show", () => {
    const fc = media("(forced-colors: active)");
    const i = fc.indexOf('.side-pane__tab[aria-selected="true"]:focus-visible,');
    expect(i).toBeGreaterThan(-1);
    const ring = fc.slice(i, fc.indexOf("}", i));
    expect(ring).toContain('.side-pane__tab-wrap[data-selected="true"] .side-pane__close:focus-visible {');
    expect(ring).toContain("outline-color: HighlightText");
    // The tab's ring is set in from the edge, so the fill shows on both sides of it: at the edge it would run into a Canvas of its own colour.
    const j = fc.indexOf('.side-pane__tab[aria-selected="true"]:focus-visible {');
    expect(j).toBeGreaterThan(-1);
    expect(fc.slice(j, fc.indexOf("}", j))).toContain("outline-offset: -4px");
  });
  it("hides the strip's scrollbar and lets it scroll sideways", () => {
    expect(rule(".side-pane__tabs")).toContain("scrollbar-width: none");
    expect(rule(".side-pane__tabs")).toContain("overflow: auto clip");
  });
  it("sizes a tab from 96 to 200 px and cuts its title with an ellipsis", () => {
    expect(rule(".side-pane__tab-wrap")).toContain("min-inline-size: 96px");
    expect(rule(".side-pane__tab-wrap")).toContain("200px");
    expect(rule(".side-pane__title")).toContain("text-overflow: ellipsis");
  });
});
