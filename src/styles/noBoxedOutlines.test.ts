import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const files = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? files(join(dir, e.name)) : e.name.endsWith(".css") ? [join(dir, e.name)] : []));
/** The review page's own frames live in src/design; the app's components and styles are what ships. */
const shipped = files(root).filter((f) => !f.replace(/\\/g, "/").includes("/src/design/") && !f.endsWith("scrollbars.css"));
const withoutForcedColors = (css: string) => css.replace(/@media \(forced-colors: active\) \{(?:[^{}]|\{[^{}]*\})*\}/g, "");

describe("no boxed outlines", () => {
  it("finds the stylesheets", () => {
    expect(shipped.length).toBeGreaterThan(15);
  });
  for (const f of shipped)
    describe(f.replace(/\\/g, "/").split("/src/")[1] ?? f, () => {
      const css = withoutForcedColors(readFileSync(f, "utf8"));
      it("draws no stroke around a control, card, chip or key: surfaces are told apart by fill, not by a box", () => {
        expect(css).not.toMatch(/(?:^|[\s;{])border:\s*(?!0\b)[\d.]+px\s+solid/m);
        expect(css).not.toMatch(/(?:^|[\s;{])border-color:/m);
      });
      it("keeps keyboard focus visible with an outline, the one outline that stays", () => {
        expect(css).not.toMatch(/outline:\s*none/);
      });
    });
});

/** Every `selector { declarations }` pair, innermost blocks included, with comments and the High Contrast block taken out. */
const read = (f: string) => withoutForcedColors(readFileSync(f, "utf8")).replace(/\/\*[\s\S]*?\*\//g, "");
const rulesOf = (css: string) => [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ selector: m[1]!.trim().replace(/\s+/g, " "), body: m[2]! }));

/** A shadow that draws a ring: no offset, no blur, a spread (`0 0 0 2px`, inset or not). `inset 0 0 0 100vmax` is a press fill, not a ring. */
const RING = /box-shadow:\s*(?:inset\s+)?0\s+0\s+0\s+(?!100vmax)[\d.]+px/;
/** The border box shorthands that were not caught by `border:`: a width or a style turns a border on. */
const BORDER_BOX = /(?:^|[\s;])border-(?:width|style)\s*:/m;
/** An outline outside :focus. A transparent one is a High Contrast placeholder that draws nothing. */
const STRAY_OUTLINE = /(?:^|[\s;])outline:\s*(?=\S)(?!none\b|0\b)(?![^;]*\btransparent\b)/m;

/**
 * The few places a ring or box is the right thing, each with its reason. A selector that stops matching must leave this
 * list, so it can never grow into a hiding place.
 */
const ALLOWED: Record<string, Record<string, string>> = {
  "features/window/MainWindow.css": { ".window": "The OS window's own edge, which a real window has; not a control." },
  "features/window/SplitView.css": { '.pane[data-split]::after': "The focused pane's ring says which terminal gets your keys; the user chose a ring over the header bar (2026-10-17)." },
  "features/window/LayoutMenu.css": { ".layout-menu__box": "The checkbox's box is the control itself: without it an unchecked box is invisible (WCAG 1.4.11 asks 3:1 for the control's boundary)." },
};

describe("no ring, box or stray outline either", () => {
  const hits = shipped.flatMap((f) => {
    const name = f.replace(/\\/g, "/").split("/src/")[1] ?? f;
    return rulesOf(read(f)).flatMap((r) => {
      const kinds: string[] = [];
      if (RING.test(r.body)) kinds.push("ring-shaped box-shadow");
      if (BORDER_BOX.test(r.body)) kinds.push("border-width or border-style");
      if (STRAY_OUTLINE.test(r.body) && !/:focus/.test(r.selector)) kinds.push("outline outside :focus");
      return kinds.map((kind) => ({ name, selector: r.selector, kind }));
    });
  });

  it("recognises what it is about", () => {
    for (const d of ["box-shadow: inset 0 0 0 2px var(--error)", "box-shadow: 0 0 0 1px var(--hairline)", "box-shadow: inset 0 0 0 1.5px red"]) expect(d).toMatch(RING);
    for (const d of ["box-shadow: inset 0 0 0 100vmax var(--selected)", "box-shadow: inset 0 1px 0 var(--hairline)", "box-shadow: 0 4px 18px #0004"]) expect(d).not.toMatch(RING);
    expect("border-width: 1px").toMatch(BORDER_BOX);
    expect("border-style: solid").toMatch(BORDER_BOX);
    expect("border-block-end: 2px solid red").not.toMatch(BORDER_BOX);
    expect("outline: 2px solid red").toMatch(STRAY_OUTLINE);
    expect("outline: 2px solid transparent").not.toMatch(STRAY_OUTLINE);
    expect("outline: none").not.toMatch(STRAY_OUTLINE);
  });

  it("finds nothing that is not on the allowed list", () => {
    const stray = hits.filter((h) => !(h.name in ALLOWED && h.selector in ALLOWED[h.name]!)).map((h) => `${h.name}: ${h.selector} (${h.kind})`);
    expect(stray).toEqual([]);
  });

  it("keeps the allowed list honest: every entry still matches something and says why", () => {
    for (const [file, sels] of Object.entries(ALLOWED))
      for (const [sel, reason] of Object.entries(sels)) {
        expect(reason.length, `${file}: ${sel} needs a reason`).toBeGreaterThan(20);
        expect(hits.some((h) => h.name === file && h.selector === sel), `${file}: ${sel} no longer needs to be allowed`).toBe(true);
      }
  });
});

describe("no browser-drawn box either", () => {
  it("flattens text inputs, selects and text areas in base.css, since the browser's own inset border is a box no declaration scan can see (see formControls.test.ts)", () => {
    const base = readFileSync(join(root, "styles/base.css"), "utf8");
    expect(base).toMatch(/:where\(input:not\(\[type="checkbox"\]\):not\(\[type="radio"\]\), select, textarea\)\s*\{[^}]*border:\s*0/);
  });
});
