import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Sidebar.css", import.meta.url), "utf8");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("Sidebar.css", () => {
  it("lets the session list scroll with nothing drawn, and not leak the scroll to the page", () => {
    const r = rule(".sidebar__list");
    expect(r).toMatch(/overflow-y:\s*auto/);
    expect(r).toMatch(/scrollbar-width:\s*none/);
    expect(r).toMatch(/overscroll-behavior:\s*contain/);
    expect(rule(".sidebar__list::-webkit-scrollbar")).toContain("display: none");
  });
  it("fades an edge only where there is more to scroll to, driven by the scroll position", () => {
    expect(css).toContain("@property --fade-start");
    expect(css).toContain("@property --fade-end");
    expect(css).toMatch(/animation-timeline:\s*scroll\(self block\)/);
    expect(css).toMatch(/mask-image:\s*linear-gradient\(to bottom,[^;]*var\(--fade-start\)[^;]*var\(--fade-end\)/);
    // A list that fits has no scroll timeline, so both lengths stay at their initial 0.
    expect(css).toMatch(/initial-value:\s*0px/);
  });
  it("applies the fade only where scroll-driven animation exists", () => {
    expect(css).toContain("@supports (animation-timeline: scroll())");
  });
  it("gives the grid a fourth row for the shell panel, sized by one custom property", () => {
    expect(rule(".sidebar[data-shell]")).toMatch(/grid-template-rows:\s*auto minmax\(0, 1fr\) var\(--shell-h\) var\(--footer-h, 40px\)/);
  });
  it("no longer has the overflow button's styles", () => {
    expect(css).not.toContain("sidebar__more");
  });
  it("uses tokens and logical properties only", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
  });
});
