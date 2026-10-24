import { useEffect, useRef, type ReactNode } from "react";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { nextStep, prevStep, STEPS, stepLabel, type Step } from "./onboardingFlow";
import "./Onboarding.css";

/** The frame around each of the four screens: step text, a four-segment progress bar drawn with fills, a scrolling body that is itself a focusable region (so the keyboard can scroll it), one primary button, an optional secondary, and Back from step 2. Not a live region. */
export function OnboardingFrame({
  step,
  agent,
  title,
  primary,
  secondary,
  children,
  onPrimary,
  onSecondary,
  onBack,
  primaryDisabled = false,
}: {
  step: Step;
  /** The agent whose card the consent step shows. Claude's card and Codex's are the same step, so a change of agent starts the focus again too. */
  agent?: string;
  title: string;
  primary: string;
  secondary?: string;
  children: ReactNode;
  /** The live window gives the buttons something to do; the stories leave them out. */
  onPrimary?: () => void;
  onSecondary?: () => void;
  onBack?: () => void;
  primaryDisabled?: boolean;
}) {
  const frame = useRef<HTMLElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  // It covers the window like a modal: the title takes the focus (not an action, so Enter applies nothing by reflex) and Tab
  // stays inside. It has no Escape: the onboarding ends through its buttons.
  useDialogFocus(frame, heading);
  // A new step, or the next agent's card, replaces the buttons and their text, so the focus starts again from the title.
  useEffect(() => heading.current?.focus(), [step, agent]);
  const canGoBack = prevStep(step) !== step;
  const hasNext = nextStep(step) !== step;
  return (
    <section ref={frame} className="onboarding" aria-label={`Onboarding: ${title}`} data-step={step} data-agent={agent}>
      <ol className="onboarding__progress" aria-label="Progress">
        {STEPS.map((s) => (
          <li key={s} className="onboarding__segment" data-done={STEPS.indexOf(s) <= STEPS.indexOf(step) || undefined} aria-current={s === step ? "step" : undefined}>
            <span className="onboarding__segment-name">{stepLabel(s).split(": ")[1]}</span>
          </li>
        ))}
      </ol>
      <p className="onboarding__step">{stepLabel(step).split(": ")[0]}</p>
      <h2 ref={heading} className="onboarding__title" tabIndex={-1}>{title}</h2>
      <div className="onboarding__body" role="region" aria-label="Step content" tabIndex={0}>
        {children}
      </div>
      <footer className="onboarding__foot">
        {canGoBack && (
          <button type="button" className="onboarding__back" onClick={onBack}>
            Back
          </button>
        )}
        {secondary && (
          <button type="button" className="onboarding__secondary" onClick={onSecondary}>
            {secondary}
          </button>
        )}
        <button type="button" className="onboarding__primary" data-last={!hasNext || undefined} disabled={primaryDisabled} onClick={onPrimary}>
          {primary}
        </button>
      </footer>
    </section>
  );
}
