import { describe, expect, it } from "vitest";
import { conflictFrom, conflictsIn, countConflicts, type Conflict } from "./conflicts";

describe("conflictsIn", () => {
  it("finds a two-part conflict", () => {
    expect(conflictsIn(["a", "<<<<<<< mine", "x", "=======", "y", ">>>>>>> disk", "b"])).toEqual([
      { start: 2, base: null, middle: 4, end: 6 },
    ]);
  });
  it("finds a three-part one", () => {
    expect(conflictsIn(["a", "<<<<<<< mine", "x", "||||||| base", "o", "=======", "y", ">>>>>>> disk"])).toEqual([
      { start: 2, base: 4, middle: 6, end: 8 },
    ]);
  });
  it("finds a conflict with git's labels", () => {
    expect(conflictsIn(["<<<<<<< HEAD", "x", "=======", "y", ">>>>>>> feature/x"])).toEqual([
      { start: 1, base: null, middle: 3, end: 5 },
    ]);
  });
  it("finds two in one text", () => {
    expect(
      conflictsIn(["<<<<<<< mine", "=======", ">>>>>>> disk", "m", "<<<<<<< mine", "x", "=======", "y", ">>>>>>> disk"]),
    ).toEqual([
      { start: 1, base: null, middle: 2, end: 3 },
      { start: 5, base: null, middle: 7, end: 9 },
    ]);
  });
  it("ignores a lone ======= line", () => {
    expect(conflictsIn(["Title", "=======", "text"])).toEqual([]);
  });
  it("ignores a start with no end", () => {
    expect(conflictsIn(["<<<<<<< mine", "x", "=======", "y"])).toEqual([]);
  });
  it("starts over at a second start line", () => {
    expect(conflictsIn(["<<<<<<< a", "x", "<<<<<<< b", "=======", ">>>>>>> c"])).toEqual([
      { start: 3, base: null, middle: 4, end: 5 },
    ]);
  });
  it("ignores an end before a middle", () => {
    expect(conflictsIn(["<<<<<<< a", ">>>>>>> b", "=======", "x"])).toEqual([]);
  });
  it("needs exactly seven", () => {
    expect(conflictsIn(["<<<<<<", "<<<<<<<<", "<<<<<<<mine", "======= x"])).toEqual([]);
  });
  it("ignores a base line or a middle line outside a conflict", () => {
    expect(conflictsIn(["||||||| base", "=======", ">>>>>>> disk"])).toEqual([]);
  });
});

describe("countConflicts", () => {
  it("is 0 for an empty text and counts the conflicts of a text", () => {
    expect(countConflicts("")).toBe(0);
    expect(countConflicts("<<<<<<< mine\nx\n=======\ny\n>>>>>>> disk\n")).toBe(1);
  });
});

describe("conflictFrom", () => {
  const two: Conflict[] = [
    { start: 3, base: null, middle: 5, end: 7 },
    { start: 10, base: null, middle: 12, end: 14 },
  ];
  it("goes to the next conflict after the line, wrapping to the first", () => {
    expect(conflictFrom(two, 1, "next")).toBe(0);
    expect(conflictFrom(two, 3, "next")).toBe(1);
    expect(conflictFrom(two, 8, "next")).toBe(1);
    expect(conflictFrom(two, 10, "next")).toBe(0);
    expect(conflictFrom(two, 20, "next")).toBe(0);
  });
  it("goes to the previous conflict before the line, wrapping to the last", () => {
    expect(conflictFrom(two, 20, "previous")).toBe(1);
    expect(conflictFrom(two, 10, "previous")).toBe(0);
    expect(conflictFrom(two, 5, "previous")).toBe(0);
    expect(conflictFrom(two, 3, "previous")).toBe(1);
    expect(conflictFrom(two, 1, "previous")).toBe(1);
  });
  it("goes to the first", () => {
    expect(conflictFrom(two, 20, "first")).toBe(0);
  });
  it("is null when there are none", () => {
    for (const which of ["first", "next", "previous"] as const) expect(conflictFrom([], 1, which)).toBeNull();
  });
});
