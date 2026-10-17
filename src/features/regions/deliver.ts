import { hasPromptLine } from "../sidebar/promptLine";
import type { RowModel } from "../sidebar/types";

export type Via = "paste" | "clipboard";

/** Where a region goes for the focused row: pasted into an agent's prompt, or onto the clipboard for a plain terminal. */
export function deliveryFor(row: RowModel | null): { name: string; via: Via } | null {
  if (row === null) return null;
  return { name: row.name, via: hasPromptLine(row) ? "paste" : "clipboard" };
}

/** What goes on the clipboard: the attachment's reference, then the region's text. */
export const clipboardText = (reference: string, text: string): string => `${reference} ${text}`;

export const noticeFor = (via: Via, name: string): string =>
  via === "paste" ? `Added the region to ${name}.` : `${name} has no prompt line, so the region is on the clipboard.`;
