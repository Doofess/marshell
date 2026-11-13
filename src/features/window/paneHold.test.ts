import { describe, expect, it } from "vitest";
import { HOLD_CAP_MS, holdEnds, holdOnChange } from "./paneHold";

describe("paneHold", () => {
  it("a hide holds while a page is on screen", () => {
    expect(holdOnChange(false, true)).toBe(true);
  });

  it("a hide with no page on screen doesn't hold", () => {
    expect(holdOnChange(false, false)).toBe(false);
  });

  it("a show ends any hold at once", () => {
    expect(holdOnChange(true, true)).toBe(false);
    expect(holdOnChange(true, false)).toBe(false);
  });

  it("the hold ends when the freeze has hidden the page", () => {
    expect(holdEnds(false, 10)).toBe(true);
  });

  it("the hold ends at the cap", () => {
    expect(HOLD_CAP_MS).toBe(150);
    expect(holdEnds(true, 149)).toBe(false);
    expect(holdEnds(true, 150)).toBe(true);
  });
});
