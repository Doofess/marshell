import type { SessionShortcut } from "../../lib/appShortcuts";
import type { Os } from "../../lib/keymap";

/**
 * The window shortcuts that still act while the launcher is open, which otherwise holds every key (slice 6k decision 8). On an
 * empty window the launcher opens by itself, so Reopen closed session must get past it. It passes only when a closed session is
 * there to reopen, so the launcher never closes onto an empty window, and never with AltGr held: AltGr is Ctrl+Alt on Windows, and
 * AltGr+Shift+T types a character on some layouts.
 */
export const passesLauncher = (kind: SessionShortcut["kind"], hasClosed: boolean, altGraph: boolean): boolean =>
  kind === "reopen" && hasClosed && !altGraph;

/** Whether the first-run onboarding is not known yet (the saved settings haven't answered), shown, or seen and closed. */
export type OnboardingShown = "unknown" | "showing" | "done";

/**
 * Whether the window opens the launcher by itself: only when the core has sent its list and the list is empty (a first run, or
 * everything closed). A core that restarted seeds the saved tabs as Ended, so a restart with tabs to show leaves it closed. It
 * waits for the onboarding, which would otherwise sit under the launcher while the launcher holds the focus, so the Enter meant for
 * the onboarding would start a session; it opens when the onboarding ends.
 */
export function launcherOpens(loaded: boolean, rowCount: number, onboarding: OnboardingShown, reopening = false): boolean {
  return loaded && rowCount === 0 && onboarding === "done" && !reopening;
}

/** What a palette command does to the launcher: "Start a session" opens it as usual, "New session as administrator" opens it with the chip on. Null for any other command. */
export function launcherFromPalette(id: string): { elevated: boolean } | null {
  if (id === "launcher") return { elevated: false };
  return id === "new-session-admin" ? { elevated: true } : null;
}

/** "New session as administrator" is listed on Windows only (decision 4). */
export const adminCommandHidden = (os: Os | undefined): boolean => os !== "windows";
