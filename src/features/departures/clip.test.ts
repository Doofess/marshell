import { describe, expect, it } from "vitest";
import { flipDuration } from "./flip";
import { CLIP_DURATION_MS, CLIP_EVENTS, cellAt } from "./clip";
describe("the 10 second clip", () => {
  it("is ten seconds, in order, and flips only the status and waiting cells", () => {
    expect(CLIP_DURATION_MS).toBe(10_000);
    expect([...CLIP_EVENTS].map((e) => e.at)).toEqual([...CLIP_EVENTS].map((e) => e.at).sort((a, b) => a - b));
    expect(CLIP_EVENTS.every((e) => e.cell === "status" || e.cell === "waiting")).toBe(true);
    expect(CLIP_EVENTS.every((e) => e.at + flipDuration(Math.max(e.from.length, e.to.length)) <= CLIP_DURATION_MS)).toBe(true);
  });
  it("lets a cell finish flipping before it flips again", () => {
    for (const a of CLIP_EVENTS) for (const b of CLIP_EVENTS) if (a !== b && a.rowId === b.rowId && a.cell === b.cell && a.at < b.at) expect(b.at - a.at).toBeGreaterThanOrEqual(flipDuration(Math.max(a.from.length, a.to.length)));
  });
  it("reads the text of a cell at any time", () => {
    const e = CLIP_EVENTS[0]!;
    expect(cellAt(e.rowId, e.cell, e.from, e.at - 1)).toBe(e.from);
    expect(cellAt(e.rowId, e.cell, e.from, e.at + 1_000)).toBe(e.to);
  });
});
