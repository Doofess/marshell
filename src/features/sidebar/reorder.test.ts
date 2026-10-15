import { describe, expect, it } from "vitest";
import { dropOrder, moveBy } from "./reorder";

describe("moveBy", () => {
  it("moves a session one place up or down", () => {
    expect(moveBy(["a", "b", "c"], "b", -1)).toEqual(["b", "a", "c"]);
    expect(moveBy(["a", "b", "c"], "b", 1)).toEqual(["a", "c", "b"]);
  });
  it("returns null at the ends and for an unknown id, so nothing is sent", () => {
    expect(moveBy(["a", "b", "c"], "a", -1)).toBeNull();
    expect(moveBy(["a", "b", "c"], "c", 1)).toBeNull();
    expect(moveBy(["a", "b", "c"], "x", 1)).toBeNull();
  });
});

describe("dropOrder", () => {
  it("drops before or after the row it lands on", () => {
    expect(dropOrder(["a", "b", "c"], "c", "a", "before")).toEqual(["c", "a", "b"]);
    expect(dropOrder(["a", "b", "c"], "a", "c", "after")).toEqual(["b", "c", "a"]);
    expect(dropOrder(["a", "b", "c"], "a", "b", "after")).toEqual(["b", "a", "c"]);
  });
  it("returns null when the order would not change", () => {
    expect(dropOrder(["a", "b", "c"], "b", "b", "before")).toBeNull();
    expect(dropOrder(["a", "b", "c"], "b", "a", "after")).toBeNull();
    expect(dropOrder(["a", "b", "c"], "b", "c", "before")).toBeNull();
    expect(dropOrder(["a", "b", "c"], "x", "a", "before")).toBeNull();
  });
});
