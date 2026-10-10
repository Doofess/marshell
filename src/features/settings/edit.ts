import { unifiedDiff } from "../../lib/diff";
import type { SettingField } from "./fields";

/**
 * Phase 0 re-serialises the object, which keeps key order but not the file's own formatting. Phase 3 edits the file with the
 * core's CST (jsonc), so formatting and comments survive, and the page only ever shows the diff the core returns.
 */
export function applyChanges(beforeText: string, changes: Record<string, unknown>): string {
  return JSON.stringify({ ...(JSON.parse(beforeText) as Record<string, unknown>), ...changes }, null, 2) + "\n";
}

/** The exact change, as a unified diff of the whole file; empty when nothing changes. */
export function previewChange(path: string, beforeText: string, changes: Record<string, unknown>): string {
  return unifiedDiff(path, beforeText, applyChanges(beforeText, changes));
}

/** The edits whose value really differs from the field's current value. */
export function pendingChanges(fields: SettingField[], edits: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const f of fields) if (f.key in edits && JSON.stringify(edits[f.key]) !== JSON.stringify(f.value)) out[f.key] = edits[f.key];
  return out;
}

export const changeSummary = (n: number) => (n === 0 ? "No changes" : `${n} ${n === 1 ? "change" : "changes"}`);
export const fileChanged = (openedText: string, currentText: string) => openedText !== currentText;
