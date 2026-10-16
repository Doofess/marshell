import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * The look is made of tokens. A feature or component stylesheet names colours only through `var(--token)`: no hex, no
 * rgb()/hsl()/oklch() of its own, so a theme, an accent or High Contrast can change every colour from one place. The three
 * files that DEFINE colours are the only place a colour is written out.
 */
const SRC = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const walk = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name)) : e.name.endsWith(".css") ? [join(dir, e.name)] : []));
const FILES = [...walk(join(SRC, "features")), ...walk(join(SRC, "components")), ...walk(join(SRC, "styles"))].map((f) => relative(SRC, f).replace(/\\/g, "/"));

/** Where colours are defined. */
const DEFINITIONS = new Set(["styles/tokens.css", "styles/accents.css", "styles/appearance.css", "components/AgentMark/agent-colors.css"]);

/** Files that hold a colour of their own, each with its reason. A file that stops needing it must leave this list. */
const ALLOWED: Record<string, string> = {
  "components/Logo/Logo.css": "The app icon's three fixed brand colours (tile, hand, signal): artwork, identical in every theme and accent, not a themed surface.",
};

const read = (f: string) => readFileSync(join(SRC, f), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** A `mask-image` gradient uses only alpha, so its opaque black stop is not a visible colour. */
const withoutMasks = (css: string) => css.replace(/mask-image:[^;]*;/g, "");

const RAW = /#[0-9a-fA-F]{3,8}\b|\b(?:rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch|color)\(/;
const rawColours = (css: string) =>
  withoutMasks(css)
    .split("\n")
    .map((l, i) => ({ line: i + 1, text: l.trim() }))
    .filter((l) => RAW.test(l.text));

describe("the colour detector", () => {
  it("catches hex, functions and raw oklch, and lets tokens, system colours and masks through", () => {
    for (const d of ["color: #fff", "color: #FF00FFAA", "background: rgb(0 0 0)", "box-shadow: 0 2px 10px oklch(0% 0 none / 0.28)", "color: hsl(10 20% 30%)"]) expect(rawColours(d), d).toHaveLength(1);
    for (const d of ["color: var(--text-1)", "outline: 1px solid CanvasText", "background: color-mix(in oklch, var(--error) 9%, transparent)", "mask-image: linear-gradient(to bottom, oklch(0% 0 none) 55%, transparent);", "box-shadow: var(--shadow-menu)"])
      expect(rawColours(d), d).toHaveLength(0);
  });
});

describe("feature, component and global stylesheets use tokens only", () => {
  it("scans every stylesheet, new ones included", () => {
    expect(FILES.length).toBeGreaterThan(25);
    expect(FILES).toContain("features/window/MainWindow.css");
    expect(FILES).toContain("styles/tokens.css");
    for (const d of DEFINITIONS) expect(FILES, d).toContain(d);
  });

  for (const f of FILES.filter((x) => !DEFINITIONS.has(x) && !(x in ALLOWED)))
    it(`${f} writes no colour of its own`, () => {
      expect(rawColours(read(f)).map((l) => `${l.line}: ${l.text}`)).toEqual([]);
    });

  it("keeps the allowed list honest: each file still has a colour of its own and says why", () => {
    for (const [f, reason] of Object.entries(ALLOWED)) {
      expect(reason.length, `${f} needs a reason`).toBeGreaterThan(20);
      expect(rawColours(read(f)).length, `${f} no longer needs to be allowed`).toBeGreaterThan(0);
    }
  });
});
