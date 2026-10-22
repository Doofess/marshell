import { describe, expect, it } from "vitest";
import { FLIP_CAP_MS, flipDuration, flipFrame, flipFrames, retarget, settleAt } from "./flip";

describe("flip schedule", () => {
  it("starts on the old text and ends on the new text", () => {
    expect(flipFrame("WORKING", "NEEDS YOU", 0)).toBe("WORKING");
    expect(flipFrame("WORKING", "NEEDS YOU", flipDuration(9))).toBe("NEEDS YOU");
  });
  it("settles each character 15 ms after the one before, 60 ms after it starts, and never takes more than 400 ms", () => {
    expect(settleAt(0)).toBe(60);
    expect(settleAt(1)).toBe(75);
    expect(flipDuration(100)).toBeLessThanOrEqual(FLIP_CAP_MS);
    expect(flipDuration(0)).toBe(0);
  });
  it("never changes a character again once it has settled", () => {
    const frames = flipFrames("ERROR", "DONE", 8);
    const target = "DONE ";
    for (let i = 0; i < 4; i++) {
      const at = frames.findIndex((f) => f.t >= settleAt(i));
      for (const f of frames.slice(at)) expect(f.text.padEnd(5)[i]).toBe(target[i]);
    }
  });
  it("is deterministic: the same time gives the same frame", () => {
    expect(flipFrame("IDLE", "WORKING", 100)).toBe(flipFrame("IDLE", "WORKING", 100));
  });
  it("keeps spaces where both texts have them", () => {
    expect(flipFrame("NEEDS YOU", "NEEDS YOU", 30)).toBe("NEEDS YOU");
  });
});

describe("retarget", () => {
  it("does not start a run when the settled text is unchanged", () => {
    expect(retarget(null, "IDLE", "IDLE", 0)).toBeNull();
  });
  it("starts a run from the settled text", () => {
    expect(retarget(null, "IDLE", "NEEDS YOU", 5)).toEqual({ from: "IDLE", to: "NEEDS YOU", start: 5 });
  });
  it("keeps the same run when its target is unchanged", () => {
    const run = { from: "NEEDS YOU", to: "WORKING", start: 0 };
    expect(retarget(run, "NEEDS YOU", "WORKING", 100)).toBe(run);
  });
  it("restarts from the text on screen when the target changes mid-flip", () => {
    const run = { from: "NEEDS YOU", to: "WORKING", start: 0 };
    expect(retarget(run, "NEEDS YOU", "DONE", 100)).toEqual({ from: flipFrame("NEEDS YOU", "WORKING", 100), to: "DONE", start: 100 });
  });
  it("settles on the latest target without leaving a stale frame", () => {
    const run = retarget({ from: "NEEDS YOU", to: "WORKING", start: 0 }, "NEEDS YOU", "DONE", 100)!;
    expect(flipFrame(run.from, run.to, 100 + flipDuration(9) - run.start)).toBe("DONE");
  });
});
