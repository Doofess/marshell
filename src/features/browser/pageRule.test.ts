import { describe, expect, it } from "vitest";
import { paneColumn, type ColumnInputs } from "../window/paneColumn";
import { pageRule } from "./pageRule";

const base: ColumnInputs = { pref: "expanded", drawer: false, shown: true, saved: null, terminalMin: 640, rtl: false };
const inset = { start: 1, end: 12 };

function at(pieces: ReturnType<typeof pageRule>, w: number) {
  const p = [...pieces].reverse().find((q) => q.from <= w)!;
  return { x: p.x + p.xPerPx * (w - p.from), width: p.width + p.widthPerPx * (w - p.from) };
}

describe("pageRule", () => {
  it("has a piece per stretch", () => {
    const pieces = pageRule(base, inset);
    const froms = pieces.map((p) => p.from);
    // The room crosses 720 at 772 (rail: 52 + 720) and again at 1008 (expanded: 288 + 720); the window crosses 960 between them.
    for (const w of [772, 960, 1008]) expect(froms).toContain(w);
    // The rest are the pane's own limits (its minimum and the 80-column stop) and the tiny windows no one sees.
    expect(pieces.length).toBeLessThan(10);
  });

  it("lands within 1e-6 of the column less the insets at every whole width from 400 to 2000", () => {
    for (const i of [base, { ...base, rtl: true }, { ...base, drawer: true }, { ...base, saved: 500 }, { ...base, saved: 3000 }]) {
      const pieces = pageRule(i, inset);
      for (let w = 400; w <= 2000; w++) {
        const col = paneColumn(w, i);
        const got = at(pieces, w);
        expect(Math.abs(got.x - (col.x + inset.start))).toBeLessThan(1e-6);
        expect(Math.abs(got.width - Math.max(0, col.width - inset.start - inset.end))).toBeLessThan(1e-6);
      }
    }
  });

  it("makes a piece of a saved width held by the 80-column stop", () => {
    const free = pageRule({ ...base, saved: 300 }, inset).length;
    const held = pageRule({ ...base, saved: 3000 }, inset);
    expect(held.length).toBeGreaterThan(0);
    // Held at room - 1 - 640, the width grows with the window instead of staying put.
    expect(held.some((p) => p.widthPerPx !== 0)).toBe(true);
    expect(free).toBeGreaterThan(0);
  });
});
