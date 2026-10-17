import { describe, expect, it } from "vitest";
import { canMarkRegion, describeRegion, fromDrag, initialRegion, nudge, regionLabel, regionStillValid, regionText } from "./region";

describe("region", () => {
  it("starts 40% x 25%, centred in the visible part of the page", () => {
    expect(initialRegion(2, { top: 0, bottom: 1 })).toEqual({ page: 2, x: 0.3, y: 0.375, w: 0.4, h: 0.25 });
  });

  it("turns a drag into a box, in either direction, and drops a sliver", () => {
    expect(fromDrag(1, { x: 10, y: 10 }, { x: 15, y: 80 }, { w: 100, h: 100 })).toBeNull();
    expect(fromDrag(1, { x: 80, y: 90 }, { x: 20, y: 10 }, { w: 100, h: 100 })).toEqual({ page: 1, x: 0.2, y: 0.1, w: 0.6, h: 0.8 });
  });

  it("moves, fine-moves and resizes with the arrow keys, inside the page", () => {
    const r = { page: 1, x: 0.9, y: 0, w: 0.1, h: 0.1 };
    expect(nudge(r, "ArrowRight", { shift: false, ctrl: false }, { w: 100, h: 100 }).x).toBe(0.9); // clamped
    expect(nudge(r, "ArrowLeft", { shift: false, ctrl: true }, { w: 100, h: 100 }).x).toBeCloseTo(0.89);
    expect(nudge(r, "ArrowLeft", { shift: true, ctrl: false }, { w: 100, h: 100 }).w).toBeCloseTo(0.08); // min 8 px
  });

  it("knows when a page is gone", () => {
    const r = { page: 1, x: 0.9, y: 0, w: 0.1, h: 0.1 };
    expect(regionStillValid({ ...r, page: 4 }, 3)).toBe(false);
    expect(regionStillValid({ ...r, page: 3 }, 3)).toBe(true);
  });

  it("labels and words the paste", () => {
    expect(regionLabel("report.pdf", 3)).toBe("report.pdf, page 3");
    expect(regionLabel("chart.png", null)).toBe("chart.png");
    expect(regionText("chart.png", "  look\r\n\there ")).toBe("chart.png: look here");
    expect(regionText("chart.png", " \n ")).toBe("chart.png");
    expect(regionText("a.pdf, page 1", "x".repeat(5000))).toHaveLength(2000);
  });

  it("turns every control character into a space, not only whitespace", () => {
    expect(regionText("a.png", "a\u001b[31mb")).toBe("a.png: a [31mb");
    expect(regionText("a.png", "a\u0007b")).toBe("a.png: a b");
    expect(regionText("a.png", "a\u009bb\u007fc\u0000d")).toBe("a.png: a b c d");
    expect(regionText("a.png", "\u001b\u0007")).toBe("a.png");
  });

  it("cuts at 2000 code points and never inside a surrogate pair", () => {
    const text = regionText("a.png", "😀".repeat(3000));
    expect(Array.from(text)).toHaveLength(2000);
    expect(text).not.toMatch(/[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/);
    // "a.png: " is 7 units, so UTF-16 unit 2000 falls in the middle of the second emoji.
    const edge = regionText("a.png", "x".repeat(1990) + "😀".repeat(4));
    expect(Array.from(edge)).toHaveLength(2000);
    expect(edge.endsWith("x" + "😀".repeat(3))).toBe(true);
  });

  it("describes the box for a screen reader", () => {
    expect(describeRegion({ page: 3, x: 0.3, y: 0.2, w: 0.4, h: 0.25 })).toBe("Box 40% wide, 25% high, 30% across, 20% down, page 3");
  });
});

describe("canMarkRegion", () => {
  it("marks an image, and a PDF only once its document is loaded", () => {
    expect(canMarkRegion("image", false)).toBe(true);
    expect(canMarkRegion("pdf", true)).toBe(true);
    expect(canMarkRegion("pdf", false)).toBe(false);
  });
  it("marks nothing else", () => {
    for (const phase of ["text", "loading", "error", "deleted", "missing"]) expect(canMarkRegion(phase, true)).toBe(false);
  });
});
