import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { formatHex, parse } from "culori";
import { contrast } from "../../design/contrast";
import { parseTokens } from "../../design/cssTokens";
import { HIGH_CONTRAST_PALETTES, MIN_CONTRAST, PALETTES } from "./palettes";

const tokens = parseTokens(readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8"), ":root");
const appearanceCss = readFileSync(new URL("../../styles/appearance.css", import.meta.url), "utf8");
const highContrast = parseTokens(appearanceCss, ':root[data-contrast="more"]');
const hexOf = (css: string) => formatHex(parse(css))!;

describe("default terminal backgrounds", () => {
  // The same colour as the window to the sRGB byte, or a seam shows between the terminal and the window.
  for (const scheme of ["dark", "light"] as const) {
    it(`${scheme} is the base surface`, () => {
      expect(PALETTES[scheme].background).toBe(hexOf(tokens["--bg-base"]![scheme]));
      expect(PALETTES[scheme].cursorAccent).toBe(PALETTES[scheme].background);
    });
    it(`high-contrast ${scheme} is the high-contrast base surface`, () => {
      expect(HIGH_CONTRAST_PALETTES[scheme].background).toBe(hexOf(highContrast["--bg-base"]![scheme]));
      expect(HIGH_CONTRAST_PALETTES[scheme].cursorAccent).toBe(HIGH_CONTRAST_PALETTES[scheme].background);
    });
  }
  it("high contrast keeps true black and pure white", () => {
    expect(HIGH_CONTRAST_PALETTES.dark.background).toBe("#000000");
    expect(HIGH_CONTRAST_PALETTES.light.background).toBe("#ffffff");
  });
});

const ANSI = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white", "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightMagenta", "brightCyan", "brightWhite"] as const;

for (const [label, set] of [["", PALETTES], ["high-contrast ", HIGH_CONTRAST_PALETTES]] as const)
for (const scheme of ["dark", "light"] as const) {
  describe(`${label}${scheme} terminal palette`, () => {
    const p = set[scheme];
    const bg = p.background!;
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
