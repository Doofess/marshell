import { describe, expect, it } from "vitest";
import { skillFlowStartsOpen } from "./useSkillFlow";

describe("skillFlowStartsOpen", () => {
  it("opens the diff when an edited skill is about to be replaced", () => {
    expect(skillFlowStartsOpen("install", "edited")).toBe(true);
  });
  it("keeps it closed for every other skill state and for a removal", () => {
    expect(skillFlowStartsOpen("install", "outdated")).toBe(false);
    expect(skillFlowStartsOpen("install", "none")).toBe(false);
    expect(skillFlowStartsOpen("install", undefined)).toBe(false);
    expect(skillFlowStartsOpen("remove", "edited")).toBe(false);
  });
});
