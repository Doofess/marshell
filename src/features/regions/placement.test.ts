import { describe, expect, it } from "vitest";
import { placePopover } from "./placement";

const bounds = { left: 0, top: 0, width: 400, height: 300 };

describe("placePopover", () => {
  it("goes below the box when it fits", () => {
    expect(placePopover({ left: 50, top: 40, width: 100, height: 60 }, { w: 200, h: 80 }, bounds)).toEqual({ left: 50, top: 108, side: "below" });
  });
  it("goes above when below does not fit, and is clamped in from the right", () => {
    expect(placePopover({ left: 300, top: 200, width: 90, height: 60 }, { w: 200, h: 80 }, bounds)).toEqual({ left: 200, top: 112, side: "above" });
  });
  it("takes the side with more room when neither fits, kept inside the bounds", () => {
    const p = placePopover({ left: 10, top: 40, width: 100, height: 200 }, { w: 100, h: 90 }, bounds);
    expect(p.side).toBe("below");
    expect(p.top).toBeLessThanOrEqual(300 - 90);
  });
});
