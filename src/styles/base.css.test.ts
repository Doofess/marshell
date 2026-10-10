import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./base.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("base.css", () => {
  it("strips the browser's raised button border, so a button is told apart by its fill (the no-boxed-outlines rule) unless a class says otherwise", () => {
    expect(css).toMatch(/(^|\n)button\s*\{[^}]*border:\s*0/);
  });
  it("lets a button inherit the page's type and colour", () => {
    expect(css).toMatch(/(^|\n)button\s*\{[^}]*font:\s*inherit/);
    expect(css).toMatch(/(^|\n)button\s*\{[^}]*color:\s*inherit/);
  });
});
