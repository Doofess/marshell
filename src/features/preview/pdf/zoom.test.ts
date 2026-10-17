import { describe, expect, it } from "vitest";
import { parsePageField, scaleFor, zoomIn, zoomOut } from "./zoom";

describe("zoom", () => {
  it("fit width fills the available width; a fixed zoom is its own value", () => {
    expect(scaleFor({ kind: "fit" }, 612, 900)).toBeCloseTo(900 / 612);
    expect(scaleFor({ kind: "scale", value: 1.5 }, 612, 900)).toBe(1.5);
  });
  it("steps from the current scale to the next stop, and stops at the ends", () => {
    expect(zoomIn({ kind: "fit" }, 1.1)).toEqual({ kind: "scale", value: 1.25 });
    expect(zoomOut({ kind: "fit" }, 1.1)).toEqual({ kind: "scale", value: 1 });
    expect(zoomIn({ kind: "scale", value: 3 }, 3)).toEqual({ kind: "scale", value: 3 });
    expect(zoomOut({ kind: "scale", value: 0.5 }, 0.5)).toEqual({ kind: "scale", value: 0.5 });
  });
  it("reads a page number, and refuses anything out of range or not a number", () => {
    expect(parsePageField(" 3 ", 12)).toBe(3);
    expect(parsePageField("0", 12)).toBeNull();
    expect(parsePageField("13", 12)).toBeNull();
    expect(parsePageField("2a", 12)).toBeNull();
  });
});
