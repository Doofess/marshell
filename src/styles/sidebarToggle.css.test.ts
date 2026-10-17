import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const strip = (s: string) => s.replace(/\/\*[\s\S]*?\*\//g, "");
const css = strip(readFileSync(new URL("../features/window/MainWindow.css", import.meta.url), "utf8"));
const escape = (s: string) => s.replace(/[.[\]"=()]/g, (c) => "\\" + c);
const rule = (sel: string) => css.match(new RegExp("(?:^|\\n)" + escape(sel) + "\\s*\\{([^}]*)\\}"))?.[1] ?? "";

describe("sidebar toggle button", () => {
  const base = rule(".window__sidebar-toggle");
  it("is a fixed 32 by 28 target that never shrinks", () => {
    expect(base).toMatch(/flex:\s*none/);
    expect(base).toMatch(/block-size:\s*28px/);
    expect(base).toMatch(/inline-size:\s*32px/);
    expect(base).toMatch(/margin-inline-start:\s*var\(--space-1\)/);
  });
  it("hovers only under a fine pointer", () => {
    expect(css).toMatch(/@media \(hover: hover\) and \(pointer: fine\)\s*\{[^@]*\.window__sidebar-toggle:hover/);
    expect(css.replace(/@media \(hover: hover\) and \(pointer: fine\)\s*\{[^@]*?\n\}/g, "")).not.toMatch(/\.window__sidebar-toggle:hover/);
  });
  it("lets the palette button shrink and clip its text at the 200 px sidebar", () => {
    expect(rule(".window__palette")).toMatch(/min-inline-size:\s*0/);
    expect(rule(".window__palette span")).toMatch(/text-overflow:\s*ellipsis/);
  });
  it("sizes the rail's bar column to its content, the expanded one to the sidebar", () => {
    expect(css).toMatch(/\.window\[data-mode="rail"\]\s+\.window__bar\s*\{[^}]*grid-template-columns:\s*max-content minmax\(0, 1fr\) auto/);
    expect(rule(".window__bar")).toMatch(/grid-template-columns:\s*var\(--side-w\)/);
  });
});
