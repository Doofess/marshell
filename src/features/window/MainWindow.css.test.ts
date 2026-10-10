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
  it("dims an inactive pane header to 60% but never the terminal text", () => {
    expect(rule('.pane[data-active="false"] .pane__header')).toContain("opacity: 0.6");
    expect(css).not.toMatch(/\.pane[^{]*\.scripted-terminal[^{]*\{[^}]*opacity/);
  });
  it("marks the active pane with a 2 px accent bar under its header", () => {
    expect(rule('.pane[data-active="true"] .pane__header')).toContain("2px");
    expect(rule('.pane[data-active="true"] .pane__header')).toContain("var(--accent)");
  });
  it("gives the split separator an 8 px hit area around a 1 px line", () => {
    expect(rule(".split__handle")).toContain("inline-size: 8px");
    expect(css).toContain("1px");
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
