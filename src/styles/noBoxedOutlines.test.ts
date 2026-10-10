import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const files = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? files(join(dir, e.name)) : e.name.endsWith(".css") ? [join(dir, e.name)] : []));
/** The review page's own frames live in src/design; the app's components and styles are what ships. */
const shipped = files(root).filter((f) => !f.replace(/\\/g, "/").includes("/src/design/") && !f.endsWith("scrollbars.css"));
const withoutForcedColors = (css: string) => css.replace(/@media \(forced-colors: active\) \{(?:[^{}]|\{[^{}]*\})*\}/g, "");

describe("no boxed outlines", () => {
  it("finds the stylesheets", () => {
    expect(shipped.length).toBeGreaterThan(15);
  });
  for (const f of shipped)
    describe(f.replace(/\\/g, "/").split("/src/")[1] ?? f, () => {
      const css = withoutForcedColors(readFileSync(f, "utf8"));
      it("draws no stroke around a control, card, chip or key: surfaces are told apart by fill, not by a box", () => {
        expect(css).not.toMatch(/(?:^|[\s;{])border:\s*(?!0\b)[\d.]+px\s+solid/m);
        expect(css).not.toMatch(/(?:^|[\s;{])border-color:/m);
      });
      it("keeps keyboard focus visible with an outline, the one outline that stays", () => {
        expect(css).not.toMatch(/outline:\s*none/);
      });
    });
});
