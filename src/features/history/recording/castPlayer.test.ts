import { describe, expect, it } from "vitest";
import type { RecordingPart } from "../../../generated/RecordingPart";
import { clock, jumpTargets, nextTarget, playbackDelay } from "./castPlayer";

const part = (over: Partial<RecordingPart> = {}): RecordingPart => ({
  index: 0,
  file: "t1-1000.cast",
  started_ms: 1_000,
  ended_ms: 100_000,
  cols: 80,
  rows: 24,
  span_from_ms: 1_000,
  span_to_ms: 100_000,
  marks_ms: [],
  stopped: null,
  ...over,
});

describe("playbackDelay", () => {
  it("caps the gap, then divides by the speed", () => {
    expect(playbackDelay(10_000, 2, 2_000)).toBe(1_000);
    expect(playbackDelay(500, 0.5, 2_000)).toBe(1_000);
  });
  it("keeps the whole gap with no cap", () => {
    expect(playbackDelay(10_000, 1, null)).toBe(10_000);
    expect(playbackDelay(10_000, 4, null)).toBe(2_500);
  });
});

describe("jumpTargets", () => {
  it("merges a mark and a turn time within 500 ms, sorts, and keeps only turn times inside the part", () => {
    const p = part({ marks_ms: [50_000, 10_000] });
    expect(jumpTargets(p, [10_200, 70_000, 500, 200_000])).toEqual([10_000, 50_000, 70_000]);
  });
  it("keeps targets 500 ms or more apart", () => {
    expect(jumpTargets(part({ marks_ms: [10_000] }), [10_500])).toEqual([10_000, 10_500]);
  });
});

describe("nextTarget", () => {
  const t = [10_000, 20_000, 30_000];
  it("goes forward to the first target after the position", () => {
    expect(nextTarget(t, 10_000, 1)).toBe(20_000);
    expect(nextTarget(t, 30_000, 1)).toBeNull();
  });
  it("at a target's own position goes back to the one before", () => {
    expect(nextTarget(t, 20_000, -1)).toBe(10_000);
    expect(nextTarget(t, 20_500, -1)).toBe(10_000);
    expect(nextTarget(t, 21_500, -1)).toBe(20_000);
    expect(nextTarget(t, 10_000, -1)).toBeNull();
  });
});

describe("clock", () => {
  it("reads m:ss, or h:mm:ss for an hour or more", () => {
    expect(clock(83_000, 760_000)).toBe("1:23");
    expect(clock(3_723_000, 4_000_000)).toBe("1:02:03");
    expect(clock(5_000, 4_000_000)).toBe("0:00:05");
    expect(clock(-5, 60_000)).toBe("0:00");
  });
});
