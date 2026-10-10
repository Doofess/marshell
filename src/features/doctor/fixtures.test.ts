import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../../components/DiffView/diff";
import { DOCTOR_PROBLEMS, DOCTOR_REPORT } from "./fixtures";
describe("doctor fixtures", () => {
  it("every fixable item has a file and a diff that parses", () => {
    for (const i of DOCTOR_REPORT.items.filter((x) => x.fixable)) {
      expect(i.file, i.id).toContain("Home Office");
      expect(parseUnifiedDiff(i.diff!).added, i.id).toBeGreaterThan(0);
    }
  });
  it("every unfixable item says why, in words", () => {
    for (const i of DOCTOR_PROBLEMS.items) expect(i.unfixableReason, i.id).toBeTruthy();
  });
  it("includes the managed settings as information that is never fixable", () => {
    const m = DOCTOR_REPORT.items.find((i) => i.severity === "info")!;
    expect(m.fixable).toBe(false);
    expect(m.detail).toContain("never changes");
  });
});
