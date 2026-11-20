import { describe, expect, it } from "vitest";
import { SHELL, shellLayout, wholeRows } from "./shellLayout";

describe("shellLayout: how tall the shell panel is", () => {
  it("is a 32 px header over a body of 30% of the sidebar, never under 120 or over 280", () => {
    expect(SHELL).toMatchObject({ header: 32, share: 0.3, minBody: 120, maxBody: 280 });
    expect(shellLayout(840, 28)).toEqual({ height: 32 + 252, collapsed: false, auto: false });
    expect(shellLayout(1200, 28).height).toBe(32 + 280);
    expect(shellLayout(660, 28).height).toBe(32 + 198);
  });
  it("never lets the list fall under two rows: the panel gives way to its header alone", () => {
    // 600 tall, lane 28, footer 40: an open panel is 32 + 180 = 212, which leaves 320, but a lane of 240 leaves 108 < 2 x 56.
    expect(shellLayout(600, 28)).toMatchObject({ collapsed: false, auto: false });
    expect(shellLayout(600, 240)).toEqual({ height: 32, collapsed: true, auto: true });
  });
  it("keeps the 40% lane and an open panel apart on a normal 880 px window", () => {
    // 840 sidebar: the lane takes at most 40% (336) with its card; 840 - 336 - 40 - 284 = 180 >= 112.
    expect(shellLayout(840, 336)).toMatchObject({ collapsed: false });
  });
  it("honours the user's choice to hide it, and says that was the user's, not automatic", () => {
    expect(shellLayout(1200, 28, true)).toEqual({ height: 32, collapsed: true, auto: false });
  });
  it("reports auto only when it collapsed by itself", () => {
    expect(shellLayout(500, 200).auto).toBe(true);
    expect(shellLayout(500, 200, true).auto).toBe(false);
    expect(shellLayout(1200, 28).auto).toBe(false);
  });
  it("opens an auto-folded panel when the user asked for it, leaving the list at least one row", () => {
    expect(shellLayout(500, 200)).toMatchObject({ collapsed: true, auto: true });
    const open = shellLayout(500, 200, false, 56, true);
    expect(open).toMatchObject({ collapsed: false, auto: false });
    expect(500 - 200 - 40 - open.height).toBeGreaterThanOrEqual(56);
    // With room, asking changes nothing.
    expect(shellLayout(1200, 28, false, 56, true)).toEqual(shellLayout(1200, 28));
    // Hiding is still the user's, and a window with no room at all still folds.
    expect(shellLayout(1200, 28, true, 56, true)).toEqual({ height: 32, collapsed: true, auto: false });
    expect(shellLayout(300, 200, false, 56, true)).toEqual({ height: 32, collapsed: true, auto: true });
    expect(shellLayout(Number.NaN, 28, false, 56, true)).toMatchObject({ collapsed: true });
  });
  it("collapses, never throws, for nonsense heights", () => {
    for (const h of [0, -5, Number.NaN, Number.POSITIVE_INFINITY, 40, 100]) {
      const l = shellLayout(h, 28);
      expect(l.height, String(h)).toBe(32);
      expect(l.collapsed, String(h)).toBe(true);
    }
    expect(shellLayout(800, Number.NaN)).toMatchObject({ collapsed: false });
    expect(shellLayout(800, -10)).toMatchObject({ collapsed: false });
  });
  it("takes the real row height from the caller", () => {
    expect(shellLayout(600, 200, false, 40)).toMatchObject({ collapsed: false });
    expect(shellLayout(600, 200, false, 80)).toMatchObject({ collapsed: true });
  });
  it("is monotonic: a taller sidebar never has a smaller open panel", () => {
    let prev = 0;
    for (let h = 600; h <= 1600; h += 40) {
      const l = shellLayout(h, 28);
      expect(l.height).toBeGreaterThanOrEqual(prev);
      prev = l.height;
    }
  });
});

const CELL = { height: 17, pad: 8 };

describe("wholeRows", () => {
  it("cuts a body down to the rows that fit", () => {
    expect(wholeRows(180, CELL)).toBe(178);
    expect(wholeRows(280, CELL)).toBe(280);
    expect(wholeRows(120, CELL)).toBe(110);
  });
  it("rounds a fractional row up to a whole pixel, and never loses a row that fits exactly", () => {
    expect(wholeRows(150, { height: 17.6, pad: 8 })).toBe(149);
    expect(wholeRows(184, { height: 17.6, pad: 8 })).toBe(184);
  });
  it("leaves the body alone without a measured row, or when not one row fits", () => {
    expect(wholeRows(180.4, null)).toBe(180);
    expect(wholeRows(180, { height: 0, pad: 8 })).toBe(180);
    expect(wholeRows(20, CELL)).toBe(20);
  });
  it("never asks for more than the body it was given", () => {
    for (let body = 60; body <= 280; body++) {
      for (const cell of [CELL, { height: 17.6, pad: 16 }, { height: 22.5, pad: 0 }]) expect(wholeRows(body, cell), `${body} ${cell.height}`).toBeLessThanOrEqual(body);
    }
  });
});

describe("shellLayout with a measured row", () => {
  it("opens to whole rows", () => {
    expect(shellLayout(600, 0, false, 56, false, CELL)).toEqual({ height: 210, collapsed: false, auto: false });
  });
  it("is as before without one", () => {
    expect(shellLayout(600, 0)).toEqual({ height: 212, collapsed: false, auto: false });
  });
  it("opens a forced panel to whole rows too", () => {
    expect(shellLayout(230, 0, false, 56, true, CELL)).toEqual({ height: 125, collapsed: false, auto: false });
  });
});
