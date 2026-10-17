import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Palette.css", import.meta.url), "utf8");
const bare = css.replace(/\/\*[\s\S]*?\*\//g, "");
const rule = (s: string) => {
  const i = bare.indexOf(`${s} {`);
  return i < 0 ? "" : bare.slice(i, bare.indexOf("}", i));
};

describe("Palette.css contrast", () => {
  it("draws a shortcut in the full text colour: on the key's own fill, text-2 falls under 4.5:1", () => {
    expect(bare).toMatch(/\.palette__kbd,\s*\.palette__footer kbd\s*\{[^}]*color:\s*var\(--text-1\)/);
  });
});

describe("Palette.css", () => {
  it("is a 560 px sheet 20% from the top, like the launcher", () => {
    expect(rule(".palette__sheet")).toContain("inline-size: min(560px");
    expect(rule(".palette__sheet")).toContain("inset-block-start: 20%");
  });
  it("styles focus and press, and hover only where hover exists", () => {
    expect(bare).toContain(".palette__input:focus-visible");
    expect(bare).toContain(".palette__opt:active");
    expect(bare.slice(0, bare.indexOf("@media (hover: hover) and (pointer: fine)"))).not.toContain(":hover");
    expect(bare).not.toMatch(/outline:\s*none/);
  });
  it("draws no ring on the box: it is the only thing in the sheet that takes focus, so the caret says where typing goes", () => {
    // Transparent, not none: Windows high contrast still draws it in the system colour.
    expect(rule(".palette__input:focus-visible")).toMatch(/outline:\s*2px solid transparent/);
    expect(rule(".palette__input:focus-visible")).not.toContain("--accent");
  });
  it("marks the active option with a bar as well as a fill", () => {
    expect(rule(".palette__opt[data-active]")).toContain("border-inline-start");
  });
  it("makes matched letters bold and underlined", () => {
    expect(rule(".palette__opt mark")).toContain("font-weight");
    expect(rule(".palette__opt mark")).toContain("text-decoration");
  });
  it("uses tokens and logical properties only", () => {
    expect(bare).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(bare).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(bare).not.toMatch(/\b(left|right|top|bottom):/);
  });
  it("tells surfaces apart by fill, with no stroke around the sheet, a key or the field", () => {
    const shipped = bare.replace(/@media \(forced-colors: active\) \{[\s\S]*$/, "");
    expect(shipped).not.toMatch(/(?:^|[\s;{])border:\s*(?!0\b)[\d.]+px\s+solid/m);
    expect(shipped).not.toMatch(/(?:^|[\s;{])border-color:/m);
  });
  it("sets session numbers in a fixed column with tabular figures, so 6 and 12 line up", () => {
    const num = rule(".palette__num");
    expect(num).toContain("font-variant-numeric: tabular-nums");
    expect(num).toMatch(/inline-size:\s*\d/);
    expect(num).toContain("text-align: end");
  });
  it("lets the results scroll with no scrollbar drawn, and fades the edges", () => {
    const list = rule(".palette__list");
    expect(list).toMatch(/overflow-y:\s*auto/);
    expect(list).toMatch(/scrollbar-width:\s*none/);
    expect(list).toMatch(/overscroll-behavior:\s*contain/);
    expect(bare).toMatch(/\.palette__list::-webkit-scrollbar\s*\{[^}]*display:\s*none/);
    expect(bare).toContain("animation-timeline: scroll(self block)");
  });
  it("keeps an edge for the sheet and the field in Windows high contrast, and draws the active option with Highlight", () => {
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    for (const sel of [".palette__sheet", ".palette__input"]) expect(forced, sel).toContain(sel);
    expect(forced).toMatch(/outline:\s*1px solid CanvasText/);
    expect(forced).toMatch(/\.palette__opt\[data-active\]\s*\{[^}]*outline:\s*2px solid Highlight/);
  });
});
