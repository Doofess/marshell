import { describe, expect, it } from "vitest";
import { flipDuration } from "./flip";
import { CLIP_DURATION_MS, CLIP_EVENTS, cellAt, clipTrack } from "./clip";
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

describe("clipTrack: one cell's whole 10 seconds as frames", () => {
  const e = CLIP_EVENTS[0]!;
  const track = clipTrack(e.rowId, e.cell, e.from);
  it("starts on the baseline at 0 and ends on what cellAt says at the end", () => {
    expect(track[0]).toEqual({ t: 0, text: e.from });
    expect(track[track.length - 1]!.text).toBe(cellAt(e.rowId, e.cell, e.from, CLIP_DURATION_MS));
  });
  it("is in time order and only lists frames where the text changes", () => {
    for (let i = 1; i < track.length; i++) {
      expect(track[i]!.t).toBeGreaterThan(track[i - 1]!.t);
      expect(track[i]!.text).not.toBe(track[i - 1]!.text);
    }
  });
  it("is just the baseline for a cell no event touches", () => {
    expect(clipTrack("done-seen", "status", "DONE")).toEqual([{ t: 0, text: "DONE" }]);
  });
  it("matches cellAt at every frame", () => {
    for (const f of track) expect(cellAt(e.rowId, e.cell, e.from, f.t)).toBe(f.text);
  });
});
