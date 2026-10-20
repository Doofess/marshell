/** Whether the first-run onboarding is not known yet (the saved settings haven't answered), shown, or seen and closed. */
export type OnboardingShown = "unknown" | "showing" | "done";

/**
 * Whether the window opens the launcher by itself: only when the core has sent its list and the list is empty (a first run, or
 * everything closed). A core that restarted seeds the saved tabs as Ended, so a restart with tabs to show leaves it closed. It
 * waits for the onboarding, which would otherwise sit under the launcher while the launcher holds the focus, so the Enter meant for
 * the onboarding would start a session; it opens when the onboarding ends.
 */
export function launcherOpens(loaded: boolean, rowCount: number, onboarding: OnboardingShown): boolean {
  return loaded && rowCount === 0 && onboarding === "done";
}
