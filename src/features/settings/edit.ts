import { unifiedDiff } from "../../lib/diff";
import type { SettingField } from "./fields";
import { parseRaw } from "./raw";

/**
 * Phase 0 re-serialises the object, which keeps key order but not the file's own formatting. Phase 3 edits the file with the
 * core's CST (jsonc), so formatting and comments survive, and the page only ever shows the diff the core returns.
 * A file that does not parse is never edited, so there is no "after" for it: the page says why instead of showing a diff.
 */
export function applyChanges(beforeText: string, changes: Record<string, unknown>): string | undefined {
  const parsed = parseRaw(beforeText);
  if (!parsed.ok) return undefined;
  return JSON.stringify({ ...parsed.value, ...changes }, null, 2) + "\n";
}

/** The exact change, as a unified diff of the whole file; empty when nothing changes, and empty when the file does not parse. */
export function previewChange(path: string, beforeText: string, changes: Record<string, unknown>): string {
  const after = applyChanges(beforeText, changes);
  return after === undefined ? "" : unifiedDiff(path, beforeText, after);
}

/** The edits whose value really differs from the field's current value. */
export function pendingChanges(fields: SettingField[], edits: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const f of fields) if (f.key in edits && JSON.stringify(edits[f.key]) !== JSON.stringify(f.value)) out[f.key] = edits[f.key];
  return out;
}

export const changeSummary = (n: number) => (n === 0 ? "No changes" : `${n} ${n === 1 ? "change" : "changes"}`);
export const fileChanged = (openedText: string, currentText: string) => openedText !== currentText;
