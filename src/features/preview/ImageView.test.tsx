import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ImageView, imageBoxStyle } from "./ImageView";

const css = readFileSync(new URL("./Preview.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
/** The declarations of the first rule whose selector list contains exactly `selector`. */
const rule = (selector: string) => {
  for (const m of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (m[1]!.split(",").some((s) => s.trim().replace(/\s+/g, " ") === selector)) return m[2]!;
  }
  return "";
};

// Why: the image is laid out in a box that must be exactly as big as the image it shows, because a region is stored as fractions of
// that box and cropped as fractions of the image. A percentage max-height on an image in a box of auto height does not limit
// the image (the box clamps and the image overflows it), so a tall or large image broke the match. Checked in Chromium:
// the box was 348 px high around a 3000 px image, and 561 x 348 around a 561 x 421 one.
describe("ImageView", () => {
  it("wraps the page box in a cell-filling frame the box is sized against", () => {
    const out = renderToStaticMarkup(<ImageView url="blob:x" name="p.png" onSize={() => {}} />);
    expect(out).toMatch(/<div class="preview__imagefit"><div class="preview__imagebox" data-page="1">/);
  });
  it("hands the loaded size to the box as two numbers", () => {
    expect(imageBoxStyle(null)).toBeUndefined();
    expect(imageBoxStyle({ w: 400, h: 3000 })).toEqual({ "--nat-w": 400, "--nat-h": 3000 });
  });
});

describe("the image box CSS", () => {
  it("the frame is a size container that fills its grid cell", () => {
    const r = rule(".preview__imagefit");
    expect(r).toMatch(/container-type:\s*size/);
    expect(r).toMatch(/justify-self:\s*stretch/);
    expect(r).toMatch(/align-self:\s*stretch/);
  });
  it("once the size is known, the box is the image's size, fitted to the frame in both directions with the ratio kept", () => {
    const r = rule('.preview__imagebox[data-fit="true"]');
    expect(r).toMatch(/aspect-ratio:\s*var\(--nat-w\)\s*\/\s*var\(--nat-h\)/);
    expect(r).toMatch(/100cqw/);
    expect(r).toMatch(/100cqh/);
    expect(r).toMatch(/var\(--nat-w\)\s*\*\s*1px/);
    expect(rule('.preview__imagebox[data-fit="true"] .preview__image')).toMatch(/inline-size:\s*100%[\s\S]*block-size:\s*100%/);
  });
});
