import { describe, expect, it } from "vitest";
import { MAX_PNG_BYTES, cropRect, cropScale, outputSize, smallerSize } from "./crop";

describe("smallerSize", () => {
  it("is the 20 MB cap the core enforces", () => {
    expect(MAX_PNG_BYTES).toBe(20 * 1024 * 1024);
  });
  it("shrinks both sides by sqrt(cap / size) x 0.9, the ratio kept", () => {
    const next = smallerSize({ w: 4096, h: 2730 }, 4 * MAX_PNG_BYTES);
    const k = 0.5 * 0.9;
    expect(next).toEqual({ w: Math.round(4096 * k), h: Math.round(2730 * k) });
  });
  it("never goes under one pixel", () => {
    expect(smallerSize({ w: 1, h: 1 }, 1000 * MAX_PNG_BYTES)).toEqual({ w: 1, h: 1 });
    expect(smallerSize({ w: 3, h: 2000 }, 1000 * MAX_PNG_BYTES).w).toBe(1);
  });
});

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
