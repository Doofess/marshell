import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("./FilesBody.css", import.meta.url), "utf8");
const forced = () => {
  const i = css.indexOf("@media (forced-colors: active)");
  if (i < 0) return "";
  let depth = 0;
  for (let j = css.indexOf("{", i); j < css.length; j++) {
    if (css[j] === "{") depth++;
    else if (css[j] === "}" && --depth === 0) return css.slice(i, j + 1);
  }
  return "";
};

describe("FilesBody.css", () => {
  it("has no hex colour", () => {
    expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
  });
  it("names only system colours in the forced-colors block", () => {
    const fc = forced();
    expect(fc).toMatch(/Highlight/);
    expect(fc).not.toMatch(/var\(/);
  });
  it("draws no scrollbar and no outline: none", () => {
    expect(css).toContain("scrollbar-width: none");
    expect(css).not.toMatch(/outline:\s*none/);
  });
  it("gives the divider a focus style and keeps hover in the fine-pointer query", () => {
    expect(css).toContain(".files-body__divider:focus-visible");
    const outside = css.replace(/@media [^{]*\(hover: hover\) and \(pointer: fine\) \{[\s\S]*?\n\}\n/g, "");
    expect(outside).not.toContain(":hover");
  });
  it("follows the writing direction as it is and uses no physical sides", () => {
    // The row already runs right to left there, and the DOM is in reading order: a reversed row would read backwards.
    expect(css).not.toContain("row-reverse");
    expect(css).not.toMatch(/\b(left|right|top|bottom)\s*:/);
  });
});
