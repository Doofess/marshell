import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Peek.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("Peek.css", () => {
  it("fades for 120 ms with a 2 px rise, and only fades under reduced motion", () => {
    expect(css).toContain("var(--dur-peek)");
    expect(css).toMatch(/translateY\(2px\)/);
    const reduced = css.slice(css.indexOf("@media (prefers-reduced-motion: reduce)"));
    expect(reduced).toContain(".peek");
    expect(reduced).not.toContain("translate");
    expect(css).toContain(':root[data-motion="reduce"]');
  });
  it("draws a 1 px CanvasText border in forced colors", () => {
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    expect(forced).toMatch(/1px solid CanvasText/);
  });
  it("uses the terminal font and tokens, logical properties, and keeps a focus outline", () => {
    expect(css).toContain("var(--font-mono)");
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).toContain(".peek__screen:focus-visible");
    expect(css).not.toMatch(/outline:\s*none/);
  });
});
