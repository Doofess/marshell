import { describe, expect, it } from "vitest";
import { cropRect, cropScale, outputSize } from "./crop";

describe("crop", () => {
  it("renders a page at up to 3x, less when it would be too many pixels", () => {
    expect(cropScale({ w: 612, h: 792 })).toBe(3);
    expect(cropScale({ w: 10_000, h: 10_000 })).toBeCloseTo(Math.sqrt(16_777_216 / 1e8));
  });

  it("turns a region into whole source pixels, at least one", () => {
    expect(cropRect({ page: 1, x: 0.25, y: 0.5, w: 0.5, h: 0.0001 }, { w: 200, h: 100 })).toEqual({ sx: 50, sy: 50, sw: 100, sh: 1 });
  });

  it("caps the longest side at 4096, keeping the ratio", () => {
    expect(outputSize(8000, 2000)).toEqual({ w: 4096, h: 1024 });
    expect(outputSize(300, 200)).toEqual({ w: 300, h: 200 });
  });
});
