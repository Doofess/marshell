import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("./FileTree.css", import.meta.url), "utf8");
const SYSTEM = ["Canvas", "CanvasText", "ButtonFace", "ButtonText", "Highlight", "HighlightText", "GrayText", "none", "transparent"];

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
const rule = (sel: string, text = css) => {
  const i = text.indexOf(`${sel} {`);
  return i < 0 ? "" : text.slice(i, text.indexOf("}", i));
};
const forced = () => blocks("@media (forced-colors: active)").join("\n");

describe("FileTree.css", () => {
  it("has no hex colour", () => {
    expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
  });
  it("rows are 24 px and the tree draws no scrollbar", () => {
    expect(rule(".file-tree__row")).toContain("block-size: 24px");
    const tree = rule(".file-tree");
    expect(tree).toContain("scrollbar-width: none");
    expect(tree).toContain("overflow-y: auto");
    expect(css).toContain(".file-tree::-webkit-scrollbar");
  });
  it("indents 12 px a level", () => {
    expect(css).toMatch(/--level\)\s*\*\s*12px/);
  });
  it("names only system colours in the forced-colors blocks", () => {
    const fc = blocks("@media (forced-colors: active)");
    expect(fc.length).toBeGreaterThan(0);
    for (const block of fc) {
      expect(block).not.toMatch(/var\(/);
      const colours = [...block.matchAll(/(?:color|background|fill|stroke|outline-color):\s*([^;]+);/g)].map((m) => m[1]!.trim());
      for (const c of colours) expect(SYSTEM).toContain(c);
    }
  });
  it("the filter box has an edge in forced colours", () => {
    expect(rule(".file-tree-filter", forced())).toContain("outline: 1px solid CanvasText");
  });
  it("forced colors give the selected row Highlight and HighlightText", () => {
    const selected = rule('.file-tree__row[aria-selected="true"]', forced());
    expect(selected).toContain("background: Highlight");
    expect(selected).toContain("color: HighlightText");
    expect(selected).toContain("forced-color-adjust: none");
  });
  it("forced colors draw an ignored row in GrayText", () => {
    const ignored = rule('.file-tree__row[data-ignored="true"]', forced());
    expect(ignored).toContain("color: GrayText");
  });
  it("a selected row stays HighlightText even when ignored", () => {
    expect(forced()).toContain('.file-tree__row[aria-selected="true"][data-ignored="true"]');
  });
  it("keeps keyboard focus visible with an outline", () => {
    expect(rule(".file-tree__row:focus-visible")).toContain("outline: 2px solid");
    expect(css).not.toMatch(/outline:\s*none/);
  });
  it("puts hover inside a hover media query", () => {
    const hover = blocks("@media (hover: hover) and (pointer: fine)").join("\n");
    expect(hover).toContain(":hover");
    expect(css.replace(hover, "")).not.toContain(":hover");
  });
  it("moves only inside prefers-reduced-motion: no-preference", () => {
    const motion = blocks("@media (prefers-reduced-motion: no-preference)").join("\n");
    expect(css.replace(motion, "")).not.toMatch(/transition:/);
    expect(css).not.toMatch(/transition:\s*all/);
  });
  it("shortens a name in the middle: the head with an ellipsis, the tail never", () => {
    expect(rule(".file-tree__head")).toContain("text-overflow: ellipsis");
    expect(rule(".file-tree__tail")).toContain("flex: none");
  });
});
