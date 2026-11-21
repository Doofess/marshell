import { describe, expect, it } from "vitest";
import { DEFAULT_WEIGHT, type Weights } from "./panels";
import { belowPerPx, movedPair, pairWeights, stackHeights, stackKey } from "./stackLayout";

const B = ["browser"] as const;
const BF = ["browser", "files"] as const;
const sum = (h: Weights) => (h.browser ?? 0) + (h.files ?? 0);

describe("stackHeights", () => {
  it("one panel takes the whole height", () => {
    expect(stackHeights(600, B, {})).toEqual({ browser: 600 });
  });
  it("two equal weights split the height less the divider", () => {
    expect(stackHeights(601, BF, {})).toEqual({ browser: 300, files: 300 });
  });
  it("weights 3 to 1 split 3 to 1", () => {
    expect(stackHeights(801, BF, { browser: 30000, files: 10000 })).toEqual({ browser: 600, files: 200 });
  });
  it("a panel under the minimum takes 160 and the other the rest", () => {
    expect(stackHeights(601, BF, { browser: 100, files: 9900 })).toEqual({ browser: 160, files: 440 });
  });
  it("a pane too short for the minimum splits evenly", () => {
    expect(stackHeights(301, BF, {})).toEqual({ browser: 150, files: 150 });
    expect(stackHeights(300, BF, {})).toEqual({ browser: 150, files: 149 });
  });
  it("heights are whole and sum to the total less the dividers", () => {
    const pairs: Weights[] = [{}, { browser: 12345, files: 6789 }, { browser: 1, files: 100000 }];
    for (const w of pairs)
      for (let total = 200; total <= 900; total++) {
        const h = stackHeights(total, BF, w);
        expect(Number.isInteger(h.browser)).toBe(true);
        expect(Number.isInteger(h.files)).toBe(true);
        expect(sum(h)).toBe(total - 1);
      }
  });
  it("gives back the heights a divider move asked for, at every position", () => {
    // The live check's case: a drag to 470 over 269 came back as 471 over 268.
    expect(stackHeights(740, BF, pairWeights({ browser: 10000, files: 10000 }, "browser", "files", 470, 269))).toEqual({ browser: 470, files: 269 });
    for (const total of [740, 630, 1081]) {
      const room = total - 1;
      for (let upper = 160; upper <= room - 160; upper++) {
        const weights = pairWeights({ browser: 12720, files: 7280 }, "browser", "files", upper, room - upper);
        expect(stackHeights(total, BF, weights), `${upper} of ${room}`).toEqual({ browser: upper, files: room - upper });
      }
    }
  });
  it("a spare pixel goes to the panel that is furthest from its share, and to the first on a tie", () => {
    expect(stackHeights(740, BF, { browser: 10000, files: 10000 })).toEqual({ browser: 370, files: 369 });
    // 739 × 0.3 = 221.7 and 739 × 0.7 = 517.3: the spare pixel is Browser's.
    expect(stackHeights(740, BF, { browser: 3, files: 7 })).toEqual({ browser: 222, files: 517 });
    // 739 × 0.7 = 517.3 and 739 × 0.3 = 221.7: the spare pixel is Files'.
    expect(stackHeights(740, BF, { browser: 7, files: 3 })).toEqual({ browser: 517, files: 222 });
  });
  it("a panel not shown has no height, and a bad weight counts as the default", () => {
    expect(stackHeights(600, B, {})).not.toHaveProperty("files");
    expect(stackHeights(601, BF, { browser: Number.NaN, files: -5 })).toEqual(stackHeights(601, BF, { browser: DEFAULT_WEIGHT, files: DEFAULT_WEIGHT }));
  });
  it("zero, negative and NaN totals give zero heights", () => {
    for (const t of [0, -10, Number.NaN, Number.POSITIVE_INFINITY]) expect(stackHeights(t, BF, {})).toEqual({ browser: 0, files: 0 });
  });
});

describe("belowPerPx", () => {
  it("the last panel has nothing below it", () => {
    expect(belowPerPx(601, BF, {}, "files")).toBe(0);
  });
  it("equal weights put half of each pixel below Browser", () => {
    expect(belowPerPx(601, BF, {}, "browser")).toBe(0.5);
  });
  it("3 to 1 puts a quarter below", () => {
    expect(belowPerPx(801, BF, { browser: 30000, files: 10000 }, "browser")).toBe(0.25);
  });
  it("Browser held at its minimum puts every pixel below", () => {
    expect(belowPerPx(601, BF, { browser: 100, files: 9900 }, "browser")).toBe(1);
  });
  it("Files held at its minimum puts none", () => {
    expect(belowPerPx(601, BF, { browser: 9900, files: 100 }, "browser")).toBe(0);
  });
  it("a pane too short puts half", () => {
    expect(belowPerPx(301, BF, {}, "browser")).toBe(0.5);
  });
  it("one panel has none", () => {
    expect(belowPerPx(600, B, {}, "browser")).toBe(0);
    expect(belowPerPx(600, B, {}, "files")).toBe(0);
  });
});

describe("movedPair", () => {
  it("keeps the sum and the minimum", () => {
    expect(movedPair(300, 300, 200)).toEqual({ upper: 440, lower: 160 });
    expect(movedPair(300, 300, -1000)).toEqual({ upper: 160, lower: 440 });
    expect(movedPair(300, 300, 16)).toEqual({ upper: 316, lower: 284 });
  });
  it("a drag in a pane too short changes nothing", () => {
    expect(movedPair(150, 150, 40)).toEqual({ upper: 150, lower: 150 });
  });
});

describe("pairWeights", () => {
  it("keeps the pair's sum", () => {
    expect(pairWeights({}, "browser", "files", 450, 150)).toEqual({ browser: 15000, files: 5000 });
    expect(pairWeights({}, "browser", "files", 1, 1)).toEqual({ browser: 10000, files: 10000 });
    expect(pairWeights({ browser: 3000, files: 1000 }, "browser", "files", 1, 3)).toEqual({ browser: 1000, files: 3000 });
  });
  it("gives each at least 1 and at most 100000", () => {
    expect(pairWeights({}, "browser", "files", 1, 1_000_000)).toEqual({ browser: 1, files: 19999 });
    const w = pairWeights({ browser: 100000, files: 100000 }, "browser", "files", 99, 1);
    expect(w.browser).toBeLessThanOrEqual(100000);
    expect(w.files).toBeGreaterThanOrEqual(1);
  });
  it("pairWeights with a zero or NaN height returns the weights it was given", () => {
    const w: Weights = { browser: 7 };
    expect(pairWeights(w, "browser", "files", 0, 100)).toBe(w);
    expect(pairWeights(w, "browser", "files", 100, Number.NaN)).toBe(w);
  });
});

describe("stackKey", () => {
  it("joins the panels with +", () => {
    expect(stackKey(["browser", "files"])).toBe("browser+files");
    expect(stackKey([])).toBe("");
  });
});
