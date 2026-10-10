import { describe, expect, it } from "vitest";
import { MORE_HEIGHT, listLayout } from "./listLayout";

describe("listLayout: rows that fit, and one 36 px slot for the rest", () => {
  it("shows everything, with no button, when it all fits", () => {
    expect(listLayout(5, 240, 48)).toEqual({ visible: 5, hidden: 0 });
  });
  it("shows nothing and hides nothing for an empty list", () => {
    expect(listLayout(0, 500, 48)).toEqual({ visible: 0, hidden: 0 });
  });
  it("fits exactly: no button needed when the last row ends on the edge", () => {
    expect(listLayout(10, 480, 48)).toEqual({ visible: 10, hidden: 0 });
  });
  it("one row too many: reserves the button's slot, which costs a whole row of space only if the slot does not fit beside the rows", () => {
    // 11 rows of 48 need 528; in 480 the button takes 36, leaving room for floor(444 / 48) = 9 rows.
    expect(listLayout(11, 480, 48)).toEqual({ visible: 9, hidden: 2 });
  });
  it("always leaves room for the button when it is shown", () => {
    for (const count of [1, 3, 8, 20, 50])
      for (const area of [0, 36, 100, 240, 480, 900]) {
        const l = listLayout(count, area, 48);
        expect(l.visible + l.hidden, `${count} in ${area}`).toBe(count);
        if (l.hidden > 0) expect(l.visible * 48 + MORE_HEIGHT, `${count} in ${area}`).toBeLessThanOrEqual(Math.max(area, MORE_HEIGHT));
        else expect(l.visible * 48, `${count} in ${area}`).toBeLessThanOrEqual(Math.max(area, 0) || 0 + count * 48);
      }
  });
  it("hides everything behind the button when not even one row fits", () => {
    expect(listLayout(4, 40, 48)).toEqual({ visible: 0, hidden: 4 });
  });
  it("copes with nonsense", () => {
    for (const area of [0, -10, Number.NaN, Number.POSITIVE_INFINITY * 0])
      expect(listLayout(3, area, 48)).toEqual({ visible: 0, hidden: 3 });
    expect(listLayout(-2, 500, 48)).toEqual({ visible: 0, hidden: 0 });
    expect(listLayout(3, 500, 0)).toEqual({ visible: 0, hidden: 3 });
  });
  it("uses a 36 px button unless told otherwise", () => {
    expect(MORE_HEIGHT).toBe(36);
    expect(listLayout(3, 100, 40, 20)).toEqual({ visible: 2, hidden: 1 });
  });
});
