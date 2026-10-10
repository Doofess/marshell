import { describe, expect, it } from "vitest";
import { freezeOrder } from "./freeze";
describe("freezeOrder", () => {
  it("keeps every row where it is, appends new ones, and keeps a closed one in place marked stale", () => {
    expect(freezeOrder(["a", "b", "c"], ["c", "x", "a"])).toEqual({ order: ["a", "b", "c", "x"], stale: ["b"] });
  });
  it("is the incoming order when nothing was there", () => {
    expect(freezeOrder([], ["a", "b"])).toEqual({ order: ["a", "b"], stale: [] });
  });
});
