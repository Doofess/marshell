import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./CloseBar.css", import.meta.url), "utf8");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("CloseBar.css: the administrator acknowledgement", () => {
  it("overlays the header's fixed slot without changing the terminal's size", () => {
    const r = rule(".close-bar--ack");
    expect(r).toContain("position: absolute");
    expect(r).toContain("inset-inline: 0");
    expect(r).toContain("inset-block-start: 0");
    expect(r).toContain("block-size: auto");
    expect(Number(/z-index:\s*(\d+)/.exec(r)?.[1])).toBeGreaterThan(11);
    expect(rule(".close-bar")).toContain("position: relative");
    expect(rule(".close-bar")).toContain("block-size: 40px");
  });
  it("covers terminal text with an opaque caution surface and a bottom edge", () => {
    const r = rule(".close-bar--ack");
    expect(r).toContain("background: color-mix(in oklch, var(--caution) 10%, var(--bg-raised))");
    expect(r).toContain("border-block-end: 1px solid var(--hairline)");
  });
  it("uses the system canvas and a visible border in forced colours", () => {
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    expect(forced).toMatch(/\.close-bar--ack\s*\{[^}]*background: Canvas;[^}]*border: 1px solid CanvasText/);
  });
});
