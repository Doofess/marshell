import { describe, expect, it } from "vitest";
import { paneColumn, type ColumnInputs } from "../window/paneColumn";
import type { PageState } from "./pageStore";
import { insetRect, openUnder, PAGE_GUTTER, slotReport, slotStep } from "./usePageSlot";

const page: PageState = { url: "https://example.com/", title: "", canBack: false, canForward: false, loading: false, icon: null, focused: false, status: null, live: true };
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
  // Found in review: the shell refused the tab's address before it made a page, and the tab never asked again.
  it("a page that was asked for and never loaded is still to open", () => {
    const refused: PageState = { ...page, url: "", status: "refused", live: false };
    expect(slotStep(refused, true, false)).toBe("open");
    expect(slotStep(refused, false, false)).toBe("none");
  });
  it("a tab that was never active opens nothing", () => {
    expect(slotStep(undefined, false, false)).toBe("none");
    expect(slotStep(undefined, false, true)).toBe("none");
  });
});

// Found in review: a link opens a tab and puts focus on its slot before the shell has made the page, so the page was asked to
// take the keyboard while it didn't exist, and the keys stayed with the app.
describe("openUnder", () => {
  const api = (calls: string[], refuse = false) => ({
    open: async (key: string, url: string) => {
      calls.push(`open ${key} ${url}`);
      if (refuse) throw new Error("refused");
    },
    focus: async (key: string, reason: string) => void calls.push(`focus ${key} ${reason}`),
  });
  it("a page that opens under the focused slot takes the keyboard", async () => {
    const calls: string[] = [];
    await openUnder(api(calls), "s/t", "https://a.test/", () => true);
    expect(calls).toEqual(["open s/t https://a.test/", "focus s/t programmatic"]);
  });
  it("a page that opens while focus is elsewhere leaves the keyboard there", async () => {
    const calls: string[] = [];
    await openUnder(api(calls), "s/t", "https://a.test/", () => false);
    expect(calls).toEqual(["open s/t https://a.test/"]);
  });
  it("a page the shell refused to open is not asked to take the keyboard", async () => {
    const calls: string[] = [];
    await openUnder(api(calls, true), "s/t", "http://tauri.localhost/", () => true);
    expect(calls).toEqual(["open s/t http://tauri.localhost/"]);
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

describe("the page's gutter", () => {
  const win = rect(0, 0, 1400, 900);
  it("the page sits 2 px inside its slot", () => {
    expect(insetRect({ x: 10, y: 20, width: 100, height: 50, right: 110, bottom: 70 }, 2)).toEqual({ x: 12, y: 22, width: 96, height: 46, right: 108, bottom: 68 });
  });
  it("a slot no wider than the gutter gives no box", () => {
    const thin = insetRect(rect(10, 20, 3, 50), PAGE_GUTTER);
    expect(thin.width).toBe(0);
    const r = slotReport(thin, win, inputs, false);
    expect(r.rect).toEqual({ x: 0, y: 0, width: 0, height: 0 });
    expect(r.rule).toBeNull();
  });
  it("the rule keeps the gutter", () => {
    const col = paneColumn(1400, inputs);
    const slot = rect(col.x, 70, col.width, 800);
    const plain = slotReport(slot, win, inputs, false);
    const inset = slotReport(insetRect(slot, PAGE_GUTTER), win, inputs, false);
    expect(inset.rule!.top).toBe(plain.rule!.top + 2);
    expect(inset.rule!.bottom).toBe(plain.rule!.bottom + 2);
    expect(inset.rule!.pieces[0]!.x).toBe(plain.rule!.pieces[0]!.x + 2);
  });
});
