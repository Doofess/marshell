import { describe, expect, it } from "vitest";
import { chipLabel, chipText, frameBox, pageBox, viewportChoice, viewportFit, viewportOf, viewportRows } from "./viewport";

describe("viewport", () => {
  it("fits a narrower size at full scale and a wider one scaled", () => {
    expect(viewportFit(800, 390)).toEqual({ width: 390, scale: 1 });
    expect(viewportFit(390, 390)).toEqual({ width: 390, scale: 1 });
    expect(viewportFit(360, 1440)).toEqual({ width: 1440, scale: 0.25 });
    expect(viewportFit(0, 390)).toBeNull();
    expect(viewportFit(800, null)).toBeNull();
  });
  it("puts a page that fits where the shell puts it: centred, rounded down", () => {
    expect(pageBox(801, 390)).toEqual({ inlineSize: 390, marginInlineStart: 205 }); // Task 8's fit test: x 100 + 205 = 305
    expect(pageBox(390, 390)).toBeUndefined();
    expect(pageBox(360, 1440)).toBeUndefined();
    expect(pageBox(801, null)).toBeUndefined();
  });
  it("words the chip", () => {
    expect(chipText("phone", 800)).toBe("Phone 390");
    expect(chipText("desktop", 360)).toBe("Desktop 1440 · 25%");
    expect(chipLabel("desktop", 360)).toBe("Viewport: Desktop, 1440 pixels wide, shown at 25%");
    expect(chipLabel("phone", 800)).toBe("Viewport: Phone, 390 pixels wide");
  });
  it("lists five radio rows with the tab's one checked", () => {
    expect(viewportRows("tablet").map((r) => "label" in r && [r.id, r.label, r.checked])).toEqual([
      ["viewport:responsive", "Responsive", false],
      ["viewport:phone", "Phone (390 px)", false],
      ["viewport:tablet", "Tablet (768 px)", true],
      ["viewport:laptop", "Laptop (1024 px)", false],
      ["viewport:desktop", "Desktop (1440 px)", false],
    ]);
  });
  it("reads a tab's size and nothing else", () => {
    expect(viewportOf({ viewport: "laptop" })).toBe("laptop");
    expect(viewportOf({ viewport: "wide" })).toBeNull();
    expect(viewportOf(undefined)).toBeNull();
    expect(viewportChoice("viewport:responsive")).toBeNull();
    expect(viewportChoice("copy")).toBeUndefined();
  });
  it("centres an HTML file's frame that fits and scales one that doesn't", () => {
    expect(frameBox({ width: 800, height: 600 }, 390)).toEqual({ inlineSize: 390, marginInline: "auto" });
    expect(frameBox({ width: 360, height: 600 }, 1440)).toEqual({ inlineSize: 1440, blockSize: 2400, transform: "scale(0.25)", transformOrigin: "0 0" });
    expect(frameBox({ width: 800, height: 600 }, null)).toBeUndefined();
  });
});
