import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { contrast, deltaE, overlay } from "../../design/contrast";
import { parseTokens } from "../../design/cssTokens";

const css = readFileSync(new URL("./ApproveCard.css", import.meta.url), "utf8");
const rule = (selector: string) => {
  const i = css.indexOf(`${selector} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

const tokens = parseTokens(readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8"), ":root");
const THEMES = ["dark", "light"] as const;
/** The token a rule's `background` names. */
const fillOf = (selector: string) => rule(selector).match(/background:\s*var\((--[\w-]+)\)/)?.[1];

describe("ApproveCard.css: the command block is recessed into the card", () => {
  it("fills the payload with a token other than the card's", () => {
    expect(fillOf(".approve-card")).toBe("--bg-overlay");
    expect(fillOf(".approve-card__payload")).toBeDefined();
    expect(fillOf(".approve-card__payload")).not.toBe(fillOf(".approve-card"));
  });
  for (const theme of THEMES) {
    it(`tells the payload from the card by at least ΔE 3, and from the risky tint too (${theme})`, () => {
      const payload = tokens[fillOf(".approve-card__payload")!]![theme];
      expect(deltaE(payload, tokens["--bg-overlay"]![theme])).toBeGreaterThanOrEqual(3);
      const risky = overlay(tokens["--error"]![theme], tokens["--bg-overlay"]![theme], 0.09);
      expect(deltaE(payload, risky)).toBeGreaterThanOrEqual(3);
    });
    it(`keeps the command readable on it at 7:1 (${theme})`, () => {
      expect(contrast(tokens["--text-1"]![theme], tokens[fillOf(".approve-card__payload")!]![theme])).toBeGreaterThanOrEqual(7);
    });
  }
});

describe("ApproveCard.css: the risky bar sits on the start edge in any writing direction", () => {
  it("draws the bar as a pseudo-element on inline-start, not a physical inset shadow", () => {
    expect(css).not.toMatch(/box-shadow:\s*inset\s+3px\s+0\s+0/);
    const bar = rule('.approve-card[data-risk="risky"]::before');
    expect(bar).toContain("inset-inline-start: 0");
    expect(bar).toContain("inset-block: 0");
    expect(bar).toContain("inline-size: 3px");
    expect(bar).toContain("var(--error)");
  });
  it("lets the card clip the bar to its rounded corners", () => {
    const risky = rule('.approve-card[data-risk="risky"]');
    expect(risky).toContain("position: relative");
    expect(risky).toContain("overflow: clip");
  });
});

describe("ApproveCard.css", () => {
  it("wraps payload text and never cuts it off mid-line (review focus 1)", () => {
    expect(rule(".approve-card__payload")).toMatch(/overflow-wrap:\s*anywhere/);
    expect(rule(".approve-card__payload")).toMatch(/white-space:\s*pre-wrap/);
  });
  it("fades a collapsed payload and clips it at four lines", () => {
    const r = rule(".approve-card__payload[data-collapsed]");
    expect(r).toContain("mask-image");
    expect(r).toMatch(/overflow:\s*clip/);
  });
  it("shows focus with :focus-visible and an outline", () => {
    expect(css).toContain(":focus-visible");
    expect(css).not.toMatch(/outline:\s*none/);
  });
  it("only styles hover on devices that hover", () => {
    const hoverRules = css.match(/:hover/g)?.length ?? 0;
    expect(hoverRules).toBeGreaterThan(0);
    expect(css.slice(css.indexOf("@media (hover: hover) and (pointer: fine)"))).toContain(":hover");
    expect(css.slice(0, css.indexOf("@media (hover: hover) and (pointer: fine)"))).not.toContain(":hover");
  });
  it("gives pressable things an active state", () => {
    expect(css).toContain(".approve-card__btn:active");
  });
  it("names transition properties and animates the receipt exit with tokens", () => {
    expect(css).not.toMatch(/transition:\s*all/);
    expect(css).not.toContain("ease-in");
    expect(css).toContain("var(--dur-exit)");
  });
  it("uses tokens for colour, not literals", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
  });
  it("uses logical properties", () => {
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).not.toMatch(/\b(left|right|top|bottom):/);
  });
});
