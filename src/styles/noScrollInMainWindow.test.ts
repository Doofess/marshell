import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

/** The main window scrolls nowhere except the session conversation (terminal scrollback), and shows no scrollbar. */
const SRC = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const walk = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name)) : e.name.endsWith(".css") ? [join(dir, e.name)] : []));
/** Every feature, component and global stylesheet (tokens, base, app): a new file is scanned without anyone remembering to list it. */
const FILES = [...walk(join(SRC, "features")), ...walk(join(SRC, "components")), ...walk(join(SRC, "styles"))].map((f) => relative(SRC, f).replace(/\\/g, "/"));

/**
 * Selectors that may scroll: menus and settings (`.scroll-menu`, `.layout-menu`), the request-details drawer (a menu-like
 * inspector for a 40-line command), the prompt field, which scrolls past eight lines with its scrollbar hidden, and the raw
 * settings editor's text area (a text box scrolls its own text, like the prompt field; it is not the live window), and the
 * bodies of the fix sheet and the onboarding frame (a change or three consent cards can be longer than a 720 by 480 window;
 * their actions sit in a row outside the scrolling body, with a visible themed scrollbar; see dialogBodies.test.ts), and the
 * Review sheet's file list (`.review__files`: a capped listbox in a modal sheet, so the diff beside it stays in view, like
 * the launcher's and the palette's lists), and the keyboard shortcuts sheet (`.live-shortcuts__sheet`: a modal sheet whose
 * thirty-odd rows are longer than a 720 by 480 window; it has no actions, Esc or Ctrl+Shift+O closes it, and clipping it would
 * hide keys).
 */
const ALLOWED = [/^\.live-shortcuts__sheet$/,/^\.drawer$/, /^\.composer__field$/, /^\.scroll-menu$/, /^\.layout-menu$/, /^\.sidebar__list$/, /^\.rail__list$/, /^\.palette__list$/, /^\.launcher__list$/, /^\.page__body$/, /^\.raw-editor__text$/, /^\.fix-sheet__body$/, /^\.onboarding__body$/, /^\.review__files$/];

const read = (f: string) => readFileSync(join(SRC, f), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const rules = (css: string) => [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ selectors: m[1]!.split(",").map((s) => s.trim()), body: m[2]! }));

const SCROLLY = String.raw`(?:auto|scroll|overlay)`;
/** Any way to make an element scroll: the shorthand in either order, and every longhand, logical ones included. */
const scrolls = (body: string) =>
  new RegExp(String.raw`(^|[;\s])overflow(?:-(?:x|y|block|inline))?:\s*(?:[a-z-]+\s+)?${SCROLLY}\b`).test(body) ||
  new RegExp(String.raw`(^|[;\s])overflow:\s*${SCROLLY}\s+[a-z-]+`).test(body);

describe("the scroll detector", () => {
  it("catches the shorthand, the two-value form in both orders, and every longhand", () => {
    for (const d of ["overflow: auto", "overflow: scroll", "overflow-y: auto", "overflow-x: scroll", "overflow-block: auto", "overflow-inline: scroll", "overflow: hidden auto", "overflow: auto hidden", "overflow: hidden overlay"])
      expect(scrolls(`color: red; ${d};`), d).toBe(true);
  });
  it("lets clipping through", () => {
    for (const d of ["overflow: clip", "overflow: hidden", "overflow: visible", "overflow-y: clip", "overflow: clip clip", "overflow-block: hidden"]) expect(scrolls(`${d};`), d).toBe(false);
  });
});

describe("the main window never scrolls", () => {
  it("scans every feature and component stylesheet", () => {
    expect(FILES.length).toBeGreaterThan(15);
    expect(FILES).toContain("features/window/Sidebar.css");
    expect(FILES).toContain("components/Chip/Chip.css");
    expect(FILES).toContain("styles/base.css");
  });

  for (const f of FILES)
    it(`${f} lets nothing scroll except the allowed menus and the prompt field`, () => {
      for (const r of rules(read(f)))
        if (scrolls(r.body)) for (const sel of r.selectors) expect(ALLOWED.some((a) => a.test(sel)), `${f}: ${sel} scrolls`).toBe(true);
    });

  it("clips the lane instead: it shows whole rows and a +N more button", () => {
    const r = rules(read("features/lane/Lane.css")).find((x) => x.selectors.includes(".lane__list"));
    expect(r?.body).toMatch(/overflow:\s*clip/);
  });

  it("lets the session list and the rail scroll, with the scrollbar hidden: wheel, touch and keys, nothing drawn", () => {
    // The palette's results and the launcher's folder list are the same kind of list, in the two sheets over the window.
    for (const [f, sel] of [
      ["features/window/Sidebar.css", ".sidebar__list"],
      ["features/window/Rail.css", ".rail__list"],
      ["features/palette/Palette.css", ".palette__list"],
      ["features/launcher/Launcher.css", ".launcher__list"],
    ] as const) {
      const css = read(f);
      const r = rules(css).find((x) => x.selectors.includes(sel))!;
      expect(r.body, sel).toMatch(/overflow-y:\s*auto/);
      expect(r.body, sel).toMatch(/scrollbar-width:\s*none/);
      expect(r.body, sel).toMatch(/overscroll-behavior:\s*contain/);
      const hidden = rules(css).find((x) => x.selectors.includes(`${sel}::-webkit-scrollbar`));
      expect(hidden?.body, `${sel}::-webkit-scrollbar`).toMatch(/display:\s*none/);
    }
  });

  it("hides the prompt field's scrollbar, which still scrolls past eight lines", () => {
    const css = read("features/composer/Composer.css");
    const field = rules(css).find((r) => r.selectors.includes(".composer__field"))!;
    expect(field.body).toMatch(/scrollbar-width:\s*none/);
    const hidden = rules(css).find((r) => r.selectors.includes(".composer__field::-webkit-scrollbar"));
    expect(hidden?.body).toMatch(/display:\s*none/);
  });

  it("lets the page body (Doctor, Settings, the diff sheet) scroll, with the scrollbar visible: not the live window, and the exception cannot spread", () => {
    const css = read("components/Page/Page.css");
    const r = rules(css).find((x) => x.selectors.includes(".page__body"))!;
    expect(r.body).toMatch(/overflow-y:\s*auto/);
    expect(css).not.toMatch(/scrollbar-width:\s*none/);
    expect(css).not.toMatch(/::-webkit-scrollbar[^{]*\{[^}]*display:\s*none/);
  });

  it("keeps the themed scrollbar for menus, where scrolling is allowed", () => {
    const menu = rules(read("features/window/AllSessionsMenu.css")).find((r) => r.selectors.includes(".scroll-menu"))!;
    expect(menu.body).toMatch(/overflow-y:\s*auto/);
    const scrollbars = readFileSync(new URL("./scrollbars.css", import.meta.url), "utf8");
    expect(scrollbars).toContain("*::-webkit-scrollbar-thumb");
  });
});
