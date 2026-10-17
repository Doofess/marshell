import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { contrast } from "../design/contrast";
import { parseTokens } from "../design/cssTokens";

const strip = (s: string) => s.replace(/\/\*[\s\S]*?\*\//g, "");
const css = strip(readFileSync(new URL("../features/window/MainWindow.css", import.meta.url), "utf8"));
const tokens = parseTokens(readFileSync(new URL("./tokens.css", import.meta.url), "utf8"), ":root");

describe("window frame", () => {
  it("drops the stand-in window edge when the app is the desktop host (the OS draws the real one)", () => {
    expect(css).toMatch(/:root\[data-host="desktop"\]\s+\.window\s*\{[^}]*border-radius:\s*0[^}]*box-shadow:\s*none/);
  });
  it("keeps the stand-in edge for design review", () => {
    expect(css).toMatch(/(?:^|\n)\.window\s*\{[^}]*border-radius:\s*8px[^}]*box-shadow:\s*0 0 0 1px var\(--hairline\)/);
  });
  for (const theme of ["light", "dark"] as const)
    it(`close glyph on the red hover is readable in ${theme} (>= 4.5:1)`, () => {
      expect(css).toMatch(/\.window__control--close:hover\s*\{[^}]*color:\s*var\(--bg-base\)/);
      expect(contrast(tokens["--error"]![theme], tokens["--bg-base"]![theme])).toBeGreaterThanOrEqual(4.5);
    });
});
