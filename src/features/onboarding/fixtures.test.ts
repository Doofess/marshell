import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../../components/DiffView/diff";
import { CONSENT, CONSENT_DIFFS, DETECTIONS } from "./fixtures";

describe("onboarding fixtures", () => {
  it("has a scenario with nothing installed", () => {
    expect(DETECTIONS.none.filter((d) => d.installed)).toHaveLength(0);
  });
  it("puts every installed CLI's config under a folder with a space in the main scenario", () => {
    for (const d of DETECTIONS.allThree.filter((x) => x.installed)) expect(d.configPath).toContain("Home Office");
  });
  it("builds the Claude consent diff from a real before and after, with at least nine additions", () => {
    expect(parseUnifiedDiff(CONSENT_DIFFS.claude).added).toBeGreaterThanOrEqual(9);
  });
  it("has a Codex diff long enough to collapse", () => {
    expect(parseUnifiedDiff(CONSENT_DIFFS.codexLong).lines.length).toBeGreaterThan(150);
  });
  it("keeps every backup under the Marshell snapshots folder", () => {
    for (const s of Object.values(CONSENT)) expect(s.plan.backupPath).toContain(".marshell\\snapshots");
  });
  it("covers the problem scenarios", () => {
    expect(DETECTIONS.unparsable.some((d) => d.configState === "unparsable" && d.parseError?.line === 14)).toBe(true);
    expect(DETECTIONS.readOnly.some((d) => d.configState === "read-only")).toBe(true);
    expect(DETECTIONS.managedPresent.some((d) => d.managed)).toBe(true);
  });
});
