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
  it("colours the tile and strokes through custom properties, so the mark is themed in one place", () => {
    const h = renderToStaticMarkup(<Logo />);
    expect(h).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(h).toContain("var(--logo-tile)");
  });
});
