import { describe, expect, it } from "vitest";
import { canvasScale, MAX_CANVAS_PIXELS, MAX_CANVAS_SIDE, pdfErrorMessage } from "./pdfjs";

describe("pdfErrorMessage", () => {
  it("says a locked PDF is password-protected and any other failure can't be shown", () => {
    expect(pdfErrorMessage({ name: "PasswordException" })).toBe("This PDF is password-protected.");
    expect(pdfErrorMessage({ name: "InvalidPDFException" })).toBe("This PDF can't be shown.");
    expect(pdfErrorMessage(new Error("x"))).toBe("This PDF can't be shown.");
  });
});

describe("canvasScale", () => {
  it("is scale x dpr while the canvas stays small", () => {
    expect(canvasScale(612, 792, 1.5, 2)).toBe(3);
  });

  it("is clamped so the canvas has at most 16 777 216 pixels", () => {
    const f = canvasScale(612, 792, 3, 3);
    expect(f).toBeLessThan(9);
    expect(Math.floor(612 * f) * Math.floor(792 * f)).toBeLessThanOrEqual(MAX_CANVAS_PIXELS);
    expect(MAX_CANVAS_PIXELS).toBe(16_777_216);
  });

  it("is clamped so no side passes 32767 pixels", () => {
    const f = canvasScale(10, 14400, 3, 3);
    expect(Math.floor(14400 * f)).toBeLessThanOrEqual(MAX_CANVAS_SIDE);
    expect(Math.floor(10 * f) * Math.floor(14400 * f)).toBeLessThanOrEqual(MAX_CANVAS_PIXELS);
  });

  it("falls back to scale x dpr for a degenerate page", () => {
    expect(canvasScale(0, 0, 2, 2)).toBe(4);
  });
});
