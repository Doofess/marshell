import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { filterKey, TreeFilter } from "./TreeFilterBox";

describe("TreeFilter", () => {
  it("is a box named Filter files with the plan's placeholder", () => {
    const out = renderToStaticMarkup(<TreeFilter value="ab" onChange={() => {}} onEscape={() => {}} onDown={() => {}} />);
    expect(out).toContain('aria-label="Filter files"');
    expect(out).toContain('placeholder="Filter, or ? to search in files"');
    expect(out).toContain('value="ab"');
  });

  it("Esc clears a box with text and leaves an empty one; Down goes to the rows", () => {
    expect(filterKey("Escape", "ab")).toBe("clear");
    expect(filterKey("Escape", "")).toBe("escape");
    expect(filterKey("ArrowDown", "ab")).toBe("down");
    expect(filterKey("a", "ab")).toBeNull();
  });
  it("has a search mark the reader never hears, drawn after the box so it paints over it", () => {
    const html = renderToStaticMarkup(<TreeFilter value="" onChange={() => {}} onEscape={() => {}} onDown={() => {}} />);
    expect(html).toMatch(/<div class="file-tree-filter-box"><input class="file-tree-filter"[^>]*aria-label="Filter files"[^>]*><svg class="file-tree-filter__mark"[^>]*aria-hidden="true"/);
  });
});
