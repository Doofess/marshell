import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const read = (f: string) => readFileSync(new URL(f, import.meta.url), "utf8");
const rule = (css: string, s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};
const HOVER = "@media (hover: hover) and (pointer: fine) {";

describe.each([
  ["Sidebar.css", ".sidebar__menu"],
  ["Rail.css", ".rail__menu"],
])("%s menu button", (file, sel) => {
  const css = read(`./${file}`);
  it("keeps the focus ring and has no outline: none", () => {
    expect(rule(css, sel)).not.toMatch(/outline:\s*none/);
    expect(rule(css, `${sel}:focus-visible`)).toMatch(/outline:\s*2px solid/);
  });
  it("fills on press, and on hover only where hover exists", () => {
    expect(rule(css, `${sel}:active`)).toContain("background");
    const at = css.indexOf(HOVER, css.indexOf(sel));
    expect(at).toBeGreaterThan(-1);
    const block = css.slice(at, css.indexOf("\n}\n", at));
    expect(block).toContain(`${sel}:hover`);
    const hovers = css.split(`${sel}:hover`).length - 1;
    expect(hovers).toBe(1);
  });
  it("draws its icon in ButtonText in forced colors", () => {
    const fc = css.slice(css.indexOf("@media (forced-colors: active)"));
    const i = fc.indexOf(sel);
    expect(i).toBeGreaterThan(-1);
    expect(fc.slice(i, fc.indexOf("}", i))).toMatch(/color:\s*ButtonText/);
  });
});
