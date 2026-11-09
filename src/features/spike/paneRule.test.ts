import { describe, expect, it } from "vitest";
import { paneColumn, paneRule, type Piece, type RuleState } from "./paneRule";

/** The live check on 2026-11-09: a 251 px sidebar, the divider at half, no drawer. */
const live: RuleState = { pref: "expanded", sideReq: 251, ratio: 0.5, drawer: false };

const at = (pieces: Piece[], w: number) => {
  const p = [...pieces].reverse().find((q) => q.from <= w) ?? pieces[0];
  if (!p) throw new Error("no pieces");
  return { x: p.x + p.xPerPx * (w - p.from), width: p.width + p.widthPerPx * (w - p.from) };
};

describe("paneColumn", () => {
  it("lands where the front measured the page on each side of every jump", () => {
    // (window width, measured x, measured width) from the hand drag's reports.
    const measured: [number, number, number][] = [
      [704, 52, 652],
      [770, 52, 718],
      [772, 412.5, 359.5],
      [849, 451, 398],
      [959, 506, 453],
      [960, 251, 709],
      [970, 251, 719],
      [971, 611.5, 359.5],
      [1091, 671.5, 419.5],
    ];
    for (const [w, x, width] of measured) expect(paneColumn(w, live), `${w}`).toEqual({ x, width });
  });

  it("follows the divider and a rail the person chose", () => {
    expect(paneColumn(1091, { ...live, ratio: 0.3 }).width).toBeCloseTo(839 * 0.7);
    expect(paneColumn(1200, { ...live, pref: "rail" }).x).toBe(52 + (1200 - 52 - 1) / 2 + 1);
  });
});

describe("paneRule", () => {
  it("is a handful of pieces, starting at each jump", () => {
    const pieces = paneRule(live);
    expect(pieces.map((p) => p.from)).toEqual([0, 772, 960, 971]);
  });

  it("gives what paneColumn gives at every whole-pixel width", () => {
    for (const s of [live, { ...live, ratio: 0.42 }, { ...live, drawer: true }, { ...live, pref: "rail" as const }]) {
      const pieces = paneRule(s, 3000);
      for (let w = 0; w <= 3000; w++) {
        const want = paneColumn(w, s);
        const got = at(pieces, w);
        expect(got.x, `${w}`).toBeCloseTo(want.x, 6);
        expect(got.width, `${w}`).toBeCloseTo(want.width, 6);
      }
    }
  });
});
