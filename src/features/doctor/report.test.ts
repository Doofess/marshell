import { describe, expect, it } from "vitest";
import { afterFixMessage, counts, fixHeadline, groupByFile, listNumbers, restartSessionsOf } from "./report";
import { DOCTOR_CLEAR, DOCTOR_PROBLEMS, DOCTOR_REPORT } from "./fixtures";

describe("counts", () => {
  it("counts issues, not information, and splits fixable from not", () => {
    expect(counts(DOCTOR_REPORT)).toEqual({ issues: 4, fixable: 4, unfixable: 0 });
    expect(counts(DOCTOR_PROBLEMS)).toMatchObject({ unfixable: 3, fixable: 0 });
    expect(counts(DOCTOR_CLEAR)).toEqual({ issues: 0, fixable: 0, unfixable: 0 });
  });
});
describe("fixHeadline", () => {
  it("pluralises and says when there is nothing to fix", () => {
    expect(fixHeadline(DOCTOR_REPORT)).toBe("Fix 4 issues");
    expect(fixHeadline({ ...DOCTOR_REPORT, items: DOCTOR_REPORT.items.slice(0, 1) })).toBe("Fix 1 issue");
    expect(fixHeadline(DOCTOR_PROBLEMS)).toBe("Nothing to fix");
  });
});
describe("groupByFile", () => {
  it("groups fixable items by file with one combined diff and its counts", () => {
    const g = groupByFile(DOCTOR_REPORT.items);
    expect(g.map((x) => x.file)).toEqual(["C:\Users\Home Office\.claude\settings.json"]);
    expect(g[0]!.items).toHaveLength(4);
    expect(g[0]!.added).toBeGreaterThan(0);
  });
  it("never includes an item that cannot be fixed or has no file", () => {
    expect(groupByFile(DOCTOR_PROBLEMS.items)).toEqual([]);
  });
});
describe("restart wording", () => {
  it("lists session numbers in words", () => {
    expect(listNumbers([3])).toBe("3");
    expect(listNumbers([3, 5])).toBe("3 and 5");
    expect(listNumbers([1, 2, 3])).toBe("1, 2 and 3");
  });
  it("unions and sorts the sessions that need a restart", () => {
    expect(restartSessionsOf(DOCTOR_REPORT.items)).toEqual([3, 5]);
  });
  it("words the result as the plan does, singular and plural, with or without a restart", () => {
    expect(afterFixMessage(4, [3, 5])).toBe("Fixed 4 issues. 2 sessions pick this up after a restart.");
    expect(afterFixMessage(1, [3])).toBe("Fixed 1 issue. 1 session picks this up after a restart.");
    expect(afterFixMessage(2, [])).toBe("Fixed 2 issues.");
  });
  it("never uses the banned words", () => {
    expect(afterFixMessage(4, [3, 5])).not.toMatch(/!|successfully|oops/i);
  });
});
