import { describe, expect, it } from "vitest";
import { fixAnnouncement, shownIssues } from "./doctorUi";
import { UNDO_FIXED } from "./fixtures";

describe("fixAnnouncement", () => {
  it("says the result the user caused, as an action result", () => {
    expect(fixAnnouncement(UNDO_FIXED)).toEqual({ text: "Fixed 3 issues. 2 sessions pick this up after a restart.", kind: "action-result" });
  });
});

// Found in the 7a final review: the core's first frame comes before any check ends, and the sidebar read its 0 as
// "Doctor: no issues".
describe("shownIssues", () => {
  it("gives no count until a check has ended, then the last one's, also while the next one runs", () => {
    expect(shownIssues(null)).toBeUndefined();
    expect(shownIssues({ running: true, issues: 0 })).toBeUndefined();
    expect(shownIssues({ running: false, issues: 0, checked_at_ms: 5 })).toBe(0);
    expect(shownIssues({ running: true, issues: 3, checked_at_ms: 5 })).toBe(3);
  });
});
