import { describe, expect, it } from "vitest";
import { GLYPHS, paintStrip, tileLayout, type StripLooks, type Surface } from "./tiles";

const looks: StripLooks = {
  bar: "bar",
  hairline: "line",
  rest: { bg: "rest-bg", fg: "rest-fg" },
  hover: { bg: "hover-bg", fg: "hover-fg" },
  closeHover: { bg: "close-bg", fg: "close-fg" },
};

type Call = unknown[];
/** A hand-written surface: it records the drawing calls and refuses the colour "bad" by leaving the style unchanged. */
function fake(log: Call[][], width: number, height: number): Surface {
  let fill = "";
  let stroke = "";
  const ctx = {
    get fillStyle() {
      return fill;
    },
    set fillStyle(v: string) {
      if (v !== "bad") fill = v;
    },
    get strokeStyle() {
      return stroke;
    },
    set strokeStyle(v: string) {
      if (v !== "bad") stroke = v;
    },
    lineWidth: 0,
    fillRect: (...a: number[]) => log[log.length - 1]?.push(["fillRect", ...a, fill]),
    setTransform: (...a: number[]) => log[log.length - 1]?.push(["setTransform", ...a]),
    translate: (...a: number[]) => log[log.length - 1]?.push(["translate", ...a]),
    stroke: (p: { d: string }) => log[log.length - 1]?.push(["stroke", p.d, stroke]),
    getImageData: () => ({ data: new Uint8ClampedArray(width * height * 4) }),
  };
  return { ctx: ctx as unknown as CanvasRenderingContext2D, path: (d) => ({ d }) as unknown as Path2D };
}
const maker = (log: Call[][] = []) => (w: number, h: number) => {
  log.push([]);
  return fake(log, w, h);
};

describe("tiles", () => {
  it("lays the buttons out in whole device pixels", () => {
    expect(tileLayout(1)).toMatchObject({ height: 40, hairline: 1, edges: [0, 46, 92, 138] });
    expect(tileLayout(1.25)).toMatchObject({ height: 50, hairline: 1, edges: [0, 58, 115, 173] });
    expect(tileLayout(1.5)).toMatchObject({ height: 60, hairline: 1, edges: [0, 69, 138, 207] });
    expect(tileLayout(2)).toMatchObject({ height: 80, hairline: 2, edges: [0, 92, 184, 276] });
  });

  it("paints the bar, the hairline, the button and its glyph, in that order", () => {
    const log: Call[][] = [];
    const paint = paintStrip(tileLayout(1), looks, maker(log));
    expect(paint).not.toBeNull();
    // Pictures are made in the order minimize, maximize, restore, close; each as rest then hover. Close's hover is the last.
    const last = log[log.length - 1];
    expect(last).toEqual([
      ["fillRect", 0, 0, 46, 40, "bar"],
      ["fillRect", 0, 39, 46, 1, "line"],
      ["fillRect", 0, 0, 46, 39, "close-bg"],
      ["setTransform", 1, 0, 0, 1, -92, 0],
      ["translate", 110, 14.5],
      ["stroke", GLYPHS.close, "close-fg"],
    ]);
  });

  it("gives every picture its own width at a fractional scale", () => {
    const paint = paintStrip(tileLayout(1.25), looks, maker());
    expect(paint).not.toBeNull();
    const p = paint!;
    expect(p.height).toBe(50);
    expect([p.minimize.width, p.maximize.width, p.restore.width, p.close.width]).toEqual([58, 57, 57, 58]);
    for (const b of [p.minimize, p.maximize, p.restore, p.close]) {
      expect(b.rest).toHaveLength(b.width * 50 * 4);
      expect(b.hover).toHaveLength(b.width * 50 * 4);
    }
  });

  it("uses the hover look for Minimize and Maximize and the close look for Close", () => {
    const log: Call[][] = [];
    paintStrip(tileLayout(1), looks, maker(log));
    const bg = (i: number) => (log[i]?.[2] as unknown[])[5];
    // minimize rest, hover; maximize rest, hover; restore rest, hover; close rest, hover.
    expect(log).toHaveLength(8);
    expect([0, 1, 2, 3, 4, 5, 6, 7].map(bg)).toEqual(["rest-bg", "hover-bg", "rest-bg", "hover-bg", "rest-bg", "hover-bg", "rest-bg", "close-bg"]);
  });

  it("returns null when a colour is refused or no canvas can be made", () => {
    expect(paintStrip(tileLayout(1), { ...looks, bar: "bad" }, maker())).toBeNull();
    expect(paintStrip(tileLayout(1), { ...looks, hover: { bg: "x", fg: "bad" } }, maker())).toBeNull();
    expect(paintStrip(tileLayout(1), looks, () => null)).toBeNull();
  });
});
