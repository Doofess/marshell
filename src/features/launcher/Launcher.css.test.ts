import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Launcher.css", import.meta.url), "utf8");
const bare = css.replace(/\/\*[\s\S]*?\*\//g, "");
const rule = (s: string) => {
  const i = bare.indexOf(`${s} {`);
  return i < 0 ? "" : bare.slice(i, bare.indexOf("}", i));
};

describe("Launcher.css", () => {
  it("is a 560 px sheet 20% from the top", () => {
    expect(rule(".launcher__sheet")).toContain("inline-size: min(560px");
    expect(rule(".launcher__sheet")).toContain("inset-block-start: 20%");
  });
  it("styles focus and press on chips, programs and options", () => {
    expect(bare).toContain(".launcher__chip:focus-visible");
    expect(bare).toContain(".launcher__chip:active");
    expect(bare).toContain(".launcher__opt:active");
    expect(bare).toContain(".launcher__program:active");
    expect(bare).not.toMatch(/outline:\s*none/);
  });
  it("only styles hover where the device hovers", () => {
    expect(bare.slice(0, bare.indexOf("@media (hover: hover) and (pointer: fine)"))).not.toContain(":hover");
  });
  it("makes the matched letters stand out without relying on colour alone", () => {
    expect(rule(".launcher__opt mark")).toContain("font-weight");
    expect(rule(".launcher__opt mark")).toContain("text-decoration");
  });
  it("marks the active option with a shape as well as a fill: a bar down the side of a row, a line under a program", () => {
    expect(rule(".launcher__opt[data-active]")).toContain("border-inline-start");
    expect(rule(".launcher__program[data-active]")).toContain("box-shadow: inset 0 -2px");
  });
  it("uses tokens and logical properties only", () => {
    expect(bare).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(bare).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(bare).not.toMatch(/\b(left|right|top|bottom):/);
  });
  it("tells surfaces apart by fill, with no stroke around a chip, key, list or the sheet", () => {
    const shipped = bare.replace(/@media \(forced-colors: active\) \{[\s\S]*$/, "");
    expect(shipped).not.toMatch(/(?:^|[\s;{])border:\s*(?!0\b)[\d.]+px\s+solid/m);
    expect(shipped).not.toMatch(/(?:^|[\s;{])border-color:/m);
  });
  it("sets the prompt line like the one in a pane: a flat bar with an edge on top, the vendor's colour while focused, mono type", () => {
    const line = rule(".launcher__line");
    expect(line).toContain("border-block-start: 2px solid var(--hairline)");
    expect(line).not.toContain("border-radius");
    expect(line).not.toContain("box-shadow");
    expect(bare).toMatch(/\.launcher__line:focus-within\s*\{[^}]*border-block-start-color:\s*var\(--focus, var\(--accent\)\)/);
    expect(rule(".launcher__prompt")).toContain("font-family: var(--font-mono)");
    expect(rule(".launcher__glyph")).toContain("color: var(--accent)");
  });
  it("keeps text on a fill at AA: the Enter key on the Start button has no chip behind it, and the key name is not dimmed", () => {
    expect(rule(".launcher__start kbd")).toContain("background: none");
    expect(rule(".launcher__start kbd")).not.toContain("color-mix");
  });
  it("lets the folder list scroll with no scrollbar drawn, and fades its edges", () => {
    const list = rule(".launcher__list");
    expect(list).toMatch(/overflow-y:\s*auto/);
    expect(list).toMatch(/scrollbar-width:\s*none/);
    expect(list).toMatch(/overscroll-behavior:\s*contain/);
    expect(bare).toMatch(/\.launcher__list::-webkit-scrollbar\s*\{[^}]*display:\s*none/);
  });
  it("keeps an edge for every filled control in Windows high contrast, and draws the active option with Highlight", () => {
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    for (const sel of [".launcher__sheet", ".launcher__chip", ".launcher__input", ".launcher__start", ".launcher__program"]) expect(forced, sel).toContain(sel);
    expect(forced).toMatch(/outline:\s*1px solid CanvasText/);
    expect(forced).toMatch(/\[data-active\][^}]*\{[^}]*outline:\s*2px solid Highlight/);
  });
  it("reads the vendor's accent from the sheet, so Start and the line follow the agent", () => {
    expect(rule(".launcher__start")).toContain("background: var(--accent)");
    expect(rule(".launcher__start")).toContain("color: var(--accent-ink)");
  });
});
