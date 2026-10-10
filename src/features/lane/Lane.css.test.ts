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
  it("scrolls internally and fades the cut edge", () => {
    expect(rule(".lane__list")).toMatch(/overflow-y:\s*auto/);
    expect(css).toContain("mask-image");
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
  it("adds no gap or end padding to the list, so the lane's size (header + card + one-liners) is exactly what laneLayout says and nothing scrolls by a few pixels", () => {
    const r = rule(".lane__list");
    expect(r).toMatch(/gap:\s*0/);
    expect(r).not.toMatch(/padding-block-end/);
  });
});
