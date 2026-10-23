import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./History.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
const rule = (selector: string) => {
  const i = css.indexOf(`${selector} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("History.css", () => {
  it("names no colour of its own: tokens, system colours and color-mix over tokens only", () => {
    expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?|oklch|oklab|lab|lch|hwb)\(/);
    expect(css).toMatch(/var\(--text-1\)/);
  });

  it("sizes type from the scale and spacing from the grid", () => {
    expect(css).not.toMatch(/font-size:\s*[\d.]+(px|rem)/);
    expect(css).not.toMatch(/(?:padding|margin|gap)[a-z-]*:\s*[^;]*\b\d+px/);
  });

  it("is a 360 px list beside the conversation, and swaps them when the view is under 900 px wide", () => {
    expect(css).toMatch(/grid-template-columns:\s*360px\s+minmax\(0,\s*1fr\)/);
    expect(css).toMatch(/@container history \(max-width:\s*899px\)/);
  });

  it("marks a hit with weight and an underline, not colour alone", () => {
    expect(rule(".history__snippet mark")).toMatch(/font-weight/);
    expect(rule(".history__snippet mark")).toMatch(/text-decoration:\s*underline/);
  });

  it("marks the selected row with a start-edge bar as well as a fill", () => {
    expect(rule('.history__row[aria-selected="true"]')).toMatch(/background:\s*var\(--selected\)/);
    expect(rule('.history__row[aria-selected="true"]')).toMatch(/border-inline-start:\s*3px solid/);
  });

  it("marks a failed tool card with a start-edge bar in the error colour", () => {
    expect(rule('.tool[data-error="true"]')).toMatch(/border-inline-start:\s*3px solid var\(--error\)/);
  });

  it("wraps long text instead of scrolling it sideways", () => {
    expect(rule(".tool__detail")).toMatch(/white-space:\s*pre-wrap/);
    expect(rule(".tool__detail")).toMatch(/overflow-wrap:\s*anywhere/);
  });

  it("never removes a focus outline, and styles focus", () => {
    expect(css).not.toMatch(/outline:\s*none/);
    expect(css).toMatch(/:focus-visible/);
  });

  it("only styles hover where the device hovers", () => {
    expect(css.slice(0, css.indexOf("@media (hover: hover) and (pointer: fine)"))).not.toContain(":hover");
  });

  it("covers hits, the selected row, the controls and the error card in forced colours", () => {
    expect(forced.startsWith("@media (forced-colors: active)")).toBe(true);
    expect(forced).toMatch(/mark[^{]*\{[^}]*(?:Mark|Highlight)/);
    expect(forced).toMatch(/\.history__row\[aria-selected="true"\][^{]*\{[^}]*outline:\s*2px solid Highlight/);
    expect(forced).toMatch(/\.history__search[^{]*\{[^}]*outline:\s*1px solid CanvasText/);
    expect(forced).toMatch(/\.history__select[^{]*\{[^}]*outline:\s*1px solid CanvasText/);
    expect(forced).toMatch(/\.tool\[data-error="true"\][^{]*\{[^}]*outline/);
    expect(forced).not.toMatch(/border(?:-[a-z]+)*-color\s*:/);
  });

  it("scrolls only the list and the conversation, each as its own pane", () => {
    expect([...css.matchAll(/overflow(-x|-y)?:\s*(auto|scroll)/g)]).toHaveLength(2);
    expect(rule(".history__list")).toMatch(/overflow-y:\s*auto/);
    expect(rule(".history__detail")).toMatch(/overflow-y:\s*auto/);
    expect(rule(".history__list")).toMatch(/overscroll-behavior:\s*contain/);
  });
});
