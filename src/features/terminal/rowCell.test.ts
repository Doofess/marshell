import { describe, expect, it } from "vitest";
import { rowCell, sameCell } from "./rowCell";

describe("rowCell", () => {
  it("is the screen's height over its rows, with the padding beside it", () => {
    expect(rowCell(340, 20, 8, 1)).toEqual({ height: 17, pad: 8 });
  });
  it("keeps a row that is a fraction of a pixel tall at a fractional display scale", () => {
    expect(rowCell(352, 20, 0, 1.25)).toEqual({ height: 17.6, pad: 0 });
  });
  it("is a whole number of device pixels, whatever the screen's rounded height says", () => {
    // 11 rows of 17.6 px are 193.6 px, and the screen reports 194: the row is still 22 device pixels at 125%.
    expect(rowCell(194, 11, 0, 1.25)).toEqual({ height: 17.6, pad: 0 });
    // 12 rows are 211.2 px, reported as 211.
    expect(rowCell(211, 12, 0, 1.25)).toEqual({ height: 17.6, pad: 0 });
    expect(rowCell(187, 11, 16, 1)).toEqual({ height: 17, pad: 16 });
  });
  it("takes a scale it can't use as 1", () => {
    expect(rowCell(340, 20, 8, 0)).toEqual({ height: 17, pad: 8 });
    expect(rowCell(340, 20, 8, Number.NaN)).toEqual({ height: 17, pad: 8 });
  });
  it("is null until the terminal has rows and a height", () => {
    expect(rowCell(0, 20, 8, 1)).toBeNull();
    expect(rowCell(340, 0, 8, 1)).toBeNull();
    expect(rowCell(Number.NaN, 20, 8, 1)).toBeNull();
    // Too little to be one device pixel a row.
    expect(rowCell(2, 20, 8, 1)).toBeNull();
  });
  it("counts padding it can't read as none", () => {
    expect(rowCell(340, 20, Number.NaN, 1)).toEqual({ height: 17, pad: 0 });
  });
  it("reports only a change", () => {
    expect(sameCell({ height: 17, pad: 8 }, { height: 17, pad: 8 })).toBe(true);
    expect(sameCell({ height: 17, pad: 8 }, { height: 18, pad: 8 })).toBe(false);
    expect(sameCell({ height: 17, pad: 8 }, { height: 17, pad: 0 })).toBe(false);
    expect(sameCell(null, { height: 17, pad: 8 })).toBe(false);
    expect(sameCell(null, null)).toBe(true);
  });
});
