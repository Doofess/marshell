import { describe, expect, it } from "vitest";
import { navLayoutOf, type Measured } from "./navLayout";

const ORIGIN = { x: 10, y: 20, w: 1000, h: 700 };
const at = (attrs: Measured["attrs"], x: number, y: number, w = 100, h = 50): Measured => ({ attrs, rect: { x, y, w, h } });

describe("navLayoutOf", () => {
  it("turns panes, rows and top controls into targets, with rects relative to the window", () => {
    const layout = navLayoutOf(
      [at({ pane: "0" }, 310, 60, 300, 300), at({ pane: "1" }, 610, 60, 300, 300), at({ row: "1" }, 10, 60), at({ row: "2" }, 10, 110), at({ row: "5" }, 10, 160), at({ top: "palette" }, 140, 20, 140, 40)],
      ORIGIN,
    );
    expect(layout).toEqual([
      { target: { kind: "pane", index: 0 }, rect: { x: 300, y: 40, w: 300, h: 300 } },
      { target: { kind: "pane", index: 1 }, rect: { x: 600, y: 40, w: 300, h: 300 } },
      { target: { kind: "row", number: 1 }, rect: { x: 0, y: 40, w: 100, h: 50 } },
      { target: { kind: "row", number: 2 }, rect: { x: 0, y: 90, w: 100, h: 50 } },
      { target: { kind: "row", number: 5 }, rect: { x: 0, y: 140, w: 100, h: 50 } },
      { target: { kind: "top", id: "palette" }, rect: { x: 130, y: 0, w: 140, h: 40 } },
    ]);
  });
  it("drops what is not drawn: a 0 by 0 row is out of sight (scrolled away, or display: none)", () => {
    expect(navLayoutOf([at({ row: "3" }, 0, 0, 0, 0)], ORIGIN)).toEqual([]);
  });
  it("drops a row scrolled out of its list: it is not in sight, so the cursor cannot land there", () => {
    const list = { x: 10, y: 60, w: 288, h: 200 };
    expect(navLayoutOf([{ ...at({ row: "9" }, 10, 400), clip: list }], ORIGIN)).toEqual([]);
    expect(navLayoutOf([{ ...at({ row: "1" }, 10, 60), clip: list }], ORIGIN)).toHaveLength(1);
  });
  it("drops a row whose number is not a number", () => {
    expect(navLayoutOf([at({ row: "x" }, 10, 60)], ORIGIN)).toEqual([]);
  });
});
