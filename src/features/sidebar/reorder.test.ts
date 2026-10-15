import { describe, expect, it } from "vitest";
import { dragOver, dropOrder, moveBy, TAB_MIME } from "./reorder";

describe("dragOver", () => {
  it("marks the edge of the row the pointer is over", () => {
    expect(dragOver({ id: "a" }, "b", "after")).toEqual({ id: "a", over: { id: "b", edge: "after" } });
  });
  it("clears the indicator when the pointer is over the dragged row itself", () => {
    expect(dragOver({ id: "a", over: { id: "b", edge: "before" } }, "a", "before")).toEqual({ id: "a" });
  });
});

describe("TAB_MIME", () => {
  it("is a custom type, so a dropped row never lands as text in a field", () => {
    expect(TAB_MIME).toBe("application/x-marshell-tab");
  });
});

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
