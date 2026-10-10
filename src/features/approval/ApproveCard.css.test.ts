import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./ApproveCard.css", import.meta.url), "utf8");
const rule = (selector: string) => {
  const i = css.indexOf(`${selector} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

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
