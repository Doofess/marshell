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
    expect(rule('.context-menu [role="menuitem"]:focus')).toContain("var(--accent)");
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
});
