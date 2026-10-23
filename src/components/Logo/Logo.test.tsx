import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Logo } from "./Logo";

const source = readFileSync(new URL("../../../assets/icon/app-night-shift.svg", import.meta.url), "utf8");
const num = (s: string, name: string) => [...s.matchAll(new RegExp(`${name}="(-?\\d+)"`, "g"))].map((m) => m[1]);

describe("Logo", () => {
  it("is the signed-off Night shift icon: same tile and same four strokes as assets/icon", () => {
    const h = renderToStaticMarkup(<Logo />);
    expect(h).toContain('viewBox="0 0 1024 1024"');
    for (const a of ["x1", "y1", "x2", "y2"]) expect(num(h, a), a).toEqual(num(source, a));
    expect(num(h, "rx")).toEqual(num(source, "rx"));
  });
  it("is an image named Marshell by default", () => {
    const h = renderToStaticMarkup(<Logo />);
    expect(h).toContain('role="img"');
    expect(h).toContain('aria-label="Marshell"');
  });
  it("hides from assistive tech when it sits inside a control that already has a name", () => {
    const h = renderToStaticMarkup(<Logo decorative />);
    expect(h).toContain('aria-hidden="true"');
    expect(h).not.toContain("aria-label");
  });
  it("takes its size from the prop", () => {
    expect(renderToStaticMarkup(<Logo size={22} />)).toContain('width="22"');
  });
  it("is the Daylight icon in the light theme and Night shift in the dark one", () => {
    const css = readFileSync(new URL("./Logo.css", import.meta.url), "utf8");
    const pair = (name: string) => new RegExp(`--logo-${name}:\\s*light-dark\\(([^,]+),\\s*([^;]+)\\);`).exec(css)?.slice(1, 3);
    // alt-daylight.svg: #eef0f3, #22252a, #f26a0c. app-night-shift.svg: #16181c, #d9dde3, #ff7a1a.
    expect(pair("tile")).toEqual(["oklch(95.4% 0.005 258)", "oklch(21% 0.008 265)"]);
    expect(pair("hand")).toEqual(["oklch(26.4% 0.01 261)", "oklch(89% 0.006 255)"]);
    expect(pair("signal")).toEqual(["oklch(68.2% 0.188 46)", "oklch(72% 0.19 55)"]);
  });
  it("colours the tile and strokes through custom properties, so the mark is themed in one place", () => {
    const h = renderToStaticMarkup(<Logo />);
    expect(h).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(h).toContain("var(--logo-tile)");
  });
});
