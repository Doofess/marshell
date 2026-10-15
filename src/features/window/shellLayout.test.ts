import { describe, expect, it } from "vitest";
import { SHELL, shellLayout } from "./shellLayout";

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
