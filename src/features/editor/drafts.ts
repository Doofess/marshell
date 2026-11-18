import type { DraftInfo } from "../../generated/DraftInfo";
import type { Os } from "../../lib/keymap";

/** A draft with no tab is dropped this long after its last edit (the plan's "30 days"). */
export const DRAFT_KEEP_MS = 30 * 24 * 60 * 60 * 1000;

/** Two paths name one file: on Windows case and the kind of slash do not matter; elsewhere the paths compare exactly. */
export function samePath(a: string, b: string, os: Os): boolean {
  if (a === b) return true;
  if (os !== "windows") return false;
  const norm = (p: string) => p.replace(/\\/g, "/").toLowerCase();
  return norm(a) === norm(b);
}

/** The paths of the drafts to drop: no tab's current entry shows the file, and the last edit is over 30 days old. */
export function staleDrafts(drafts: DraftInfo[], tabPaths: string[], now: number, os: Os = "windows"): string[] {
  return drafts.filter((d) => now - d.edited_ms > DRAFT_KEEP_MS && !tabPaths.some((p) => samePath(p, d.path, os))).map((d) => d.path);
}

/** Whether the set holds this path, whatever its spelling. */
export function isUnsaved(unsaved: ReadonlySet<string>, path: string, os: Os): boolean {
  for (const p of unsaved) if (samePath(p, path, os)) return true;
  return false;
}

/** The set with the path added or removed. The same set comes back when nothing changes. */
export function withUnsaved(unsaved: ReadonlySet<string>, path: string, on: boolean, os: Os): ReadonlySet<string> {
  if (on === isUnsaved(unsaved, path, os)) return unsaved;
  const next = new Set<string>();
  for (const p of unsaved) if (!samePath(p, path, os)) next.add(p);
  if (on) next.add(path);
  return next;
}
