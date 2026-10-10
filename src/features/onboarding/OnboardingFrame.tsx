import type { ReactNode } from "react";
import { nextStep, prevStep, STEPS, stepLabel, type Step } from "./onboardingFlow";
import "./Onboarding.css";

/** The frame around each of the four screens: step text, a four-segment progress bar drawn with fills, one primary button, an optional secondary, and Back from step 2. Not a live region. */
export function OnboardingFrame({ step, title, primary, secondary, children }: { step: Step; title: string; primary: string; secondary?: string; children: ReactNode }) {
  const canGoBack = prevStep(step) !== step;
  const hasNext = nextStep(step) !== step;
  return (
    <section className="onboarding" aria-label={`Onboarding: ${title}`} data-step={step}>
      <ol className="onboarding__progress" aria-label="Progress">
        {STEPS.map((s) => (
          <li key={s} className="onboarding__segment" data-done={STEPS.indexOf(s) <= STEPS.indexOf(step) || undefined} aria-current={s === step ? "step" : undefined}>
            <span className="onboarding__segment-name">{stepLabel(s).split(": ")[1]}</span>
          </li>
        ))}
      </ol>
      <p className="onboarding__step">{stepLabel(step).split(": ")[0]}</p>
      <h2 className="onboarding__title">{title}</h2>
      <div className="onboarding__body">{children}</div>
      <footer className="onboarding__foot">
        {canGoBack && (
          <button type="button" className="onboarding__back">
            Back
          </button>
        )}
        {secondary && (
          <button type="button" className="onboarding__secondary">
            {secondary}
          </button>
        )}
        <button type="button" className="onboarding__primary" data-last={!hasNext || undefined}>
          {primary}
        </button>
      </footer>
    </section>
  );
}
