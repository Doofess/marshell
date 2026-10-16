import { describe, expect, it } from "vitest";
import { groupBounds, groupedOrder, groupRows, moveWithinGroup } from "./grouping";
import type { RowModel } from "./types";

const row = (id: string, project: string) => ({ id, project }) as RowModel;
const rows = [row("a1", "A"), row("b1", "B"), row("a2", "A"), row("c1", "C")];

describe("groupRows", () => {
  it("groups follow first appearance and keep core order", () => {
    const groups = groupRows(rows);
    expect(groups.map((g) => [g.project, g.rows.map((r) => r.id), g.start])).toEqual([
      ["A", ["a1", "a2"], 0],
      ["B", ["b1"], 2],
      ["C", ["c1"], 3],
    ]);
    expect(groupedOrder(groups).map((r) => r.id)).toEqual(["a1", "a2", "b1", "c1"]);
  });
  it("has no groups for no rows", () => {
    expect(groupRows([])).toEqual([]);
  });
});

describe("groupBounds", () => {
  it("gives the displayed range of the group holding the row", () => {
    const groups = groupRows(rows);
    expect(groupBounds(groups, "a2")).toEqual({ start: 0, end: 2 });
    expect(groupBounds(groups, "c1")).toEqual({ start: 3, end: 4 });
    expect(groupBounds(groups, "x")).toBeUndefined();
  });
});

describe("moveWithinGroup", () => {
  it("moves inside the group in the displayed order, and sends every id", () => {
    expect(moveWithinGroup(rows, "a2", -1)).toEqual(["a2", "a1", "b1", "c1"]);
  });
  it("stops at the group edge", () => {
    expect(moveWithinGroup(rows, "a2", 1)).toBeNull();
    expect(moveWithinGroup(rows, "b1", -1)).toBeNull();
    expect(moveWithinGroup(rows, "x", 1)).toBeNull();
  });
});
