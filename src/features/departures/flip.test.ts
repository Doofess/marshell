import { describe, expect, it } from "vitest";
import { FLIP_CAP_MS, flipDuration, flipFrame, flipFrames, settleAt } from "./flip";

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
