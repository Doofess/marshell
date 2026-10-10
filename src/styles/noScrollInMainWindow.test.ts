import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

/** The main window scrolls nowhere except the session conversation (terminal scrollback), and shows no scrollbar. */
const FILES = [
  "features/lane/Lane.css",
  "features/window/Sidebar.css",
  "features/window/Rail.css",
  "features/window/MainWindow.css",
  "features/sidebar/SessionRow.css",
  "features/approval/ApproveCard.css",
  "features/composer/Composer.css",
  "features/terminal/ScriptedTerminal.css",
  "features/window/SplitView.css",
];
/**
 * Selectors that may scroll: menus and settings (`.scroll-menu`, `.layout-menu`), the request-details drawer (a menu-like
 * inspector for a 40-line command), and the prompt field, which scrolls past eight lines with its scrollbar hidden.
 */
const ALLOWED = [/^\.drawer$/, /^\.composer__field$/, /^\.scroll-menu$/, /^\.layout-menu$/];

const read = (f: string) => readFileSync(new URL(`../${f}`, import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const rules = (css: string) => [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ selectors: m[1]!.split(",").map((s) => s.trim()), body: m[2]! }));
const scrolls = (body: string) => /(^|[;\s])overflow(-[xy])?:\s*(auto|scroll|overlay)/.test(body);

describe("the main window never scrolls", () => {
  for (const f of FILES)
    it(`${f} lets nothing scroll except the allowed menus and the prompt field`, () => {
      for (const r of rules(read(f)))
        if (scrolls(r.body)) for (const sel of r.selectors) expect(ALLOWED.some((a) => a.test(sel)), `${f}: ${sel} scrolls`).toBe(true);
    });

  it("clips the lane, sidebar list and rail list instead", () => {
    for (const [f, sel] of [["features/lane/Lane.css", ".lane__list"], ["features/window/Sidebar.css", ".sidebar__list"], ["features/window/Rail.css", ".rail__list"]] as const) {
      const r = rules(read(f)).find((x) => x.selectors.includes(sel));
      expect(r?.body, sel).toMatch(/overflow:\s*clip/);
    }
  });

  it("hides the prompt field's scrollbar, which still scrolls past eight lines", () => {
    const css = read("features/composer/Composer.css");
    const field = rules(css).find((r) => r.selectors.includes(".composer__field"))!;
    expect(field.body).toMatch(/scrollbar-width:\s*none/);
    const hidden = rules(css).find((r) => r.selectors.includes(".composer__field::-webkit-scrollbar"));
    expect(hidden?.body).toMatch(/display:\s*none/);
  });

  it("keeps the themed scrollbar for menus, where scrolling is allowed", () => {
    const menu = rules(read("features/window/AllSessionsMenu.css")).find((r) => r.selectors.includes(".scroll-menu"))!;
    expect(menu.body).toMatch(/overflow-y:\s*auto/);
    const scrollbars = readFileSync(new URL("./scrollbars.css", import.meta.url), "utf8");
    expect(scrollbars).toContain("*::-webkit-scrollbar-thumb");
  });
});
