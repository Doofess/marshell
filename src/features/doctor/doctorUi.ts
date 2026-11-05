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
  return { text: afterFixMessage(last.count, last.restart, last.item_ids), kind: "action-result" };
}

/** What the announcer says when the core refuses an undo because files changed since the fix; the page then shows why. */
export function undoRefusedAnnouncement(paths: readonly string[]): { text: string; kind: AnnounceKind } {
  const files = paths.length === 0 ? "a file" : paths.length === 1 ? paths[0]! : `${paths.slice(0, -1).join(", ")} and ${paths[paths.length - 1]}`;
  return { text: `Nothing was undone: ${files} changed since the fix, so undo would overwrite ${paths.length > 1 ? "them" : "it"}.`, kind: "action-result" };
}
