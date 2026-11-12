import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const strip = (s: string) => s.replace(/\/\*[\s\S]*?\*\//g, "");
const css = strip(readFileSync(new URL("../features/window/MainWindow.css", import.meta.url), "utf8"));
const escape = (s: string) => s.replace(/[.[\]"=()]/g, (c) => "\\" + c);
const rule = (sel: string) => css.match(new RegExp("(?:^|\\n)" + escape(sel) + "\\s*\\{([^}]*)\\}"))?.[1] ?? "";

describe("side pane toggle button", () => {
  const base = rule(".window__pane-toggle");
  it("is a fixed 32 by 28 target that never shrinks, with its margin at the end", () => {
    expect(base).toMatch(/flex:\s*none/);
    expect(base).toMatch(/block-size:\s*28px/);
    expect(base).toMatch(/inline-size:\s*32px/);
    expect(base).toMatch(/margin-inline-end:\s*var\(--space-1\)/);
    expect(base).not.toMatch(/margin-inline-start/);
  });
  it("has its own focus, pressed and disabled rules", () => {
    expect(rule(".window__pane-toggle:focus-visible")).toMatch(/outline:\s*2px solid/);
    expect(rule(".window__pane-toggle:active")).toMatch(/background/);
    expect(rule('.window__pane-toggle[aria-disabled="true"]')).toMatch(/opacity/);
  });
  it("hovers only under a fine pointer", () => {
    expect(css).toMatch(/@media \(hover: hover\) and \(pointer: fine\)\s*\{[^@]*\.window__pane-toggle:hover/);
    expect(css.replace(/@media \(hover: hover\) and \(pointer: fine\)\s*\{[^@]*?\n\}/g, "")).not.toMatch(/\.window__pane-toggle:hover/);
  });
  it("is outlined in forced colors", () => {
    expect(css).toMatch(/@media \(forced-colors: active\)\s*\{[^@]*\.window__pane-toggle,[^@]*outline:\s*1px solid CanvasText/);
  });
});
