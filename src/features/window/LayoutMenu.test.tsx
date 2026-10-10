import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { LayoutIcon, LayoutMenu, type LayoutMenuProps } from "./LayoutMenu";
import { ARRANGEMENTS } from "./splitLayout";

const menu = (p: Partial<LayoutMenuProps> = {}) => renderToStaticMarkup(<LayoutMenu count={4} arrangement="grid" auto {...p} />);
const count = (h: string, re: RegExp) => (h.match(re) ?? []).length;

describe("LayoutIcon", () => {
  it("draws one cell per pane, and is decoration", () => {
    for (const n of [1, 2, 3, 4] as const)
      for (const a of ARRANGEMENTS[n]) {
        const h = renderToStaticMarkup(<LayoutIcon count={n} arrangement={a} />);
        expect(count(h, /<rect/g), `${n} ${a}`).toBe(n);
        expect(h).toContain('aria-hidden="true"');
      }
  });
  it("draws the main pane of main-left the height of the icon, beside two stacked", () => {
    const h = renderToStaticMarkup(<LayoutIcon count={3} arrangement="main-left" size={24} />);
    const heights = [...h.matchAll(/height="([\d.]+)"/g)].map((m) => Number(m[1])).slice(1);
    expect(Math.max(...heights)).toBeGreaterThan(Math.min(...heights) * 1.5);
  });
  it("uses currentColor, so the icon follows the control's text", () => {
    expect(renderToStaticMarkup(<LayoutIcon count={2} arrangement="columns" />)).toContain("currentColor");
  });
});

describe("LayoutMenu: counts the window cannot hold", () => {
  it("turns off the counts above what fits, and says what it would take", () => {
    const h = menu({ count: 2, arrangement: "rows", max: 2 });
    expect(h).toMatch(/aria-label="3 panes"[^>]*aria-disabled="true"|aria-disabled="true"[^>]*aria-label="3 panes"/);
    expect(h).toMatch(/aria-disabled="true"[^>]*aria-label="4 panes"|aria-label="4 panes"[^>]*aria-disabled="true"/);
    expect(h).not.toMatch(/aria-disabled="true"[^>]*aria-label="2 panes"/);
    expect(h).toContain("This window fits 2");
    expect(h).toContain("make it larger");
  });
  it("says so when fewer are shown than asked for, so a missing pane is never a mystery", () => {
    const h = menu({ count: 2, requested: 4, arrangement: "rows", max: 2 });
    expect(h).toContain("Showing 2 of 4");
    expect(h).toContain("The other 2 keep running");
  });
  it("is quiet when everything asked for is on screen, and when all four fit", () => {
    expect(menu({ max: 4 })).not.toContain("This window fits");
    expect(menu({ count: 2, requested: 2, max: 2 })).not.toContain("Showing");
    expect(menu({ count: 2, arrangement: "columns", max: 4 })).not.toMatch(/aria-disabled="true"[^>]*aria-label="\d panes?"/);
  });
});

describe("LayoutMenu", () => {
  it("is a named dialog with the pane count as a radio group of one to four", () => {
    const h = menu({ count: 3, arrangement: "main-left" });
    expect(h).toContain('role="dialog"');
    expect(h).toContain('aria-label="Split layout"');
    expect(h).toContain('role="radiogroup"');
    for (const n of [1, 2, 3, 4]) expect(h).toContain(`aria-label="${n} ${n === 1 ? "pane" : "panes"}"`);
    expect(count(h, /aria-checked="true"[^>]*aria-label="\d panes?"/g)).toBe(1);
    expect(h).toMatch(/aria-checked="true"[^>]*aria-label="3 panes"/);
  });
  it("offers only the arrangements that suit the count, with the current one checked", () => {
    const h = menu({ count: 3, arrangement: "main-top" });
    for (const label of ["One main, two beside", "One main on top", "Side by side", "Stacked"]) expect(h).toContain(`aria-label="${label}"`);
    expect(h).not.toContain('aria-label="Grid"');
    expect(h).toMatch(/aria-checked="true"[^>]*aria-label="One main on top"/);
    expect(menu({ count: 4, arrangement: "grid" })).toContain('aria-label="Grid"');
  });
  it("has nothing to arrange for one pane, and says how to add another", () => {
    const h = menu({ count: 1, arrangement: "single" });
    expect(h).not.toContain('aria-label="Side by side"');
    expect(h).toContain("Add a pane");
    expect(h).toContain("Ctrl+Shift+\\");
  });
  it("lets the user hand the arrangement back to the app, and says what automatic does", () => {
    const h = menu();
    expect(h).toMatch(/role="checkbox"[^>]*aria-checked="true"/);
    expect(h).toContain("Automatic");
    expect(h).toContain("picks the layout that suits the window");
    expect(menu({ auto: false })).toMatch(/role="checkbox"[^>]*aria-checked="false"/);
  });
  it("lists zoom and moving between panes with their shortcuts", () => {
    const h = menu();
    expect(h).toContain("Zoom the active pane");
    expect(h).toContain("Ctrl+Shift+Z");
    expect(h).toContain("Ctrl+Shift+]");
    expect(menu({ os: "mac" })).toContain("⌘Z");
  });
  it("puts no literal colour in the markup", () => {
    expect(menu()).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});
