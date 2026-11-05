import { describe, expect, it } from "vitest";
import { fixAnnouncement, shownIssues, undoRefusedAnnouncement } from "./doctorUi";
import { UNDO_FIXED } from "./fixtures";

describe("fixAnnouncement", () => {
  it("says the result the user caused, as an action result", () => {
    // The fix included Codex's hooks in a distro, so it ends with the trust step.
    expect(fixAnnouncement(UNDO_FIXED)).toEqual({ text: "Fixed 3 issues. 2 sessions pick this up after a restart. Codex runs the changed hooks once you trust them in /hooks.", kind: "action-result" });
  });
});

// Found in the 7a final review: a refused undo only withdrew the button, so a screen reader said nothing.
describe("undoRefusedAnnouncement", () => {
  it("says nothing was undone and names the changed files, as an action result", () => {
    expect(undoRefusedAnnouncement(["C:\\u\\.claude\\settings.json"])).toEqual({
      text: "Nothing was undone: C:\\u\\.claude\\settings.json changed since the fix, so undo would overwrite it.",
      kind: "action-result",
    });
    expect(undoRefusedAnnouncement(["a.json", "b.json", "c.json"]).text).toBe("Nothing was undone: a.json, b.json and c.json changed since the fix, so undo would overwrite them.");
    expect(undoRefusedAnnouncement([]).text).toBe("Nothing was undone: a file changed since the fix, so undo would overwrite it.");
  });
});

// Found in the 7a final review: the core's first frame comes before any check ends, and the sidebar read its 0 as
// "Doctor: no issues".
describe("shownIssues", () => {
  it("gives no count until a check has ended, then the last one's, also while the next one runs", () => {
    expect(shownIssues(null)).toBeUndefined();
    expect(shownIssues({ running: true, cli_running: false, issues: 0 })).toBeUndefined();
    expect(shownIssues({ running: false, cli_running: false, issues: 0, checked_at_ms: 5 })).toBe(0);
    expect(shownIssues({ running: true, cli_running: false, issues: 3, checked_at_ms: 5 })).toBe(3);
  });
});
