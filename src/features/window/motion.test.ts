import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { LAYOUT_MOTION, easeAt, motionValue, nextMotion, type Motion } from "./motion";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const tokens = readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8");

describe("layout motion", () => {
  it("the curve starts at 0, ends at 1 and matches the CSS curve at the middle", () => {
    expect(easeAt(0)).toBe(0);
    expect(easeAt(1)).toBe(1);
    // x = 0.5 at the curve's parameter of about 0.7245, where y is 0.946.
    expect(Math.abs(easeAt(0.5) - 0.946)).toBeLessThan(0.005);
    let last = 0;
    for (let i = 0; i <= 10; i++) {
      const y = easeAt(i / 10);
      expect(y).toBeGreaterThanOrEqual(last);
      last = y;
    }
    // Out of range is held to the ends.
    expect(easeAt(-1)).toBe(0);
    expect(easeAt(2)).toBe(1);
  });

  it("an idle motion draws the target", () => {
    expect(motionValue(null, 288, 1234)).toEqual({ value: 288, done: true });
  });

  it("a motion draws between from and the live target, and is done after 200 ms", () => {
    const m: Motion = { from: 0, startedAt: 1000 };
    expect(motionValue(m, 400, 1000)).toEqual({ value: 0, done: false });
    // Before the start it still draws `from`.
    expect(motionValue(m, 400, 900)).toEqual({ value: 0, done: false });
    const mid = motionValue(m, 400, 1100);
    expect(mid.done).toBe(false);
    expect(mid.value).toBeCloseTo(400 * easeAt(0.5), 6);
    // The live target is what the motion heads for.
    expect(motionValue(m, 800, 1100).value).toBeCloseTo(800 * easeAt(0.5), 6);
    expect(motionValue(m, 400, 1200)).toEqual({ value: 400, done: true });
    expect(motionValue(m, 400, 5000)).toEqual({ value: 400, done: true });
    expect(LAYOUT_MOTION.ms).toBe(200);
  });

  it("a target that changes without a switch is followed at once", () => {
    const prev = { key: "beside", motion: null, drawn: 300 };
    const next = nextMotion(prev, "beside", 50, false);
    expect(next).toBeNull();
    expect(motionValue(next, 500, 50)).toEqual({ value: 500, done: true });
  });

  it("a second switch starts from the value on screen", () => {
    const first = nextMotion({ key: "a", motion: null, drawn: 0 }, "b", 0, false);
    expect(first).toEqual({ from: 0, startedAt: 0 });
    const drawn = motionValue(first, 400, 100).value;
    expect(drawn).toBeGreaterThan(0);
    expect(drawn).toBeLessThan(400);
    const second = nextMotion({ key: "b", motion: first, drawn }, "a", 100, false);
    expect(second).toEqual({ from: drawn, startedAt: 100 });
    // The same key keeps the motion that is running.
    expect(nextMotion({ key: "b", motion: first, drawn }, "b", 150, false)).toBe(first);
  });

  it("reduced motion never starts a motion", () => {
    expect(nextMotion({ key: "a", motion: null, drawn: 0 }, "b", 0, true)).toBeNull();
    // It also ends one that is running.
    expect(nextMotion({ key: "b", motion: { from: 0, startedAt: 0 }, drawn: 90 }, "a", 50, true)).toBeNull();
  });

  it("the constants match the tokens", () => {
    expect(tokens).toMatch(/--dur-drawer:\s*200ms/);
    expect(tokens).toContain("--ease-out: cubic-bezier(0.2, 0.8, 0.2, 1)");
    expect(LAYOUT_MOTION.curve).toEqual([0.2, 0.8, 0.2, 1]);
  });
});
