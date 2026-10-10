import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { selectedProps } from "./SelectedMark";

describe("selectedProps", () => {
  it("options get aria-selected true or false and the shared class", () => {
    expect(renderToStaticMarkup(<li {...selectedProps(true, "selected", "palette__opt")} />)).toBe('<li class="palette__opt is-selected-mark" aria-selected="true"></li>');
    expect(renderToStaticMarkup(<li {...selectedProps(false)} />)).toBe('<li class="is-selected-mark" aria-selected="false"></li>');
  });
  it("list-of-places items get aria-current only while current", () => {
    expect(renderToStaticMarkup(<button {...selectedProps(true, "current", "rail__item")} />)).toBe('<button class="rail__item is-selected-mark" aria-current="true"></button>');
    expect(renderToStaticMarkup(<button {...selectedProps(false, "current")} />)).toBe('<button class="is-selected-mark"></button>');
  });
});
