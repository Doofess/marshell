import { describe, expect, it } from "vitest";
import { DOCTOR_PLAN, DOCTOR_REPORT, UNDO_FIXED } from "./fixtures";
import { doctorReducer, INITIAL_DOCTOR, type DoctorModel } from "./doctorModel";
import type { DoctorReport } from "./types";

const run = (s: DoctorModel, ...a: Parameters<typeof doctorReducer>[1][]) => a.reduce(doctorReducer, s);
const loaded = () => run(INITIAL_DOCTOR, { type: "runStarted" }, { type: "reportLoaded", report: DOCTOR_REPORT });

describe("doctorReducer", () => {
  it("starts running with nothing, then is ready with the fixable Broken rows checked", () => {
    expect(INITIAL_DOCTOR.phase).toBe("running");
    const s = loaded();
    expect(s.phase).toBe("ready");
    expect(s.report).toBe(DOCTOR_REPORT);
    expect([...s.selected]).toEqual(DOCTOR_REPORT.items.filter((i) => i.fixable && i.group === "broken").map((i) => i.id));
  });
  it("goes ready, fixing, fixed through a plan and an apply", () => {
    let s = run(loaded(), { type: "planned", plan: DOCTOR_PLAN });
    expect(s.plan).toBe(DOCTOR_PLAN);
    s = run(s, { type: "fixStarted" });
    expect(s.phase).toBe("fixing");
    s = run(s, { type: "fixApplied", fix: UNDO_FIXED });
    expect(s.phase).toBe("fixed");
    expect(s.plan).toBeNull();
    expect(s.undo).toBe(UNDO_FIXED);
  });
  it("keeps the person's checks across a new report", () => {
    const first = DOCTOR_REPORT.items.find((i) => i.fixable && i.group === "broken")!;
    let s = run(loaded(), { type: "toggle", id: first.id });
    expect(s.selected.has(first.id)).toBe(false);
    const next: DoctorReport = { ...DOCTOR_REPORT, checked_at_ms: DOCTOR_REPORT.checked_at_ms + 1 };
    s = run(s, { type: "runStarted" }, { type: "reportLoaded", report: next });
    expect(s.selected.has(first.id)).toBe(false);
    expect(s.phase).toBe("ready");
  });
  it("shows the conflict on the sheet, and a reload plans again", () => {
    let s = run(loaded(), { type: "planned", plan: DOCTOR_PLAN }, { type: "fixStarted" }, { type: "fixConflict" });
    expect(s.phase).toBe("ready");
    expect(s.conflict).toBe(true);
    expect(s.plan).toBe(DOCTOR_PLAN);
    s = run(s, { type: "planned", plan: DOCTOR_PLAN });
    expect(s.conflict).toBe(false);
  });
  it("closes the sheet on cancel or on a failed fix", () => {
    const planned = run(loaded(), { type: "planned", plan: DOCTOR_PLAN });
    expect(run(planned, { type: "cancelPlan" }).plan).toBeNull();
    const s = run(planned, { type: "fixStarted" }, { type: "fixFailed" });
    expect(s.plan).toBeNull();
    expect(s.phase).toBe("ready");
  });
  it("keeps a report that arrives during a fix out of the way", () => {
    const fixed = run(loaded(), { type: "planned", plan: DOCTOR_PLAN }, { type: "fixStarted" }, { type: "fixApplied", fix: UNDO_FIXED });
    const s = run(fixed, { type: "reportLoaded", report: { ...DOCTOR_REPORT, items: [] } });
    expect(s.phase).toBe("fixed");
    expect(s.report).toBe(DOCTOR_REPORT);
  });
  it("reads the undo record when the page opens, after a restart, without leaving ready", () => {
    const s = run(loaded(), { type: "undoLoaded", fix: { ...UNDO_FIXED, restart: [] } });
    expect(s.phase).toBe("ready");
    expect(s.undo?.restart).toEqual([]);
    expect(run(s, { type: "undoLoaded", fix: null }).undo).toBeNull();
  });
  it("withdraws undo when files changed, and clears it once undone", () => {
    const s = run(loaded(), { type: "undoLoaded", fix: UNDO_FIXED }, { type: "undoStale" });
    expect(s.undo?.files_match).toBe(false);
    expect(run(s, { type: "undone" }).undo).toBeNull();
    expect(run(s, { type: "undone" }).phase).toBe("running");
  });
  it("fails a run with its reason, and Run again starts over", () => {
    const s = run(INITIAL_DOCTOR, { type: "runFailed", message: "no core" });
    expect(s.phase).toBe("failed");
    expect(s.failure).toBe("no core");
    expect(run(s, { type: "runStarted" }).phase).toBe("running");
  });
});
