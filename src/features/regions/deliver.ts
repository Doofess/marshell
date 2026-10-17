import { ApiError } from "../../lib/api/client";
import { hasPromptLine } from "../sidebar/promptLine";
import type { RowModel } from "../sidebar/types";

export type Via = "paste" | "clipboard";

/** Where a region goes, or why it can go nowhere (a paused or an ended session): nothing is saved for a refusal. */
export type Delivery = { name: string; via: Via } | { name: string; refused: string };

/** Where a region goes for the focused row: pasted into an agent's prompt, or onto the clipboard for a plain terminal. */
export function deliveryFor(row: RowModel | null): Delivery | null {
  if (row === null) return null;
  if (row.status === "paused") return { name: row.name, refused: `${row.name} is paused.` };
  if (row.status === "ended") return { name: row.name, refused: `${row.name} has ended.` };
  return { name: row.name, via: hasPromptLine(row) ? "paste" : "clipboard" };
}

/** The words for a failed Add: the core's `{reason}` body when it has one, plain text as it came, and a plain reason for a 404 or an empty body. */
export function addFailure(e: unknown): string {
  if (!(e instanceof ApiError)) return e instanceof Error && e.message !== "" ? e.message : "Something went wrong.";
  try {
    const body = JSON.parse(e.message) as { reason?: unknown };
    if (typeof body.reason === "string" && body.reason !== "") return body.reason;
  } catch {
    // Not a JSON body: the text itself.
  }
  if (e.message.trim() !== "") return e.message;
  return e.status === 404 ? "The session has ended." : `The core refused it (${e.status}).`;
}

/** What goes on the clipboard: the attachment's reference, then the region's text. */
export const clipboardText = (reference: string, text: string): string => `${reference} ${text}`;

export const noticeFor = (via: Via, name: string): string =>
  via === "paste" ? `Added the region to ${name}.` : `${name} has no prompt line, so the region is on the clipboard.`;
