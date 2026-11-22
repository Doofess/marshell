import { describe, expect, it } from "vitest";
import { guideLevels } from "./indentGuides";

describe("guideLevels", () => {
  it("counts levels from leading spaces and tabs", () => {
    expect(guideLevels(["a", "  b", "    c", "\tc"], 4, 2)).toEqual([0, 1, 2, 2]);
  });
  it("a blank line takes the smaller of its neighbours", () => {
    expect(guideLevels(["  a", "", "    b"], 4, 2)).toEqual([1, 1, 2]);
    expect(guideLevels(["    a", "   ", "  b"], 4, 2)).toEqual([2, 1, 1]);
  });
  it("blank lines at either end, and runs of them, take what neighbours they have", () => {
    expect(guideLevels(["", "  a"], 4, 2)).toEqual([0, 1]);
    expect(guideLevels(["  a", "", ""], 4, 2)).toEqual([1, 0, 0]);
    expect(guideLevels(["    a", "", "", "  b"], 4, 2)).toEqual([2, 1, 1, 1]);
  });
  it("a partial indent counts whole levels only, and a unit of 0 is read as 1", () => {
    expect(guideLevels(["   a"], 4, 2)).toEqual([1]);
    expect(guideLevels(["  a"], 4, 0)).toEqual([2]);
  });
});
