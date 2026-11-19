import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { colorMenuKey } from "./ColorMenu";
import { SelectionBar, selectionBarShows } from "./SelectionBar";

describe("selectionBarShows", () => {
  it("selectionBarShows_never_while_notes_are_blocked", () => {
    // A read-only file keeps its editor (its unsaved notes stay on screen), but its tools are off, the bar with them.
    expect(selectionBarShows(true, "none", "a.pdf is read-only.")).toBe(false);
    expect(selectionBarShows(true, "freetext", "a.pdf is read-only.")).toBe(false);
  });
  it("selectionBarShows_not_before_the_verdict_or_the_editor", () => {
    expect(selectionBarShows(true, "none", undefined)).toBe(false);
    expect(selectionBarShows(false, "none", null)).toBe(false);
  });
  it("selectionBarShows_with_no_tool_or_free_text_but_not_with_highlight", () => {
    expect(selectionBarShows(true, "none", null)).toBe(true);
    expect(selectionBarShows(true, "freetext", null)).toBe(true);
    expect(selectionBarShows(true, "highlight", null)).toBe(false);
  });
});

describe("colorMenuKey", () => {
  it("colorMenuKey_down_and_up_move_and_wrap", () => {
    expect(colorMenuKey("ArrowDown", 0, 5)).toEqual({ type: "focus", index: 1 });
    expect(colorMenuKey("ArrowDown", 4, 5)).toEqual({ type: "focus", index: 0 });
    expect(colorMenuKey("ArrowUp", 2, 5)).toEqual({ type: "focus", index: 1 });
    expect(colorMenuKey("ArrowUp", 0, 5)).toEqual({ type: "focus", index: 4 });
  });
  it("colorMenuKey_home_and_end", () => {
    expect(colorMenuKey("Home", 3, 5)).toEqual({ type: "focus", index: 0 });
    expect(colorMenuKey("End", 1, 5)).toEqual({ type: "focus", index: 4 });
  });
  it("colorMenuKey_enter_picks_and_escape_closes", () => {
    expect(colorMenuKey("Enter", 2, 5)).toEqual({ type: "pick", index: 2 });
    expect(colorMenuKey("Escape", 2, 5)).toEqual({ type: "close" });
  });
  it("colorMenuKey_other_keys_are_not_taken", () => {
    expect(colorMenuKey("a", 0, 5)).toBeNull();
    expect(colorMenuKey("Tab", 0, 5)).toBeNull();
  });
});

describe("SelectionBar markup", () => {
  const noop = () => {};
  const html = renderToStaticMarkup(<SelectionBar color="#53FFBC" onColor={noop} onHighlight={noop} onComment={noop} onClose={noop} />);
  it("is_a_toolbar_named_selected_text", () => {
    expect(html).toContain('role="toolbar"');
    expect(html).toContain('aria-label="Selected text"');
  });
  it("has_highlight_the_colour_menu_and_comment_in_order", () => {
    const at = (s: string) => html.indexOf(s);
    const order = [">Highlight</button>", 'aria-label="Highlight colour: Green"', ">Comment</button>"].map(at);
    expect(order.every((i) => i >= 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
  });
  it("the_colour_button_is_a_closed_menu_button", () => {
    expect(html).toContain('aria-haspopup="menu"');
    expect(html).toContain('aria-expanded="false"');
    expect(html).not.toContain('role="menu"');
  });
});
