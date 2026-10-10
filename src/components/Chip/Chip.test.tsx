import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Chip } from "./Chip";

describe("Chip", () => {
  it("renders text with a tone", () => {
    const h = renderToStaticMarkup(<Chip tone="caution">Bypass</Chip>);
    expect(h).toContain("Bypass");
    expect(h).toContain('data-tone="caution"');
  });
  it("defaults to neutral", () => {
    expect(renderToStaticMarkup(<Chip>Plan</Chip>)).toContain('data-tone="neutral"');
  });
});
