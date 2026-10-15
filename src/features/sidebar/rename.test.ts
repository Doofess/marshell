import { describe, expect, it } from "vitest";
import { renameOutcome } from "./rename";

describe("renameOutcome", () => {
  it("Enter commits the trimmed new name", () => {
    expect(renameOutcome("Enter", "  api work ", "Shell 1")).toEqual({ kind: "commit", name: "api work" });
  });
  it("an empty or unchanged name cancels and keeps the original", () => {
    expect(renameOutcome("Enter", "   ", "Shell 1")).toEqual({ kind: "cancel" });
    expect(renameOutcome("Enter", "Shell 1", "Shell 1")).toEqual({ kind: "cancel" });
  });
  it("Escape cancels", () => {
    expect(renameOutcome("Escape", "api work", "Shell 1")).toEqual({ kind: "cancel" });
  });
  it("other keys do nothing", () => {
    expect(renameOutcome("a", "api work", "Shell 1")).toBeNull();
  });
  it("does not commit or cancel while an input method is composing: that Enter or Escape belongs to the composition", () => {
    expect(renameOutcome("Enter", "api work", "Shell 1", true)).toBeNull();
    expect(renameOutcome("Escape", "api work", "Shell 1", true)).toBeNull();
  });
});
