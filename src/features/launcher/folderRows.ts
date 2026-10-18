import type { FolderInfo } from "../../generated/FolderInfo";
import type { Hit, Project } from "./filter";

/**
 * What the core said about a typed path. `path` is the text that was checked, not the folder's own spelling. `failed` means
 * the check itself went wrong (not a 404), so nothing is known about the folder.
 */
export type PathCheck =
  | { path: string; state: "checking" }
  | { path: string; state: "found"; folder: FolderInfo }
  | { path: string; state: "missing" }
  | { path: string; state: "failed" };

/** One row of the folder list: the typed path, a recent folder, or Browse…. */
export type FolderRow = { kind: "path"; check: PathCheck } | { kind: "project"; hit: Hit<Project> } | { kind: "browse" };

// Either OS's syntax: a drive with a separator, a UNC or rooted path, or the home folder. The core decides which is a full path here.
const PATH_START = /^(?:[A-Za-z]:[\\/]|[\\/]|~)/;

/**
 * The typed text as a path to check, or null when it only filters. Surrounding space and one pair of pasted quotes are
 * dropped. `/x` passes on Windows too: the core answers that it isn't a full path there, and no row is shown.
 */
export function typedPath(query: string): string | null {
  let text = query.trim();
  const quote = text[0];
  if (text.length >= 2 && (quote === '"' || quote === "'") && text.endsWith(quote)) text = text.slice(1, -1).trim();
  return PATH_START.test(text) ? text : null;
}

/** The typed path first, so Enter takes it; then the matching folders; Browse… always last, so the picker is one row away. */
export function folderRows(check: PathCheck | null, hits: Hit<Project>[]): FolderRow[] {
  const rows: FolderRow[] = hits.map((hit) => ({ kind: "project", hit }));
  if (check) rows.unshift({ kind: "path", check });
  rows.push({ kind: "browse" });
  return rows;
}

/** What Enter (or a click) on a row does; null for a row that can't be taken, so nothing launches. */
export function folderEnter(row: FolderRow | undefined): { kind: "pick"; id: string } | { kind: "use"; folder: FolderInfo } | { kind: "browse" } | null {
  if (!row) return null;
  if (row.kind === "browse") return { kind: "browse" };
  if (row.kind === "project") return { kind: "pick", id: row.hit.item.id };
  return row.check.state === "found" ? { kind: "use", folder: row.check.folder } : null;
}

/** Only a found folder can be taken, not a path still being checked, one with no folder there, or one that couldn't be checked. */
export function rowEnabled(row: FolderRow): boolean {
  return row.kind !== "path" || row.check.state === "found";
}

/** Ctrl+O by the physical key, so it works on any keyboard layout (`e.key` is not "o" on a Hebrew or Russian one). */
export function browseKey(e: { code: string; ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean }): boolean {
  return e.code === "KeyO" && e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey;
}
