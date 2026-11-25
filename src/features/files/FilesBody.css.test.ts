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

const rule = (sel: string) => {
  const i = css.indexOf(`${sel} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("FilesBody.css", () => {
  it("an empty tools place takes no room, and a full one never shrinks", () => {
    expect(rule(".path-bar__tools")).toContain("flex: none");
    // The "Updated" note is always drawn, so `:empty` would never match (review 4).
    expect(rule(".path-bar__tools:not(:has(> :not(.preview__updated)))")).toContain("display: none");
  });
  it("a path longer than the bar scrolls: its parts never shrink into each other (live check 6w: the names overlapped at 320 px)", () => {
    // The app's reset gives every flex item a zero minimum, so without this the folders shrink and their text runs over the next one.
    expect(rule(".path-bar__path > *")).toContain("flex: none");
  });
  it("the file's cell is a grid, so the tab panel inside fills it (live check 6r-2: a block let the preview's body collapse to 0 px)", () => {
    const rule = css.slice(css.indexOf(".files-body__content {"), css.indexOf("}", css.indexOf(".files-body__content {")));
    expect(rule).toContain("display: grid");
    expect(rule).toContain("grid-template-rows: minmax(0, 1fr)");
  });
  it("lays the narrow tree over the file in one cell, with no absolute placement", () => {
    expect(rule(".files-body[data-narrow] .files-body__main")).toContain("display: grid");
    expect(rule(".files-body[data-narrow] .files-body__main > *")).toContain("grid-area: 1 / 1");
    // The file holds positioned boxes (`.preview__body`, CodeMirror's `.cm-editor` and `.cm-scroller`), which would paint over a tree with no z-index (review 1).
    expect(rule(".files-body[data-narrow] .files-body__tree")).toContain("z-index: 1");
    expect(rule(".files-body[data-narrow] .files-body__tree")).not.toContain("position");
    expect(rule(".files-body")).not.toContain("position: relative");
  });
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
  it("is two rows: the path bar, then the tree and the file", () => {
    expect(rule(".files-body")).toContain("grid-template-rows: auto minmax(0, 1fr)");
    expect(rule(".files-body__main")).toContain("display: flex");
  });
  it("draws one line under the path bar, across the panel", () => {
    expect(rule(".path-bar")).toContain("border-block-end: 1px solid var(--hairline)");
  });
  it("never shrinks the path bar's buttons: the path gives way and scrolls", () => {
    // The first `.path-bar__button` block is shared with `.path-bar__folder`; the button's own block follows it (review 5).
    expect(css).toMatch(/\n\.path-bar__button \{[^}]*flex: none;/);
    expect(rule(".path-bar__path")).toContain("min-inline-size: 0");
  });
  it("leaves a gutter between the code and a tree that follows it", () => {
    expect(css).toContain('.files-body[data-edge="end"][data-tree]:not([data-narrow]) .files-body__content .cm-scroller');
    expect(css).toMatch(/\.cm-scroller \{\s*margin-inline-end: var\(--space-2\);/);
  });
});
