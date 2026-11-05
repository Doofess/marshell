import { describe, expect, it } from "vitest";
import { DOCTOR_CLEAR, DOCTOR_PROBLEMS, DOCTOR_REPORT } from "./fixtures";
import { afterFixMessage, counts, fixHeadline, groups, initialSelection, listNumbers, selectedFixable } from "./report";

describe("counts", () => {
  it("counts issues, not Info, and splits fixable from not; offered counts every fixable item", () => {
    expect(counts(DOCTOR_REPORT)).toEqual({ issues: 3, fixable: 2, unfixable: 1, offered: 3 });
    expect(counts(DOCTOR_PROBLEMS)).toMatchObject({ unfixable: 2, fixable: 0 });
    expect(counts(DOCTOR_CLEAR)).toEqual({ issues: 0, fixable: 0, unfixable: 0, offered: 0 });
  });
});
describe("initialSelection", () => {
  it("pre-checks fixable Broken items only", () => {
    expect([...initialSelection(DOCTOR_REPORT)].sort()).toEqual(["hooks.claude", "hooks.codex@Ubuntu"]);
  });
  it("keeps the earlier choice for an id in the previous report, and decides new ids afresh", () => {
    const prevReport = { ...DOCTOR_REPORT, items: DOCTOR_REPORT.items.filter((i) => i.id !== "hooks.codex@Ubuntu") };
    const prev = new Set(["retention.claude"]);
    const sel = initialSelection(DOCTOR_REPORT, prev, prevReport);
    expect(sel.has("hooks.claude")).toBe(false);
    expect(sel.has("retention.claude")).toBe(true);
    expect(sel.has("hooks.codex@Ubuntu")).toBe(true);
  });
  it("never selects an item that cannot be fixed", () => {
    expect(initialSelection(DOCTOR_PROBLEMS).size).toBe(0);
  });
});
describe("selectedFixable", () => {
  it("returns the fixable items whose id is selected", () => {
    expect(selectedFixable(DOCTOR_REPORT, new Set(["hooks.claude", "hooks.antigravity", "managed.claude"])).map((i) => i.id)).toEqual(["hooks.claude"]);
  });
});
describe("fixHeadline", () => {
  it("pluralises", () => {
    expect(fixHeadline(1)).toBe("Fix 1 issue");
    expect(fixHeadline(3)).toBe("Fix 3 issues");
  });
});
describe("groups", () => {
  it("orders Broken, Waste, Info and drops the empty ones", () => {
    // Nothing in 7a is Waste yet, so one Broken item stands in for it, listed last.
    const first = DOCTOR_REPORT.items[0]!;
    const withWaste = { ...DOCTOR_REPORT, items: [...DOCTOR_REPORT.items, { ...first, id: "waste.one", group: "waste" as const }] };
    expect(groups(withWaste).map((g) => [g.heading, g.items.length])).toEqual([["Broken", 3], ["Waste", 1], ["Info", 2]]);
    expect(groups(DOCTOR_PROBLEMS).map((g) => g.heading)).toEqual(["Broken"]);
    expect(groups(DOCTOR_CLEAR)).toEqual([]);
  });
});
describe("listNumbers", () => {
  it("joins session numbers in order with and", () => {
    expect(listNumbers([3])).toBe("3");
    expect(listNumbers([5, 3])).toBe("3 and 5");
    expect(listNumbers([1, 2, 3])).toBe("1, 2 and 3");
  });
});
describe("afterFixMessage", () => {
  it("words the result singular and plural, with or without a restart", () => {
    expect(afterFixMessage(4, ["a", "b"])).toBe("Fixed 4 issues. 2 sessions pick this up after a restart.");
    expect(afterFixMessage(1, ["a"])).toBe("Fixed 1 issue. 1 session picks this up after a restart.");
    expect(afterFixMessage(2, [])).toBe("Fixed 2 issues.");
    expect(afterFixMessage(4, ["a"])).not.toMatch(/!|successfully|oops/i);
  });
});
