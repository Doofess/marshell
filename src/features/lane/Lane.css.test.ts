import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Lane.css", import.meta.url), "utf8");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("Lane.css", () => {
  it("fixes the header at 28 px so the lane never pops in or out", () => {
    expect(rule(".lane__header")).toContain("block-size: 28px");
  });
  it("makes one-liners 36 px", () => {
    expect(rule(".lane-row")).toContain("block-size: 36px");
  });
  it("never scrolls and never fades: it shows whole rows, so there is no cut edge to hint at", () => {
    expect(rule(".lane__list")).toMatch(/overflow:\s*clip/);
    expect(css).not.toMatch(/overflow(-[xy])?:\s*(auto|scroll)/);
    expect(css).not.toContain("mask-image");
  });
  it("makes the +N more a real button: a fine-pointer hover, a press state, a focus ring", () => {
    expect(css).toContain(".lane__more:focus-visible");
    expect(css).toContain(".lane__more:active");
    expect(css).toMatch(/\.lane__more:hover/);
  });
  it("styles focus and press", () => {
    expect(css).toContain(".lane-row:focus-visible");
    expect(css).toContain(".lane-row:active");
    expect(css).not.toMatch(/outline:\s*none/);
  });
  it("uses tokens and logical properties", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
  });
  it("adds no gap or end padding to the list, so the lane's size (header + card + one-liners) is exactly what laneLayout says", () => {
    const r = rule(".lane__list");
    expect(r).toMatch(/gap:\s*0/);
    expect(r).not.toMatch(/padding-block-end/);
  });
});
