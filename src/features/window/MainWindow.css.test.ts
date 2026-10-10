import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./MainWindow.css", import.meta.url), "utf8");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("MainWindow.css", () => {
  it("makes the top bar one 40 px row", () => {
    expect(rule(".window__bar")).toContain("block-size: 40px");
  });
  it("sizes the window controls 46 by 40", () => {
    const r = rule(".window__control");
    expect(r).toContain("inline-size: 46px");
    expect(r).toContain("block-size: 40px");
  });
  it("puts the layout menu in a popover that hangs from the bar's end, above the panes", () => {
    expect(rule(".window__popover")).toContain("position: absolute");
    expect(rule(".window__popover")).toMatch(/z-index:\s*3/);
  });
  it("stacks the terminals above the prompt bar in one centre column", () => {
    const r = rule(".window__center");
    expect(r).toContain("flex-direction: column");
    expect(r).toContain("flex: 1 1 0");
  });
  it("styles focus and press on the bar buttons", () => {
    expect(css).toContain(".window__control:focus-visible");
    expect(css).toContain(".window__control:active");
    expect(css).not.toMatch(/outline:\s*none/);
  });
  it("uses tokens and logical properties only", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).not.toMatch(/\b(left|right|top|bottom):/);
  });
});
