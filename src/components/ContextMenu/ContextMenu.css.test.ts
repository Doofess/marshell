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
  it("sizes the menu by its content, not by the room beside the anchor", () => {
    expect(rule(".context-menu")).toContain("inline-size: max-content");
  });
  it("caps the menu's width to the window", () => {
    expect(rule(".context-menu")).toMatch(/max-inline-size:[^;]*100vw/);
  });
  it("starts every label at the row's start: only the shortcut and the submenu arrow are pushed to the end", () => {
    expect(rule('.context-menu [role^="menuitem"]')).not.toContain("space-between");
    expect(rule(".context-menu__shortcut")).toContain("margin-inline-start: auto");
    expect(rule(".context-menu__more")).toContain("margin-inline-start: auto");
  });
  it("keeps a reason and a note on a line of their own, and out of the menu's width", () => {
    for (const s of [".context-menu__reason", ".context-menu__note"]) {
      expect(rule(s)).toContain("inline-size: 0");
      expect(rule(s)).toContain("min-inline-size: 100%");
      expect(rule(s)).not.toContain("flex-basis: 100%");
    }
  });
  it("lines a reason and a note up with the label of a marked row, inside the row", () => {
    for (const s of [".context-menu__reason", ".context-menu__note"]) expect(rule(s)).toContain("box-sizing: border-box");
    expect(bare).toMatch(/\.context-menu__item--marked > \.context-menu__reason,\s*\.context-menu__item--marked > \.context-menu__note \{[^}]*padding-inline-start/);
    expect(rule('.context-menu [role^="menuitem"][aria-disabled="true"]')).toContain("row-gap: 0");
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
