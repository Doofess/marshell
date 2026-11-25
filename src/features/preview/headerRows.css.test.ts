import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("./Preview.css", import.meta.url), "utf8");

describe("Preview.css header rows", () => {
  it("a preview with no header of its own gives the body the whole height (review 2)", () => {
    expect(css).toMatch(/\.preview:not\(:has\(> \.preview__header\)\) \{\s*grid-template-rows: minmax\(0, 1fr\);/);
    expect(css).toMatch(/\.preview:not\(:has\(> \.preview__header\)\):has\(\.web-link\) \{\s*grid-template-rows: auto minmax\(0, 1fr\);/);
  });
});
