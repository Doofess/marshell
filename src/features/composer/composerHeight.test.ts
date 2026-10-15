import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { COMPOSER, STATUS_BAR_HEIGHT } from "../window/layout";

const read = (name: string) => readFileSync(new URL(name, import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const composer = read("./Composer.css");
const bar = read("./StatusBar.css");
const tokens = read("../../styles/tokens.css");
const rule = (css: string, s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};
const space = (n: number) => Number(new RegExp(String.raw`--space-${n}:\s*(\d+)px`).exec(tokens)![1]);
const px = (css: string, selector: string, prop: string) => Number(new RegExp(String.raw`${prop}:\s*(\d+)px`).exec(rule(css, selector))![1]);
/** A `var(--space-n)` or `Npx` value as a number. */
const len = (v: string) => (v.startsWith("var(") ? space(Number(/--space-(\d+)/.exec(v)![1])) : Number(v.replace("px", "")));

describe("the prompt line's heights are what its CSS adds up to", () => {
  const edge = Number(/border-block-start:\s*(\d+)px/.exec(rule(composer, ".composer"))![1]);
  const [padTop, padBottom] = /padding-block:\s*(var\(--space-\d\)|\d+px)(?:\s+(var\(--space-\d\)|\d+px))?/.exec(rule(composer, ".composer"))!.slice(1).map((v, i, a) => len(v ?? a[0]!));
  const gap = len(/row-gap:\s*(var\(--space-\d\)|\d+px)/.exec(rule(composer, ".composer"))![1]!);
  const line = px(composer, ".composer__line", "min-block-size");
  const tools = px(composer, ".composer__tools", "min-block-size");

  it("is the 2 px edge, padding, the prompt line, the gap and the tools row: that is the full variant", () => {
    expect(COMPOSER.full).toBe(edge + padTop! + line + gap + tools + padBottom!);
  });
  it("is the edge, even padding and the prompt line alone: that is the compact variant", () => {
    const compact = rule(composer, '.composer[data-variant="compact"]');
    const [cTop, cBottom] = /padding-block:\s*(var\(--space-\d\)|\d+px)(?:\s+(var\(--space-\d\)|\d+px))?/.exec(compact)!.slice(1).map((v, i, a) => len(v ?? a[0]!));
    expect(COMPOSER.compact).toBe(edge + cTop! + line + cBottom!);
  });
  it("makes the line and the tools row the 24 px target, and the field one 20 px line inside a 24 px line", () => {
    expect([line, tools]).toEqual([24, 24]);
    expect(px(composer, ".composer__field", "min-block-size")).toBe(24);
    expect(rule(composer, ".composer__field")).toContain("line-height: 20px");
  });
  it("makes the status bar one row, as tall as STATUS_BAR_HEIGHT", () => {
    expect(px(bar, ".status-bar", "block-size")).toBe(STATUS_BAR_HEIGHT);
  });
});
