import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ViewportChip } from "./ViewportChip";

describe("ViewportChip", () => {
  it("is a button that names the size and opens a menu", () => {
    const html = renderToStaticMarkup(<ViewportChip id="desktop" slot={360} onOpen={() => {}} />);
    expect(html).toContain('aria-haspopup="menu"');
    expect(html).toContain('aria-label="Viewport: Desktop, 1440 pixels wide, shown at 25%"');
    expect(html).toContain("Desktop 1440 · 25%");
    expect(html).toContain('title="Change the viewport"');
    expect(html).toContain('type="button"');
  });
  it("shows no percentage when the page fits", () => {
    const html = renderToStaticMarkup(<ViewportChip id="phone" slot={800} onOpen={() => {}} />);
    expect(html).toContain(">Phone 390<");
  });
});
