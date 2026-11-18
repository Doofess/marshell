import { describe, expect, it } from "vitest";
import { changedLines } from "./changedLines";

const lines = (s: string) => s.split("");

describe("changedLines", () => {
  it("nothing_changed", () => {
    expect(changedLines(lines("abc"), lines("abc"))).toEqual([]);
    expect(changedLines([], [])).toEqual([]);
  });
  it("one_line_changed", () => {
    expect(changedLines(lines("abc"), lines("aXc"))).toEqual([2]);
  });
  it("a_line_added_in_the_middle", () => {
    expect(changedLines(lines("abc"), lines("abXc"))).toEqual([3]);
    expect(changedLines([], lines("ab"))).toEqual([1, 2]);
  });
  it("a_line_removed_marks_its_place", () => {
    expect(changedLines(lines("abc"), lines("ac"))).toEqual([2]);
    // Removed at the end: the last line.
    expect(changedLines(lines("abc"), lines("ab"))).toEqual([2]);
    // Removed at the start: the line now first.
    expect(changedLines(lines("abc"), lines("bc"))).toEqual([1]);
  });
  it("everything_removed", () => {
    expect(changedLines(lines("abc"), [])).toEqual([]);
  });
  it("a_huge_middle_is_marked_whole", () => {
    const before = ["head", ...Array.from({ length: 2001 }, (_, i) => `old ${i}`), "tail"];
    const after = ["head", ...Array.from({ length: 2001 }, (_, i) => `new ${i}`), "tail"];
    const marked = changedLines(before, after);
    expect(marked).toHaveLength(2001);
    expect(marked[0]).toBe(2);
    expect(marked[2000]).toBe(2002);
  });
  it("the_head_and_tail_are_never_marked", () => {
    expect(changedLines(lines("abcde"), lines("abXYe"))).toEqual([3, 4]);
    expect(changedLines(lines("abcde"), lines("abe"))).toEqual([3]);
  });
});
