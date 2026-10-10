import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Page.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("Page.css", () => {
  it("scrolls the body only, and keeps its scrollbar visible (the drawer's exception, not the window's)", () => {
    expect(css).toMatch(/\.page__body\s*\{[^}]*overflow-y:\s*auto/);
    expect(css).not.toMatch(/scrollbar-width:\s*none|::-webkit-scrollbar\s*\{\s*display:\s*none/);
    expect(css).toMatch(/scrollbar-gutter:\s*stable/);
  });
  it("fills the space it is given instead of growing the window", () => {
    expect(css).toMatch(/\.page\s*\{[^}]*(block-size|min-block-size):\s*0|grid-template-rows:\s*auto\s+minmax\(0,\s*1fr\)/);
  });
});
