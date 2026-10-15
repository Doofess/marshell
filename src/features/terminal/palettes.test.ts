import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { contrast, deltaE } from "../../design/contrast";
import { parseTokens } from "../../design/cssTokens";
import { MIN_CONTRAST, PALETTES } from "./palettes";

const tokens = parseTokens(readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8"), ":root");
const ANSI = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white", "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightMagenta", "brightCyan", "brightWhite"] as const;

for (const scheme of ["dark", "light"] as const) {
  describe(`${scheme} terminal palette`, () => {
    const p = PALETTES[scheme];
    const bg = p.background!;
    it("sits flush with the app's base surface", () => {
      expect(deltaE(bg, tokens["--bg-base"]![scheme])).toBeLessThan(1);
    });
    it("reads at 7:1 for default text", () => {
      expect(contrast(p.foreground!, bg)).toBeGreaterThanOrEqual(7);
    });
    it("keeps the cursor visible", () => {
      expect(contrast(p.cursor!, bg)).toBeGreaterThanOrEqual(3);
    });
    it("keeps selected text readable", () => {
      expect(contrast(p.foreground!, p.selectionBackground!)).toBeGreaterThanOrEqual(4.5);
    });
    it("defines all sixteen ANSI colours", () => {
      for (const n of ANSI) expect(p[n], n).toMatch(/^#[0-9a-f]{6}$/);
    });
    for (const n of ANSI)
      // Dark "black" is the one colour that cannot be readable on black; contrast protection lifts it (review focus 4).
      if (!(scheme === "dark" && n === "black"))
        it(`${n} holds ${MIN_CONTRAST}:1 on its own background`, () => {
          expect(contrast(p[n]!, bg)).toBeGreaterThanOrEqual(MIN_CONTRAST);
        });
  });
}

describe("contrast protection", () => {
  it("defaults to 4.5", () => {
    expect(MIN_CONTRAST).toBe(4.5);
  });
});
