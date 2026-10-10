import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../../components/DiffView/diff";
import { applyChanges, changeSummary, fileChanged, pendingChanges, previewChange } from "./edit";
import type { SettingField } from "./fields";

const before = '{\n  "model": "opus",\n  "cleanupPeriodDays": 30\n}\n';
describe("edit", () => {
  it("sets keys and keeps the rest and the order", () => {
    expect(JSON.parse(applyChanges(before, { cleanupPeriodDays: 365 }))).toEqual({ model: "opus", cleanupPeriodDays: 365 });
    expect(Object.keys(JSON.parse(applyChanges(before, { theme: "dark" })))).toEqual(["model", "cleanupPeriodDays", "theme"]);
  });
  it("previews the exact change as a diff of the whole file", () => {
    const p = parseUnifiedDiff(previewChange("C:/Users/Home Office/.claude/settings.json", before, { cleanupPeriodDays: 365 }));
    expect(p.added).toBe(1);
    expect(p.removed).toBe(1);
    expect(p.files).toEqual(["C:/Users/Home Office/.claude/settings.json"]);
  });
  it("previews nothing when nothing changes", () => expect(previewChange("a.json", before, { model: "opus" })).toBe(""));
  it("only counts a field whose value really changed", () => {
    const fields = [{ key: "a", value: 1 }, { key: "b", value: "x" }] as SettingField[];
    expect(pendingChanges(fields, { a: 1, b: "y" })).toEqual({ b: "y" });
  });
  it("words the count", () => {
    expect([0, 1, 2].map(changeSummary)).toEqual(["No changes", "1 change", "2 changes"]);
  });
  it("notices that the file changed on disk after it was opened", () => {
    expect(fileChanged(before, before)).toBe(false);
    expect(fileChanged(before, before + " ")).toBe(true);
  });
});
