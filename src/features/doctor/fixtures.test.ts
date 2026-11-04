import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../../components/DiffView/diff";
import { DOCTOR_PLAN, DOCTOR_PROBLEMS, DOCTOR_REPORT } from "./fixtures";

describe("doctor fixtures", () => {
  it("the plan's files have diffs that parse, and its ids are fixable report items", () => {
    for (const f of DOCTOR_PLAN.preview.files) expect(parseUnifiedDiff(f.diff).added, f.path).toBeGreaterThan(0);
    for (const id of DOCTOR_PLAN.item_ids) expect(DOCTOR_REPORT.items.find((i) => i.id === id)?.fixable, id).toBe(true);
  });
  it("every item that is not fixable says why, except Info", () => {
    for (const i of [...DOCTOR_REPORT.items, ...DOCTOR_PROBLEMS.items].filter((x) => !x.fixable && x.group !== "info")) expect(i.cannot_fix, i.id).toBeTruthy();
  });
  it("includes managed settings as Info that is never fixable", () => {
    const m = DOCTOR_REPORT.items.find((i) => i.group === "info")!;
    expect(m.fixable).toBe(false);
    expect(m.detail).toContain("never changes");
  });
});
