import { describe, expect, it } from "vitest";
import { shellRespawn } from "./shellRespawn";

describe("shellRespawn", () => {
  it("the first exit starts another", () => {
    expect(shellRespawn([], 1000)).toEqual({ again: true, exits: [1000] });
  });
  it("the third exit within ten seconds stops", () => {
    expect(shellRespawn([1000, 4000], 9000).again).toBe(false);
  });
  it("old exits are forgotten", () => {
    expect(shellRespawn([1000, 2000], 20_000)).toEqual({ again: true, exits: [20_000] });
  });
});
