import { describe, expect, it } from "vitest";
import { AUTO_SAVE_MS, autoSaveDue, autoSaveTimer, type AutoSaveFacts } from "./autoSave";

const ready: AutoSaveFacts = { on: true, dirty: true, conflict: false, markers: false, readOnly: false, saving: false };

describe("autoSaveDue", () => {
  it("is 1,000 ms", () => {
    expect(AUTO_SAVE_MS).toBe(1000);
  });
  it("is due only when on, dirty, not saving, not in conflict and not read-only", () => {
    expect(autoSaveDue(ready)).toBe(true);
    expect(autoSaveDue({ ...ready, on: false })).toBe(false);
    expect(autoSaveDue({ ...ready, dirty: false })).toBe(false);
    expect(autoSaveDue({ ...ready, saving: true })).toBe(false);
  });
  it("auto_save_stops_at_a_conflict", () => {
    expect(autoSaveDue({ ...ready, conflict: true })).toBe(false);
  });
  it("never_auto_saves_text_that_still_holds_conflict_markers", () => {
    // A merge that left conflicts is saved only by Ctrl+S.
    expect(autoSaveDue({ ...ready, markers: true })).toBe(false);
  });
  it("never_auto_saves_read_only", () => {
    expect(autoSaveDue({ ...ready, readOnly: true })).toBe(false);
  });
});

describe("autoSaveTimer", () => {
  const f = { ...ready, text: "b", lastSent: null as string | null };
  it("starts for a dirty tab that is idle", () => {
    expect(autoSaveTimer(f)).toBe(true);
    // Keys typed while a save was out: the text is not the one that was sent.
    expect(autoSaveTimer({ ...f, text: "bc", lastSent: "b" })).toBe(true);
  });
  it("does not start while a save is out, or when auto save could not run", () => {
    expect(autoSaveTimer({ ...f, saving: true })).toBe(false);
    expect(autoSaveTimer({ ...f, on: false })).toBe(false);
    expect(autoSaveTimer({ ...f, conflict: true })).toBe(false);
    expect(autoSaveTimer({ ...f, markers: true })).toBe(false);
    expect(autoSaveTimer({ ...f, readOnly: true })).toBe(false);
    expect(autoSaveTimer({ ...f, dirty: false })).toBe(false);
  });
  it("does not try again the text that was just sent and came back unsaved", () => {
    // A refused save leaves the tab dirty with the same text: the next try waits for a change, so a failure is not said every second.
    expect(autoSaveTimer({ ...f, lastSent: "b" })).toBe(false);
  });
});
