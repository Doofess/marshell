import type { AnnounceKind } from "../../components/a11y/announce-policy";
import { afterFixMessage } from "./report";
import type { DoctorLastFix } from "./types";
import type { DoctorSummary } from "../../generated/DoctorSummary";

/** The count the sidebar and the tray show: the last check's, and none before a check has ended (no guessed 0). */
export const shownIssues = (s: DoctorSummary | null): number | undefined => (s && s.checked_at_ms != null ? s.issues : undefined);

/** Nothing is drawn for the first 150 ms of a check: a quick one never flashes a progress line. */
export const showProgress = (elapsedMs: number) => elapsedMs > 150;

/** What the window's announcer says when a fix finishes: the receipt text, as the result of something the user did. */
export function fixAnnouncement(last: DoctorLastFix): { text: string; kind: AnnounceKind } {
  return { text: afterFixMessage(last.count, last.restart), kind: "action-result" };
}
