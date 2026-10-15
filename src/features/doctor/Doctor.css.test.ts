import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Doctor.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("Doctor.css", () => {
  it("draws no box: the only borders are radius and a start-edge bar", () => {
    for (const m of css.matchAll(/border[a-z-]*\s*:[^;]+;/g)) expect(m[0], m[0]).toMatch(/border-radius|border-inline-start/);
  });
  it("tells severity by a fill and a start-edge bar, and again with an outline in forced colours", () => {
    expect(css).toMatch(/\.doctor__row\s*\{[^}]*background:\s*var\(--fill\)/);
    expect(css).toMatch(/\.doctor__row\[data-severity="error"\]\s*\{[^}]*border-inline-start/);
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    expect(forced).toMatch(/\.doctor__row/);
  });
  it("shows a fixed row with a tick in words (the Fixed label), not by colour alone", () => {
    expect(css).toMatch(/\.doctor__row\[data-state="fixed"\]/);
  });
  it("floats the toast on a pill shadow, enters it only with motion allowed and fades in 80 ms for reduced motion", () => {
    expect(css).toMatch(/\.toast\s*\{[^}]*box-shadow:\s*var\(--shadow-pill\)/);
    expect(css).toMatch(/@media \(prefers-reduced-motion: no-preference\)[\s\S]*?\.toast/);
    expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\)[\s\S]*?80ms/);
  });
  it("scrolls nothing except the fix sheet's body (the page body does the rest; see dialogBodies.test.ts)", () => {
    expect([...css.matchAll(/overflow(-x|-y)?:\s*(auto|scroll)/g)]).toHaveLength(1);
    expect(css).toMatch(/\.fix-sheet__body\s*\{[^}]*overflow-y:\s*auto/);
  });
});
