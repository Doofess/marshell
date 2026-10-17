import { describe, expect, it } from "vitest";
import { dragOver, dragStarted, dropOrder, edgeAt, moveBy, rowIdAt } from "./reorder";

describe("dragOver", () => {
  it("marks the edge of the row the pointer is over", () => {
    expect(dragOver({ id: "a" }, "b", "after")).toEqual({ id: "a", over: { id: "b", edge: "after" } });
  });
  it("clears the indicator when the pointer is over the dragged row itself", () => {
    expect(dragOver({ id: "a", over: { id: "b", edge: "before" } }, "a", "before")).toEqual({ id: "a" });
  });
});

describe("dragStarted", () => {
  it("starts only once the pointer has moved 4 px from the press", () => {
    expect(dragStarted(0, 0)).toBe(false);
    expect(dragStarted(3, 0)).toBe(false);
    expect(dragStarted(4, 0)).toBe(true);
    expect(dragStarted(3, 3)).toBe(true);
    expect(dragStarted(-3, -3)).toBe(true);
  });
});

describe("edgeAt", () => {
  it("is before in the top half of the row and after in the bottom half", () => {
    expect(edgeAt(10, 0, 30)).toBe("before");
    expect(edgeAt(16, 0, 30)).toBe("after");
    expect(edgeAt(115, 100, 30)).toBe("after");
  });
});

describe("rowIdAt", () => {
  it("reads the id of the closest row, and is null for nothing or a non-row", () => {
    expect(rowIdAt(null)).toBeNull();
    const row = { getAttribute: (n: string) => (n === "data-row-id" ? "s1" : null) };
    expect(rowIdAt({ closest: (sel: string) => (sel === "[data-row-id]" ? row : null) } as unknown as Element)).toBe("s1");
    expect(rowIdAt({ closest: () => null } as unknown as Element)).toBeNull();
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
  it("moveBy stops at the group edge", () => {
    const ids = ["a", "b", "c", "d"];
    expect(moveBy(ids, "c", -1, { start: 2, end: 4 })).toBeNull();
    expect(moveBy(ids, "d", 1, { start: 2, end: 4 })).toBeNull();
    expect(moveBy(ids, "c", 1, { start: 2, end: 4 })).toEqual(["a", "b", "d", "c"]);
    expect(moveBy(ids, "a", 1, { start: 2, end: 4 })).toBeNull();
  });
});

describe("dropOrder with bounds", () => {
  const ids = ["a", "b", "c", "d"];
  it("drops inside the bounds as usual", () => {
    expect(dropOrder(ids, "c", "d", "after", { start: 2, end: 4 })).toEqual(["a", "b", "d", "c"]);
  });
  it("refuses a drop on or from a row outside the bounds", () => {
    expect(dropOrder(ids, "c", "b", "after", { start: 2, end: 4 })).toBeNull();
    expect(dropOrder(ids, "a", "d", "after", { start: 2, end: 4 })).toBeNull();
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
