import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("../../styles/app.css", import.meta.url), "utf8");
const bar = readFileSync(new URL("../window/MainWindow.css", import.meta.url), "utf8");

describe("the performance panel's place", () => {
  it("starts below the top bar, so the window buttons (and the strip that shows them on Windows) never lie over it", () => {
    const i = css.indexOf(".perf-overlay {");
    const rule = css.slice(i, css.indexOf("}", i));
    // The bar is one 40 px row; the panel keeps its own gap below it.
    expect(bar).toMatch(/\.window__bar \{[^}]*block-size: 40px/);
    expect(rule).toContain("inset-block-start: calc(40px + var(--space-2))");
  });
});
