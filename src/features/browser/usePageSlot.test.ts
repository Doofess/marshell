import { describe, expect, it } from "vitest";
import { paneColumn, type ColumnInputs } from "../window/paneColumn";
import type { PageState } from "./pageStore";
import { slotReport, slotStep } from "./usePageSlot";

const page: PageState = { url: "https://example.com/", title: "", canBack: false, canForward: false, loading: false, icon: null, focused: false, status: null };
const rect = (x: number, y: number, width: number, height: number) => ({ x, y, width, height, right: x + width, bottom: y + height });
const inputs: ColumnInputs = { pref: "expanded", drawer: false, shown: true, saved: 480, terminalMin: 640, rtl: false };

describe("slotStep", () => {
  it("a_gone_page_loads_again_when_shown", () => {
    expect(slotStep(undefined, true, false)).toBe("open");
    expect(slotStep(undefined, true, true)).toBe("open");
  });
  it("a page that exists and is active and uncovered is shown", () => {
    expect(slotStep(page, true, false)).toBe("show");
  });
  it("a covered or inactive page is hidden", () => {
    expect(slotStep(page, true, true)).toBe("hide");
    expect(slotStep(page, false, false)).toBe("hide");
    expect(slotStep(page, false, true)).toBe("hide");
  });
  it("a tab that was never active opens nothing", () => {
    expect(slotStep(undefined, false, false)).toBe("none");
    expect(slotStep(undefined, false, true)).toBe("none");
  });
});

describe("slotReport", () => {
  const win = rect(0, 0, 1400, 900);
  it("the report's rule ends where the slot ends", () => {
    const col = paneColumn(1400, inputs);
    const slot = rect(col.x + 1, 70, col.width - 1, 800);
    const r = slotReport(slot, win, inputs, false);
    expect(r.rect).toEqual({ x: slot.x, y: 70, width: slot.width, height: 800 });
    expect(r.win).toEqual({ width: 1400, height: 900 });
    expect(r.moving).toBe(false);
    expect(r.rule!.top).toBe(70);
    expect(r.rule!.bottom).toBe(900 - 870);
    // The rule at the window's own width gives the slot back: the column less one pixel at its start, none at its end.
    const piece = [...r.rule!.pieces].reverse().find((p) => p.from <= 1400)!;
    expect(piece.x + piece.xPerPx * (1400 - piece.from)).toBeCloseTo(slot.x, 6);
    expect(piece.width + piece.widthPerPx * (1400 - piece.from)).toBeCloseTo(slot.width, 6);
  });
  it("carries moving", () => {
    expect(slotReport(rect(900, 70, 400, 700), win, inputs, true).moving).toBe(true);
  });
  it("a slot with no size reports a zero box, not NaN", () => {
    const r = slotReport(rect(Number.NaN, Number.NaN, 0, 0), win, inputs, false);
    expect(r.rect).toEqual({ x: 0, y: 0, width: 0, height: 0 });
    expect(r.rule).toBeNull();
  });
});
