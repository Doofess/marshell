export const STEPS = ["detected", "consent", "notifications", "first-session"] as const;
export type Step = (typeof STEPS)[number];

/** First run takes under a minute, from launch to a running session. */
export const ONBOARDING_BUDGET_MS = 60_000;

const NAMES: Record<Step, string> = { detected: "Detected", consent: "Consent", notifications: "Notifications", "first-session": "First session" };

export const stepNumber = (s: Step) => STEPS.indexOf(s) + 1;
export const nextStep = (s: Step): Step => STEPS[Math.min(STEPS.length - 1, STEPS.indexOf(s) + 1)]!;
export const prevStep = (s: Step): Step => STEPS[Math.max(0, STEPS.indexOf(s) - 1)]!;
export const stepLabel = (s: Step) => `Step ${stepNumber(s)} of ${STEPS.length}: ${NAMES[s]}`;
