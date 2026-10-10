import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { OnboardingFrame } from "./OnboardingFrame";

const frame = (step: Parameters<typeof OnboardingFrame>[0]["step"]) =>
  renderToStaticMarkup(
    <OnboardingFrame step={step} title="Title" primary="Continue" secondary="Skip">
      body
    </OnboardingFrame>,
  );

describe("OnboardingFrame", () => {
  it("has exactly one primary button", () => {
    expect((frame("consent").match(/class="onboarding__primary"/g) ?? []).length).toBe(1);
  });
  it("offers Back from step 2 on, never on step 1", () => {
    expect(frame("consent")).toContain("Back");
    expect(frame("detected")).not.toContain("Back");
  });
  it("says which step it is, and marks the current progress segment", () => {
    const h = frame("notifications");
    expect(h).toContain("Step 3 of 4");
    expect((h.match(/aria-current="step"/g) ?? []).length).toBe(1);
  });
  it("has no live region and names its buttons from their text", () => {
    const h = frame("consent");
    expect(h).not.toMatch(/aria-live|role="status"|role="alert"/);
    for (const b of h.match(/<button[\s\S]*?<\/button>/g) ?? []) expect(nameContainsLabel(b)).toBe(true);
  });
});
