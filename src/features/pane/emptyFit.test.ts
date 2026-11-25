import { describe, expect, it } from "vitest";
import { fitEmpty, firstFit, type FitLevel } from "./emptyFit";

describe("firstFit", () => {
  const heights = (h: [number, number, number]) => {
    const asked: number[] = [];
    return { at: (l: FitLevel) => (asked.push(l), h[l]), asked };
  };
  it("keeps everything when it fits, and measures nothing more", () => {
    const m = heights([400, 300, 90]);
    expect(firstFit(400, m.at)).toBe(0);
    expect(m.asked).toEqual([0]);
  });
  it("gives up the mark and the help line first, then the whole list", () => {
    expect(firstFit(399, heights([400, 300, 90]).at)).toBe(1);
    expect(firstFit(299, heights([400, 300, 90]).at)).toBe(2);
  });
  it("stays at the last level when even that is too tall", () => {
    expect(firstFit(50, heights([400, 300, 90]).at)).toBe(2);
  });
});

describe("fitEmpty", () => {
  it("leaves the level it found on the panel", () => {
    const panel = { clientHeight: 250, dataset: {} as Record<string, string> };
    const column = {
      get offsetHeight() {
        return [400, 300, 90][Number(panel.dataset.level)];
      },
    };
    fitEmpty(panel as unknown as HTMLElement, column as unknown as HTMLElement);
    expect(panel.dataset.level).toBe("2");
  });
});
