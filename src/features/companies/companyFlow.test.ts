import { describe, expect, it } from "vitest";
import type { ApplyResult } from "../../generated/ApplyResult";
import type { SettingsPlan } from "../../generated/SettingsPlan";
import type { AgentSettingsView } from "../../generated/AgentSettingsView";
import { afterApply, afterEdit, afterPlan, afterReload, afterUndo, announcement, focusAfter, focusBack, isBusy, rawChange, rawDroppedLine } from "./companyFlow";

const plan = { preview: { changeset_id: "c" }, normalized: false } as unknown as SettingsPlan;
const P = "C:/u/.claude/settings.json";

describe("focusAfter", () => {
  it("moves focus to the review, to the result line, or back to Review changes, never onto Apply or Undo", () => {
    expect(focusAfter("planning", "review")).toBe("review");
    expect(focusAfter("applying", "saved")).toBe("result");
    expect(focusAfter("applying", "conflict")).toBe("result");
    expect(focusAfter("planning", "conflict")).toBe("result");
    expect(focusAfter("review", "editing")).toBe("review-button");
    expect(focusAfter("planning", "error")).toBe("result");
    expect(focusAfter("undoing", "error")).toBe("result");
    expect(focusAfter("saved", "undoing")).toBeNull();
    expect(focusAfter("saved", "editing")).toBeNull();
    expect(focusAfter("editing", "editing")).toBeNull();
  });
});

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
    expect(afterUndo({ ok: true }, P)).toEqual({ kind: "undone" });
    expect(afterUndo({ ok: false, status: 409, message: "x" }, P)).toEqual({ kind: "error", message: `${P} changed since this change, so undo would overwrite it.` });
    expect(afterUndo({ ok: false, status: 500, message: "Disk full." }, P)).toEqual({ kind: "error", message: "Disk full." });
  });
});
describe("afterEdit and isBusy", () => {
  it("returns saved, conflict and error to editing, and leaves the rest", () => {
    for (const r of [{ kind: "saved", snapshotId: "s" }, { kind: "conflict" }, { kind: "error", message: "m" }] as const) expect(afterEdit(r)).toEqual({ kind: "editing" });
    for (const r of [{ kind: "planning" }, { kind: "applying" }, { kind: "undoing", snapshotId: "s" }, { kind: "editing" }] as const) expect(afterEdit(r)).toBe(r);
    expect(afterEdit({ kind: "undone" })).toEqual({ kind: "editing" });
    expect(isBusy({ kind: "undone" })).toBe(false);
    expect(focusAfter("undoing", "undone")).toBe("result");
    expect(announcement({ kind: "undone" }, "n", "p")).toBe("Put p back as it was.");
    expect(focusBack("discard", "field:model", "raw")).toBe("field:model");
    expect(focusBack("reload", "field:model", "raw")).toBe("raw");
    expect(focusBack("discard", null, null)).toBeNull();
    expect(isBusy({ kind: "planning" })).toBe(true);
    expect(isBusy({ kind: "applying" })).toBe(true);
    expect(isBusy({ kind: "undoing", snapshotId: "s" })).toBe(true);
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

const sv = (over: Partial<AgentSettingsView> = {}): AgentSettingsView => ({ agent: "codex", path: P, state: { kind: "ok" }, curated: [], others: [], raw: "a = 1\n", editable: true, hash: "h", text: "a = 1\n", raw_lock: null, ...over });
describe("afterReload", () => {
  it("keeps the form draft through a conflict and Reload, and the raw draft only while the file's text is the same", () => {
    expect(afterReload("a = 1\n", sv(), { a: "2" }, "a = 3\n")).toEqual({ draft: { a: "2" }, rawDraft: "a = 3\n", rawDropped: false });
    expect(afterReload("a = 1\n", sv({ text: "a = 1\nb = 2\n" }), { a: "2" }, "a = 3\n")).toEqual({ draft: { a: "2" }, rawDraft: null, rawDropped: true });
    expect(afterReload("a = 1\n", sv({ text: null }), {}, "a = 3\n")).toEqual({ draft: {}, rawDraft: null, rawDropped: true });
    expect(afterReload("a = 1\n", sv({ text: "b = 2\n" }), {}, null)).toEqual({ draft: {}, rawDraft: null, rawDropped: false });
  });
  it("says why the raw edit went", () => {
    expect(rawDroppedLine(P)).toBe(`${P} changed, so your raw edit was dropped. The text is the file as it is now.`);
  });
});
describe("rawChange", () => {
  it("gives the text to send only for the raw file shown, editable, with text, and changed", () => {
    expect(rawChange("raw", sv(), "a = 2\n")).toBe("a = 2\n");
    expect(rawChange("form", sv(), "a = 2\n")).toBeNull();
    expect(rawChange("raw", sv(), "a = 1\n")).toBeNull();
    expect(rawChange("raw", sv(), null)).toBeNull();
    expect(rawChange("raw", sv({ text: null }), "a = 2\n")).toBeNull();
    expect(rawChange("raw", sv({ editable: false }), "a = 2\n")).toBeNull();
    expect(rawChange("raw", sv({ raw: null }), "a = 2\n")).toBeNull();
    expect(rawChange("raw", null, "a = 2\n")).toBeNull();
    expect(rawChange("raw", sv({ raw_lock: "not_plain_json", text: "x" }), "y")).toBeNull();
    expect(rawChange("raw", sv({ raw_lock: null, text: "x" }), "y")).toBe("y");
  });
});
