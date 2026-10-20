import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { focusOrder } from "../../lib/focusOrder";
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
  it("its scrolling body is a focusable labelled region, first in the tab order before Back, the secondary and the primary", () => {
    const h = frame("consent");
    expect(h).toMatch(/<div class="onboarding__body" role="region" aria-label="Step content" tabindex="0">/);
    expect(focusOrder(h)).toEqual(["Step content", "Back", "Skip", "Continue"]);
  });
  // The window's focus goes to the title on open and on each step (useDialogFocus), never to an action, and stays out of the Tab order.
  it("its title takes the focus without joining the tab order", () => {
    const h = frame("detected");
    expect(h).toContain('<h2 class="onboarding__title" tabindex="-1">Title</h2>');
    expect(focusOrder(h)).toEqual(["Step content", "Skip", "Continue"]);
  });
  it("has no live region and names its buttons from their text", () => {
    const h = frame("consent");
    expect(h).not.toMatch(/aria-live|role="status"|role="alert"/);
    for (const b of h.match(/<button[\s\S]*?<\/button>/g) ?? []) expect(nameContainsLabel(b)).toBe(true);
  });
});
