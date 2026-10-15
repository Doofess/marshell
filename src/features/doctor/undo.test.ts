import { describe, expect, it } from "vitest";
import { TOAST_MS, toastVisible, undoOffer, type UndoState } from "./undo";

const u: UndoState = { fixedAtMs: 1_000, count: 4, filesMatch: true };
describe("undo", () => {
  it("shows the toast for 10 seconds, then leaves Undo last fix in the header", () => {
    expect(TOAST_MS).toBe(10_000);
    expect(toastVisible(u, 1_000 + 9_999)).toBe(true);
    expect(toastVisible(u, 1_000 + 10_000)).toBe(false);
    expect(undoOffer(u, 2_000)).toEqual({ kind: "toast", text: "Undo" });
    expect(undoOffer(u, 20_000)).toEqual({ kind: "header", text: "Undo last fix" });
  });
  it("stops offering undo when the files no longer match, and says why", () => {
    expect(undoOffer({ ...u, filesMatch: false }, 20_000)).toEqual({ kind: "stale", text: "A file changed since the fix, so undo would overwrite it. Show differences." });
  });
});
