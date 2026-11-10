import { describe, expect, it } from "vitest";
import { columnsWidth, terminalFloorWidth } from "./columns";

const font = { family: "monospace", size: 13 };

describe("columnsWidth", () => {
  it("a cell is whole device pixels, as xterm's WebGL renderer draws it: 80 columns of a 7.8 px glyph are 560 px at 100%", () => {
    expect(columnsWidth(80, font, () => 7.8, 1)).toBeCloseTo(560);
  });
  it("at 125% the 7.8 px glyph has 9 device pixels, so 80 columns are 576 px; at 150% it has 11, so they are 586.67 px", () => {
    expect(columnsWidth(80, font, () => 7.8, 1.25)).toBeCloseTo(576);
    expect(columnsWidth(80, font, () => 7.8, 1.5)).toBeCloseTo(586.667, 2);
  });
  it("a glyph that is already whole pixels keeps its width", () => {
    expect(columnsWidth(80, { family: "monospace", size: 14 }, () => 8, 1)).toBeCloseTo(640);
    expect(columnsWidth(80, { family: "monospace", size: 14 }, () => 8.4, 2.5)).toBeCloseTo(672);
  });
  it("without a measure the cell is 0.6 of the size", () => {
    expect(columnsWidth(80, { family: "monospace", size: 10 })).toBeCloseTo(480);
  });
  it("a scale that isn't a positive number counts as 100%", () => {
    expect(columnsWidth(80, font, () => 7.8, 0)).toBeCloseTo(560);
    expect(columnsWidth(80, font, () => 7.8, Number.NaN)).toBeCloseTo(560);
  });
});

describe("terminalFloorWidth", () => {
  it("adds the host's 16 px padding and the 14 px scrollbar to the columns: 590 px for 80 columns of the 7.8 px glyph", () => {
    expect(terminalFloorWidth(80, font, () => 7.8, 1)).toBe(560 + 16 + 14);
  });
  it("rounds the columns up", () => {
    expect(terminalFloorWidth(80, font, () => 7.8, 1.5)).toBe(587 + 16 + 14);
  });
});
