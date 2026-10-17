import type { Profile } from "../../generated/Profile";
import type { RowModel } from "../sidebar/types";

/**
 * Why Resume is off before the core is even asked, or undefined. The profile list can be empty because the fetch failed
 * (useSessions falls back to an empty list), and "no profiles" must not read as "this one was removed": an empty list is unknown,
 * and the core's own refusal answers instead.
 */
export function resumeDisabledReason(profileId: string, profiles: Profile[]): string | undefined {
  if (profiles.length === 0 || profiles.some((p) => p.id === profileId)) return undefined;
  return "Its shell profile was removed.";
}

/** The core's id for a session started outside any saved shell profile; it never has a profile to lose. */
export const CUSTOM_PROFILE = "custom";

/** Why Resume is off for this session before the core is asked, or undefined: the Resume bar disables it on this, and the row menu leaves it out. */
export const resumeBlockedReason = (profileId: string, profiles: Profile[]): string | undefined => (profileId === CUSTOM_PROFILE ? undefined : resumeDisabledReason(profileId, profiles));

/** Where Resume's request stands: out, answered ok, or refused (a 409, or a request that failed). */
export type ResumeResponse = "pending" | "ok" | "refused";

/** The state of a row as far as Resume cares: an Ended or Exited tab is its phrase ("Exited with code 3"); anything else is live. */
export const resumeStateOf = (row: RowModel): string => (row.status === "ended" ? row.phrase : "live");

/**
 * Whether the tab still shows the row it had when Resume was pressed (the strip, and no new terminal) instead of the live row.
 * - While the request is out: yes. The core says Running before the session exists.
 * - Refused: no, at once.
 * - Answered ok: only until a session snapshot shows the tab's state moved off the one it was pressed in. The answer can come
 *   before the Running frame, and a second press in that gap must find the strip still held.
 */
export function holdsResume(pressed: string, live: string, response: ResumeResponse): boolean {
  if (response === "refused") return false;
  if (response === "pending") return true;
  return live === pressed;
}
