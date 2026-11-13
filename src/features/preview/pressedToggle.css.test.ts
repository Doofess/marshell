import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

/** The text of the block opened by the first `@media <query>` in `css`, braces balanced. */
const media = (css: string, query: string) => {
  const i = css.indexOf(`@media ${query}`);
  if (i < 0) return "";
  let depth = 0;
  for (let j = css.indexOf("{", i); j < css.length; j++) {
    if (css[j] === "{") depth++;
    else if (css[j] === "}" && --depth === 0) return css.slice(i, j + 1);
  }
  return "";
};
/** The body of the rule whose selector list ends with `selector {`, or "" when there is none. */
const rule = (block: string, selector: string) => {
  const i = block.indexOf(`${selector} {`);
  return i < 0 ? "" : block.slice(i, block.indexOf("}", i));
};

// Read from disk: Vitest hands CSS imports to tests as empty strings.
describe.each([
  { file: "./Preview.css", button: ".preview__view" },
  { file: "./pdf/PdfView.css", button: ".pdf__button" },
])("$button's pressed state in forced colours ($file)", ({ file, button }) => {
  const fc = media(readFileSync(new URL(file, import.meta.url), "utf8"), "(forced-colors: active)");
  const pressed = `${button}[aria-pressed="true"]`;

  it("is a Highlight fill, so the focus ring is free to show on top of it", () => {
    // An outline here took the place of the focus ring: a focused pressed button looked like an unfocused one.
    const body = rule(fc, pressed);
    expect(body).toContain("background: Highlight");
    expect(body).toContain("color: HighlightText");
    // Without it the system paints a Canvas backplate behind the label, and HighlightText on Canvas can vanish.
    expect(body).toContain("forced-color-adjust: none");
    expect(body).not.toContain("outline");
  });
  it("rings a focused pressed button in HighlightText, set in so the fill shows on both sides of the ring", () => {
    const body = rule(fc, `${pressed}:focus-visible`);
    expect(body).toContain("outline-color: HighlightText");
    expect(body).toContain("outline-offset: -4px");
  });
});
