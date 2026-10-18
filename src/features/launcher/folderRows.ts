import type { FolderInfo } from "../../generated/FolderInfo";
import type { Hit, Project } from "./filter";

/** What the core said about a typed path. `path` is the text that was checked, not the folder's own spelling. */
export type PathCheck = { path: string; state: "checking" } | { path: string; state: "found"; folder: FolderInfo } | { path: string; state: "missing" };

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

/** Only a path still being checked, or one with no folder there, can't be taken. */
export function rowEnabled(row: FolderRow): boolean {
  return row.kind !== "path" || row.check.state === "found";
}
