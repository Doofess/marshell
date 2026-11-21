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
});
