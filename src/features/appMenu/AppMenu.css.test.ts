import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./AppMenu.css", import.meta.url), "utf8");
const bare = css.replace(/\/\*[\s\S]*?\*\//g, "");
const rule = (s: string) => {
  const i = bare.indexOf(`${s} {`);
  return i < 0 ? "" : bare.slice(i, bare.indexOf("}", i));
};

describe("AppMenu.css", () => {
  it("draws over the terminal's layers", () => {
    expect(rule(".app-menu")).toContain("z-index: 20");
  });
  it("reads the count and the arrow in the secondary ink, the item's own while focused", () => {
    expect(rule(".app-menu__count,\n.app-menu__more")).toContain("var(--text-2)");
    expect(bare).toMatch(/\[role="menuitem"\]:focus :is\(\.app-menu__count, \.app-menu__more\)\s*\{[^}]*color:\s*inherit/);
  });
  it("uses tokens only", () => {
    expect(bare).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
  });
  it("uses system colours in forced colours", () => {
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    expect(forced).toContain("CanvasText");
    expect(forced).toContain("HighlightText");
  });
});
