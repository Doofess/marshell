import { describe, expect, it } from "vitest";
import { fixAnnouncement } from "./doctorUi";
import { UNDO_FIXED } from "./fixtures";

describe("fixAnnouncement", () => {
  it("says the result the user caused, as an action result", () => {
    expect(fixAnnouncement(UNDO_FIXED, [3, 5])).toEqual({ text: "Fixed 4 issues. 2 sessions pick this up after a restart.", kind: "action-result" });
  });
});
