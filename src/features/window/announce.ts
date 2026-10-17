import { shouldAnnounce, type AnnounceKind, type AnnounceMode } from "../../components/a11y/announce-policy";
import { GLYPHS } from "../../components/StatusGlyph/glyphs";
import type { RowModel } from "../sidebar/types";

/** Which kind of event a status is, for the policy ("only what needs you"). Progress and "working" are never announced. */
export function kindOf(status: RowModel["status"]): AnnounceKind {
  switch (status) {
    case "needs-permission":
      return "request";
    case "needs-question":
      return "question";
    case "error":
      return "stopped";
    case "ended":
      return "ended";
    case "done-unseen":
    case "done-seen":
      return "done";
    default:
      return "progress";
  }
}

/**
 * What a screen reader should hear about a session, in the words the prompt line shows. A session that is working, idle or
 * done has nothing to say: only the states that need the user, or stop the session, are worth an interruption.
 */
export function sessionAnnouncement(row: RowModel | null): string {
  if (!row) return "";
  if (row.status === "needs-permission") return `${row.name} is waiting for your approval`;
  if (row.status === "needs-question") return `${row.name} has a question`;
  if (row.status === "error") return `${row.name} has stopped`;
  if (row.status === "ended") return `${row.name} has ended`;
  return "";
}

/** Focus moved to this session (or to none): its number and name, then what it needs if it needs anything. */
export function switchAnnouncement(row: RowModel | null): string {
  if (!row) return "No session selected";
  const needs = sessionAnnouncement(row);
  return needs ? `Session ${row.number}, ${needs}` : `Session ${row.number}, ${row.name}`;
}

/** "Read the focused session's status": the state and the phrase the row shows. */
export function statusSentence(row: RowModel | null): string {
  if (!row) return "No session selected";
  return `Session ${row.number}, ${row.name}: ${GLYPHS[row.status].label}. ${row.phrase}`;
}

/** A request can arrive in a session that is not the focused one; when the count of sessions that need you rises, say so. */
export function laneAnnouncement(prev: number | null, count: number): string {
  if (prev === null || count <= prev) return "";
  return count === 1 ? "1 session needs you" : `${count} sessions need you`;
}

export type Announced = { key: string; text: string };

/**
 * The window has ONE live region, for the focused session. It announces a change (focus moved, or the focused session
 * started waiting, stopped or ended), never the first render, never progress, and never the same thing twice: an unchanged
 * session returns the previous value itself.
 */
/** A notice for the window's announcer. `navigation` marks the cursor's receipts ("Selected session 4, api"). */
export type Notice = { id: number; text: string; navigation?: boolean };

/** True when a navigation receipt not yet heard arrives with a switch: the receipt already says where focus went. */
export function coveredByNotice(notice: Notice | null | undefined, heardId: number | undefined): boolean {
  return notice?.navigation === true && notice.id !== heardId;
}

export function nextAnnouncement(prev: Announced | null, row: RowModel | null, mode: AnnounceMode = "needs-you"): Announced {
  const state = sessionAnnouncement(row);
  const key = `${row?.id ?? "none"}|${state}`;
  if (prev === null) return { key, text: "" };
  if (prev.key === key) return prev;
  const switched = prev.key.split("|")[0] !== (row?.id ?? "none");
  const kind: AnnounceKind = switched ? (row ? "switched" : "none-selected") : kindOf(row?.status ?? "idle");
  if (!shouldAnnounce(kind, mode)) return { key, text: "" };
  return { key, text: switched ? switchAnnouncement(row) : state };
}
