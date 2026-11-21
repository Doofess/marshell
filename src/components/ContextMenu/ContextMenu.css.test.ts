import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./ContextMenu.css", import.meta.url), "utf8");
const bare = css.replace(/\/\*[\s\S]*?\*\//g, "");
const rule = (s: string) => {
  const i = bare.indexOf(`${s} {`);
  return i < 0 ? "" : bare.slice(i, bare.indexOf("}", i));
};

describe("ContextMenu.css", () => {
  it("is vendor-neutral: it never reads the agent signal", () => {
    expect(bare).not.toContain("--agent");
  });
  it("sits on the overlay fill", () => {
    expect(rule(".context-menu")).toContain("var(--bg-overlay)");
  });
  it("marks the active item with the app accent", () => {
    expect(rule('.context-menu [role^="menuitem"]:focus')).toContain("var(--accent)");
  });
  it("draws no ring on the menu itself", () => {
    expect(rule(".context-menu:focus")).toMatch(/outline:\s*(2px solid transparent|none)/);
  });
  it("uses tokens only", () => {
    expect(bare).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
  });
  it("draws the focused item with Highlight in Windows high contrast", () => {
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    expect(forced).toContain("Highlight");
    expect(forced).toContain("HighlightText");
  });
  it("draws the check mark and the radio dot in currentColor, so they follow the row's text", () => {
    expect(rule(".context-menu__mark")).toContain("currentColor");
    expect(rule(".context-menu__mark")).not.toMatch(/(?:color|stroke|fill):\s*var\(/);
  });
  it("names system colors only for the marks in forced colors", () => {
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    const block = /\.context-menu__mark[^{]*\{[^}]*\}/.exec(forced)?.[0] ?? "";
    expect(block).not.toBe("");
    expect(block).not.toMatch(/var\(--|#[0-9a-f]{3,8}\b|oklch|rgb/i);
    expect(block).toMatch(/CanvasText|Highlight|GrayText|LinkText/);
  });
  it("keeps the note quiet and lets it follow the focused row in forced colors", () => {
    expect(rule(".context-menu__note")).toContain("var(--text-2)");
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    expect(forced).toMatch(/:focus[^{]*\.context-menu__note/);
  });
});
