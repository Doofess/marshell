import { describe, expect, it } from "vitest";
import type { ApplyResult } from "../../generated/ApplyResult";
import type { SettingsPlan } from "../../generated/SettingsPlan";
import { afterApply, afterEdit, afterPlan, afterUndo, announcement, isBusy } from "./companyFlow";

const plan = { preview: { changeset_id: "c" }, normalized: false } as unknown as SettingsPlan;
const P = "C:/u/.claude/settings.json";

describe("afterPlan", () => {
  it("goes to review, conflict on a 409, and error with the core's sentence otherwise", () => {
    expect(afterPlan({ ok: true, plan })).toEqual({ kind: "review", plan });
    expect(afterPlan({ ok: false, status: 409, message: "x" })).toEqual({ kind: "conflict" });
    expect(afterPlan({ ok: false, status: 422, message: "Not valid JSON." })).toEqual({ kind: "error", message: "Not valid JSON." });
  });
});
describe("afterApply", () => {
  it("keeps the snapshot id, and tells a conflict from a failure", () => {
    expect(afterApply({ ok: true, result: { snapshot_id: "s1", files: [] } as ApplyResult })).toEqual({ kind: "saved", snapshotId: "s1" });
    expect(afterApply({ ok: false, conflict: true })).toEqual({ kind: "conflict" });
    expect(afterApply({ ok: false, conflict: false, message: "Nope." })).toEqual({ kind: "error", message: "Nope." });
  });
});
describe("afterUndo", () => {
  it("goes back to editing, or errors with the exact sentence", () => {
    expect(afterUndo({ ok: true }, P)).toEqual({ kind: "editing" });
    expect(afterUndo({ ok: false, status: 409, message: "x" }, P)).toEqual({ kind: "error", message: `${P} changed since this change, so undo would overwrite it.` });
    expect(afterUndo({ ok: false, status: 500, message: "Disk full." }, P)).toEqual({ kind: "error", message: "Disk full." });
  });
});
describe("afterEdit and isBusy", () => {
  it("returns saved, conflict and error to editing, and leaves the rest", () => {
    for (const r of [{ kind: "saved", snapshotId: "s" }, { kind: "conflict" }, { kind: "error", message: "m" }] as const) expect(afterEdit(r)).toEqual({ kind: "editing" });
    for (const r of [{ kind: "planning" }, { kind: "applying" }, { kind: "editing" }] as const) expect(afterEdit(r)).toBe(r);
    expect(isBusy({ kind: "planning" })).toBe(true);
    expect(isBusy({ kind: "applying" })).toBe(true);
    expect(isBusy({ kind: "editing" })).toBe(false);
  });
});
describe("announcement", () => {
  it("says the exact text for each landing", () => {
    expect(announcement({ kind: "review", plan }, "Claude Code", P)).toBe(`Review the change to ${P}.`);
    expect(announcement({ kind: "saved", snapshotId: "s" }, "Claude Code", P)).toBe(`Saved ${P}.`);
    expect(announcement({ kind: "conflict" }, "Claude Code", P)).toBe(`${P} changed since you opened it. Reload to see it.`);
    expect(announcement({ kind: "error", message: "Bad." }, "Claude Code", P)).toBe("Bad.");
    expect(announcement({ kind: "editing" }, "Claude Code", P)).toBeNull();
    expect(announcement({ kind: "planning" }, "Claude Code", P)).toBeNull();
  });
});
