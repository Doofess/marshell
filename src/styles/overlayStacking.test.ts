import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./app.css", import.meta.url), "utf8");
const palette = readFileSync(new URL("../features/palette/Palette.css", import.meta.url), "utf8");
const block = (src: string, sel: string) => {
  const i = src.indexOf(`${sel} {`);
  return i < 0 ? "" : src.slice(i, src.indexOf("}", i));
};

// The split divider (z 1), the composers (z 2) and xterm's decoration, ruler and scrollbar layers (z 6–11) are positioned with a
// z-index, so a palette layer left at auto had their lines painted across its sheet.
describe("command palette layer", () => {
  it("stacks above every layer inside the window", () => {
    const z = Number(block(css, ".live-app__palette").match(/z-index:\s*(\d+)/)?.[1] ?? 0);
    expect(z).toBeGreaterThan(11);
  });
  it("lifts the sheet with a soft shadow, not a bright halo", () => {
    const shadow = block(palette, ".palette__sheet").match(/box-shadow:\s*([^;]+);/)?.[1] ?? "";
    const pct = Number(shadow.match(/(\d+)%,\s*transparent/)?.[1] ?? 100);
    expect(pct).toBeLessThanOrEqual(12);
  });
});
