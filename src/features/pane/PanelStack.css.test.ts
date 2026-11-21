import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("./PanelStack.css", import.meta.url), "utf8");
const rule = (selector: string) => {
  const i = css.indexOf(`${selector} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};
/** The text of the block opened by the first `@media <query>`, braces balanced. */
const media = (query: string) => {
  const i = css.indexOf(`@media ${query}`);
  if (i < 0) return "";
  let depth = 0;
  for (let j = css.indexOf("{", i); j < css.length; j++) {
    if (css[j] === "{") depth++;
    else if (css[j] === "}" && --depth === 0) return css.slice(i, j + 1);
  }
  return "";
};

describe("PanelStack.css", () => {
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
    const forcedHover = media("(forced-colors: active) and (hover: hover) and (pointer: fine)");
    expect(hover).toContain(":hover");
    expect(css.replace(hover, "").replace(forcedHover, "")).not.toContain(":hover");
  });
  it("makes the divider a row-resize handle a drag cannot scroll", () => {
    expect(rule(".panel-stack__divider")).toContain("cursor: row-resize");
    expect(rule(".panel-stack__divider")).toContain("touch-action: none");
  });
  it("keeps keyboard focus visible on the divider", () => {
    expect(rule(".panel-stack__divider:focus-visible")).toContain("outline:");
  });
  it("cuts the root and every row with overflow: clip, so a squeezed panel never spills into its neighbour", () => {
    expect(rule(".panel-stack")).toContain("overflow: clip");
    expect(rule(".panel-stack__row")).toContain("overflow: clip");
  });
  it("lets a row shrink to nothing and its content fill it", () => {
    expect(rule(".panel-stack__row")).toContain("min-block-size: 0");
    expect(rule(".panel-stack__row > *")).toContain("block-size: 100%");
  });
});
