import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
const css = readFileSync(new URL("./Board.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
describe("Board.css", () => {
  it("is a dark sign in both themes, with 44 px rows", () => {
    expect(css).toMatch(/\.board\s*\{[^}]*color-scheme:\s*dark/);
    expect(css).toMatch(/\.board__row\s*\{[^}]*block-size:\s*44px/);
  });
  it("writes status, waiting and context in uppercase mono with tabular figures", () => {
    expect(css).toMatch(/\.board__status[^{]*\{[^}]*text-transform:\s*uppercase/);
    expect(css).toMatch(/font-family:\s*var\(--font-mono\)/);
    expect(css).toMatch(/font-variant-numeric:\s*tabular-nums/);
  });
  it("splits a tile at half its height with a gradient hairline, not a border", () => {
    expect(css).toMatch(/linear-gradient\([^)]*50%/);
    expect(css).not.toMatch(/border(?!-radius|-inline-start)/);
  });
  it("flips only under no-preference and cross-fades under reduce", () => {
    expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\)[\s\S]*?\.flip/);
    expect(css).toMatch(/@media \(prefers-reduced-motion: no-preference\)/);
  });
  it("never scrolls", () => {
    expect(css).not.toMatch(/overflow(-x|-y)?:\s*(auto|scroll)/);
  });
  it("colours the agent name from the session's accent", () => {
    expect(css).toMatch(/\.board__agent[^{]*\{[^}]*color:\s*var\(--accent\)/);
  });
});
