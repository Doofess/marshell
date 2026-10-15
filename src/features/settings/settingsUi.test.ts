import { describe, expect, it } from "vitest";
import { settingsAnnouncement } from "./settingsUi";

describe("settingsAnnouncement", () => {
  it("says an applied change once, as the result of something the user did, and names the backup", () => {
    expect(settingsAnnouncement({ applied: true }, { a: 1, b: 2 })).toEqual({ text: "Applied 2 changes to Claude Code settings. A backup was saved.", kind: "action-result" });
    expect(settingsAnnouncement({ applied: true }, { a: 1 })?.text).toBe("Applied 1 change to Claude Code settings. A backup was saved.");
  });
  it("says nothing until something was applied", () => {
    expect(settingsAnnouncement({}, { a: 1 })).toBeNull();
    expect(settingsAnnouncement({ changed: true, reviewing: true }, { a: 1 })).toBeNull();
  });
  it("never uses the banned words", () => {
    expect(settingsAnnouncement({ applied: true }, { a: 1 })?.text).not.toMatch(/!|successfully|oops/i);
  });
});
