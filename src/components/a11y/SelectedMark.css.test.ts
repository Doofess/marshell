import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./SelectedMark.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("SelectedMark: a selected item has three cues that do not depend on each other", () => {
  it("fills the item (colour-independent of the others)", () => {
    expect(css).toMatch(/\.is-selected-mark\[aria-selected="true"\][\s\S]*?box-shadow:\s*inset 0 0 0 100vmax var\(--selected\)/);
  });
  it("marks the end edge with a 2px bar in the accent (a shape, not a colour, so it survives colour blindness)", () => {
    expect(css).toMatch(/\.is-selected-mark\[aria-current="true"\]::before[\s\S]*?inline-size:\s*2px/);
    expect(css).toMatch(/inset-inline-end:\s*0/);
  });
  it("covers aria-selected, aria-current and data-selected", () => {
    for (const a of ['[aria-selected="true"]', '[aria-current="true"]', '[data-selected="true"]']) expect(css, a).toContain(a);
  });
  it("keeps a Highlight outline in forced colours, where fills and shadows are dropped", () => {
    const block = css.slice(css.indexOf("@media (forced-colors: active)"));
    expect(block).toMatch(/outline:\s*2px solid Highlight/);
    expect(block).not.toMatch(/border[a-z-]*color/);
  });
  it("uses logical properties only", () => {
    expect(css).not.toMatch(/\b(left|right|top|bottom)\s*:/);
  });
});
