import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Onboarding.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("Onboarding.css", () => {
  it("draws no 1 px box: the only border is a start-edge bar", () => {
    for (const m of css.matchAll(/border[a-z-]*\s*:[^;]+;/g)) expect(m[0], m[0]).toMatch(/border-radius|border-inline-start/);
  });
  it("tells a consent card from the page with a fill", () => {
    expect(css).toMatch(/\.consent\s*\{[^}]*background:\s*var\(--fill\)/);
  });
  it("marks a blocked card with a start-edge bar and a forced-colours outline", () => {
    expect(css).toMatch(/\.consent\[data-blocked\]\s*\{[^}]*border-inline-start/);
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    expect(forced).toMatch(/\.consent\[data-blocked\]/);
  });
  it("shows paths whole, in mono, wrapping at any point", () => {
    expect(css).toMatch(/font-family:\s*var\(--font-mono\)/);
    expect(css).toMatch(/overflow-wrap:\s*anywhere/);
  });
  it("scrolls only its body, so the buttons are always reachable (see dialogBodies.test.ts)", () => {
    expect(css).toMatch(/\.onboarding__body\s*\{[^}]*overflow-y:\s*auto/);
    expect([...css.matchAll(/overflow(-x|-y)?:\s*(auto|scroll)/g)]).toHaveLength(1);
  });
  it("sits in a row as tall as the window, so its max height holds and the body scrolls (live check 6a)", () => {
    const app = readFileSync(new URL("../../styles/app.css", import.meta.url), "utf8");
    const i = app.indexOf(".live-app__onboarding {");
    expect(app.slice(i, app.indexOf("}", i))).toMatch(/grid-template-rows:\s*minmax\(0, 1fr\)/);
    expect(css).toMatch(/\.onboarding\s*\{[^}]*max-block-size:\s*100%/);
  });
});
