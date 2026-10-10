import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const raw = readFileSync(new URL("./Composer.css", import.meta.url), "utf8");
const css = raw.replace(/\/\*[\s\S]*?\*\//g, "");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("Composer.css: a command line, not a floating card", () => {
  it("is a flat bar docked to the terminal's edge: no card radius over 6 px, no shadow, no margin", () => {
    for (const m of css.matchAll(/border-radius:\s*([^;]+);/g)) {
      for (const n of m[1]!.match(/\d+(?=px)/g) ?? []) expect(Number(n), m[0]).toBeLessThanOrEqual(6);
    }
    expect(rule(".composer")).not.toContain("border-radius");
    expect(css).not.toContain("box-shadow: 0");
    expect(rule(".composer")).not.toMatch(/margin/);
    expect(rule(".composer")).not.toContain("background: var(--bg-overlay)");
  });
  it("marks the focused prompt with a 2 px edge on top in the vendor colour, and keeps the others on a hairline of the same height", () => {
    const bar = rule(".composer");
    expect(bar).toMatch(/border-block-start:\s*2px solid var\(--hairline\)/);
    expect(css).toMatch(/\.composer\[data-focused="true"\],\s*\.composer:focus-within\s*\{[^}]*border-block-start-color:\s*var\(--focus, var\(--accent\)\)/);
  });
  it("sets the field in the terminal's mono font at 13 px, so typing feels like typing in the terminal", () => {
    const field = rule(".composer__field");
    expect(field).toContain("font-family: var(--font-mono)");
    expect(field).toContain("font-size: var(--text-13)");
  });
  it("draws the prompt glyph in the vendor colour", () => {
    expect(rule(".composer__prompt")).toContain("color: var(--accent)");
  });
  it("draws the Enter keycap with the accent, readable by the ink the accent already guarantees", () => {
    const kbd = rule(".composer__kbd");
    expect(kbd).toContain("background: var(--accent)");
    expect(kbd).toContain("color: var(--accent-ink)");
    expect(rule('.composer__send[aria-disabled="true"] .composer__kbd')).toContain("var(--fill-strong)");
  });
  it("makes Send a flat text button, not a filled pill or a circle", () => {
    const send = rule(".composer__send");
    expect(send).toContain("background: transparent");
    expect(send).not.toMatch(/border-radius:\s*(999|50%)/);
  });
  it("gives every control a 24 px target at least", () => {
    for (const s of [".composer__tool", ".composer__send", ".composer__resume"]) expect(rule(s), s).toMatch(/min-block-size:\s*24px/);
  });
  it("hides the hint with a range container query, not the legacy max-width form", () => {
    expect(css).toContain("@container (inline-size < 640px)");
    expect(css).not.toContain("@container (max-width");
  });
  it("hides the compact notice's words, not its glyph, when the pane is narrow", () => {
    expect(css).toMatch(/@container \(inline-size < \d+px\)\s*\{[^}]*\.composer\[data-variant="compact"\] \.composer__notice > span/);
  });
  it("draws the key hint's divider on the start side in any writing direction, if it draws one", () => {
    expect(css).not.toMatch(/box-shadow:\s*inset 1px 0 0/);
  });
  it("uses tokens and logical properties only, and has no outline: none", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).not.toMatch(/(?<![-\w])(left|right|top|bottom|width|height):/);
    expect(css).not.toMatch(/outline:\s*none/);
  });
  it("keeps its edge and the Send keycap visible in Windows high contrast", () => {
    expect(raw).toMatch(/@media \(forced-colors: active\)[\s\S]*outline: 1px solid CanvasText/);
    expect(raw).toMatch(/@media \(forced-colors: active\)[\s\S]*HighlightText/);
  });
});
