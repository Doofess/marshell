import { describe, expect, it } from "vitest";
import { columnsWidth, terminalFloorWidth } from "./columns";

describe("columnsWidth", () => {
  it("80 columns of a 14 px font with an 8.4 px cell are 672 px", () => {
    expect(columnsWidth(80, { family: "monospace", size: 14 }, () => 8.4)).toBeCloseTo(672);
  });
  it("without a measure the cell is 0.6 of the size", () => {
    expect(columnsWidth(80, { family: "monospace", size: 10 })).toBeCloseTo(480);
  });
});

describe("terminalFloorWidth", () => {
  it("adds the host's 16 px padding and the 14 px scrollbar to the columns, rounded up", () => {
    expect(terminalFloorWidth(80, { family: "monospace", size: 14 }, () => 8.41)).toBe(673 + 16 + 14);
  });
});
