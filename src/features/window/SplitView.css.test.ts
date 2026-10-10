import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./SplitView.css", import.meta.url), "utf8");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("SplitView.css", () => {
  it("dims what an inactive pane header holds to 60%, but never the terminal text, a pane that needs you, or the narrow-pane warning", () => {
    expect(rule('.pane[data-active="false"] .pane__header:not([data-needs="true"]) > :not(.chip)')).toContain("opacity: 0.6");
    expect(css).not.toMatch(/\.pane[^{]*\.scripted-terminal[^{]*\{[^}]*opacity/);
  });
  it("marks the active pane with a 2 px bar under its header, in the focus colour (the vendor's, with its own 3:1 check)", () => {
    const r = rule('.pane[data-active="true"] .pane__header');
    expect(r).toContain("2px");
    expect(r).toContain("var(--focus");
  });
  it("makes a divider one pixel of hairline with an 8 px hit area that overhangs the panes", () => {
    expect(rule(".split__handle")).toContain("--hit: 8px");
    expect(rule(".split__handle")).toContain("background: var(--hairline)");
    expect(css).toContain(".split__handle::before");
    expect(css).toMatch(/inset-inline:\s*calc\(\(var\(--hit\) - 1px\) \/ -2\)/);
    expect(css).toContain("col-resize");
    expect(css).toContain("row-resize");
  });
  it("keeps the receding header readable: its secondary text and buttons use full-strength ink so 60% opacity still passes AA", () => {
    expect(css).toMatch(/\.pane\[data-active="false"\] \.pane__place,[\s\S]*?\{\s*color: var\(--text-1\)/);
  });
  it("tints a pane that needs you with the vendor colour", () => {
    expect(rule('.pane__header[data-needs="true"]')).toContain("var(--agent-signal");
  });
  it("makes the header buttons 24 px, the minimum target", () => {
    const r = rule(".pane__button");
    expect(r).toContain("inline-size: 24px");
    expect(r).toContain("block-size: 24px");
  });
  it("styles focus and press, hover only for a fine pointer", () => {
    expect(css).toContain(".pane__button:focus-visible");
    expect(css).toContain(".pane__button:active");
    expect(css).toContain(".split__handle:focus-visible");
    expect(css).toMatch(/@media \(hover: hover\) and \(pointer: fine\)/);
  });
  it("uses tokens and logical properties only", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).not.toMatch(/\b(left|right|top|bottom):/);
  });
});
