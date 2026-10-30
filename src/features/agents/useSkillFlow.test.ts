import { describe, expect, it } from "vitest";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import { PREVIEW } from "./fixtures";
import { skillFlowOpens, skillFlowStartsOpen } from "./useSkillFlow";

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

describe("skillFlowOpens", () => {
  const file = (before_hash?: string) => {
    const { before_hash: _gone, ...rest } = PREVIEW.files[0]!;
    return before_hash === undefined ? rest : { ...rest, before_hash };
  };
  const preview = (before_hash?: string): ConfigPreview => ({ ...PREVIEW, files: [file(before_hash)] });
  it("opens the diff when the preview replaces a file that is there, whatever the setup said", () => {
    expect(skillFlowOpens("install", "none", preview("abc"))).toBe(true);
    expect(skillFlowOpens("install", undefined, preview("abc"))).toBe(true);
  });
  it("keeps the starting rule when the preview creates the file", () => {
    expect(skillFlowOpens("install", "edited", preview(undefined))).toBe(true);
    expect(skillFlowOpens("install", "none", preview(undefined))).toBe(false);
  });
  it("never opens a removal", () => {
    expect(skillFlowOpens("remove", "installed", preview("abc"))).toBe(false);
  });
});
