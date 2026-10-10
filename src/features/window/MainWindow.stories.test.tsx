import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import * as stories from "./MainWindow.stories";

const render = (name: keyof typeof stories) => renderToStaticMarkup(((stories[name] as unknown as { render: () => React.ReactElement }).render)());
/** Each story shows a dark and a light window, so every count is doubled. */
const panes = (name: keyof typeof stories) => (render(name).match(/<section class="pane/g) ?? []).length / 2;

describe("the split stories show what their captions say", () => {
  it("two, three and four panes where the window has room", () => {
    expect(panes("SplitTwo")).toBe(2);
    expect(panes("SplitThree")).toBe(3);
    expect(panes("SplitFour")).toBe(4);
    expect(panes("SplitStackedTall")).toBe(3);
    expect(panes("SplitNarrowWarning")).toBe(3);
  });
  it("one pane where four were asked for in the minimum window, with the menu open and saying why", () => {
    expect(panes("SplitFourTooSmall")).toBe(1);
    const h = render("SplitFourTooSmall");
    expect(h).toContain("Showing 1 of 4");
    expect(h).toContain("3 more running");
  });
  it("two panes where three were asked for in a 1180 by 800 window, stacked", () => {
    expect(panes("SplitThreeCappedAtTwo")).toBe(2);
    expect(render("SplitThreeCappedAtTwo")).toContain('data-arrangement="rows"');
    expect(render("SplitThreeCappedAtTwo")).toContain("1 more running");
  });
  it("a narrow-pane warning where a pane is 60 to 79 columns wide", () => {
    expect(render("SplitNarrowWarning")).toMatch(/\d+×\d+/);
  });
  it("zoomed shows one, the empty pane story shows an empty slot", () => {
    expect(panes("SplitZoomed")).toBe(1);
    expect(render("SplitEmptyPane")).toContain("pane--empty");
  });
});
