import type { AnnounceKind } from "../../components/a11y/announce-policy";
import { afterFixMessage } from "./report";
import type { UndoState } from "./undo";

/** Nothing is drawn for the first 150 ms of a check: a quick one never flashes a progress line. */
export const showProgress = (elapsedMs: number) => elapsedMs > 150;

/** What the window's announcer says when a fix finishes: the receipt text, as the result of something the user did. */
export function fixAnnouncement(undo: UndoState, restart: number[]): { text: string; kind: AnnounceKind } {
  return { text: afterFixMessage(undo.count, restart), kind: "action-result" };
}
