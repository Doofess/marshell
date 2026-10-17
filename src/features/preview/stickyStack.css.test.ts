import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const read = (name: string) => readFileSync(new URL(name, import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const preview = read("./Preview.css");
const pdf = read("./pdf/PdfView.css");

/** The declarations of the first rule whose selector list contains exactly `selector`. */
const rule = (css: string, selector: string) => {
  for (const m of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (m[1]!.split(",").some((s) => s.trim().replace(/\s+/g, " ") === selector)) return m[2]!;
  }
  return "";
};

describe("the deleted notice and the PDF toolbar", () => {
  it("both stick at the top of the body, so the toolbar must start below the notice", () => {
    expect(rule(preview, ".preview__notice")).toMatch(/position:\s*sticky/);
    expect(rule(pdf, ".pdf__bar")).toMatch(/position:\s*sticky/);
    const below = rule(pdf, ".preview__body:has(.preview__notice) .pdf__bar");
    expect(below).toMatch(/inset-block-start:\s*var\(--notice-h\)/);
  });
  it("the notice has the height the toolbar leaves room for", () => {
    expect(rule(preview, ".preview__notice")).toMatch(/block-size:\s*var\(--notice-h\)/);
    expect(rule(preview, ".preview__body")).toMatch(/--notice-h:\s*\d+px/);
  });
});
