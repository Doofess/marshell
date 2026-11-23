import type { ChangeMark } from "../../generated/ChangeMark";
import type { DiffStatus } from "../../generated/DiffStatus";
import type { TreeRow } from "./treeModel";

/** The letter at the end of a changed file's row. */
export const MARK_LETTER: Record<DiffStatus, string> = { modified: "M", added: "A", untracked: "A", renamed: "R", deleted: "D" };
/** What a screen reader adds to the row's label. */
export const MARK_WORD: Record<DiffStatus, string> = { modified: ", modified", added: ", new", untracked: ", new", renamed: ", renamed", deleted: ", deleted" };

/** The changed files by path, and every folder that holds one (all the ancestors of a change). */
export function marksOf(changes: readonly ChangeMark[]): { files: Map<string, DiffStatus>; folders: Set<string> } {
  const files = new Map<string, DiffStatus>();
  const folders = new Set<string>();
  for (const c of changes) {
    files.set(c.path, c.status);
    for (let i = c.path.indexOf("/"); i !== -1; i = c.path.indexOf("/", i + 1)) folders.add(c.path.slice(0, i));
  }
  return { files, folders };
}

/** The rows with `change` on the changed files and `holdsChanges` on the folders above them. */
export function withMarks(rows: TreeRow[], changes: readonly ChangeMark[]): TreeRow[] {
  if (changes.length === 0) return rows;
  const { files, folders } = marksOf(changes);
  return rows.map((r) => {
    if (r.kind === "folder") return folders.has(r.path) ? { ...r, holdsChanges: true } : r;
    if (r.kind === "file" || r.kind === "link") {
      const change = files.get(r.path);
      return change ? { ...r, change } : r;
    }
    return r;
  });
}
