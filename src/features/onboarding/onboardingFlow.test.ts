import { describe, expect, it } from "vitest";
import { nextStep, ONBOARDING_BUDGET_MS, prevStep, stepLabel, STEPS } from "./onboardingFlow";

describe("onboarding flow", () => {
  it("is four steps in the plan's order within a 60 second budget", () => {
    expect(STEPS).toEqual(["detected", "consent", "notifications", "first-session"]);
    expect(ONBOARDING_BUDGET_MS).toBe(60_000);
  });
  it("clamps at both ends", () => {
    expect(prevStep("detected")).toBe("detected");
    expect(nextStep("first-session")).toBe("first-session");
    expect(nextStep("detected")).toBe("consent");
  });
  it("labels a step with its number and name", () => {
    expect(stepLabel("consent")).toBe("Step 2 of 4: Consent");
  });
});
