import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Settings.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const page = readFileSync(new URL("./SettingsPage.tsx", import.meta.url), "utf8");

describe("Settings.css", () => {
  it("draws no box: the only borders are radius and a start-edge bar", () => {
    for (const m of css.matchAll(/border[a-z-]*\s*:[^;]+;/g)) expect(m[0], m[0]).toMatch(/border-radius|border-inline-start/);
  });
  it("tells a scope badge by a fill, and keeps it visible in forced colours", () => {
    expect(css).toMatch(/\.scope-badge\s*\{[^}]*background:\s*var\(--fill/);
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    expect(forced).toMatch(/\.scope-badge/);
  });
  it("marks the selected nav item with the shared selected mark, not its own style", () => {
    expect(page).toContain("selectedProps(");
  });
  it("marks a locked field with a start-edge bar", () => {
    expect(css).toMatch(/\.field\[data-locked\]\s*\{[^}]*border-inline-start/);
  });
  it("sets the raw editor in the mono font", () => {
    expect(css).toMatch(/\.raw-editor__text\s*\{[^}]*font-family:\s*var\(--font-mono\)/);
  });
  it("stacks the nav above the body when narrow", () => {
    expect(css).toMatch(/@container settings \(inline-size < 720px\)\s*\{[\s\S]*?\.settings__layout\s*\{[^}]*grid-template-columns:\s*1fr/);
  });
  it("never scrolls on its own (the page body does) except the raw text area's own content", () => {
    const blocks = [...css.matchAll(/([^{}]+)\{([^{}]*overflow(?:-x|-y)?:\s*(?:auto|scroll)[^{}]*)\}/g)].map((b) => b[1]!.trim());
    expect(blocks).toEqual([".raw-editor__text"]);
  });
});
