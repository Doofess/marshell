import { parseUnifiedDiff } from "../../components/DiffView/diff";
import type { ChangesReply } from "../../generated/ChangesReply";
import type { DiffNote } from "../../generated/DiffNote";
import type { DiffStatus } from "../../generated/DiffStatus";
import type { DraftComment } from "../../generated/DraftComment";
import { splitDiff } from "../tasks/splitDiff";
import { hashesOf, renumbered, type Placement } from "./comments";
import { copy } from "./copy";

/** A file with more rows than this starts folded. */
export const FOLD_LINES = 400;
/** Once this many rows are open, the files below start folded. */
export const OPEN_BUDGET = 10_000;

export type Row = { kind: "hunk" | "context" | "add" | "del"; text: string; oldNo?: number; newNo?: number };
export type Section = {
  path: string;
  oldPath?: string;
  status: DiffStatus;
  added: number;
  removed: number;
  note?: DiffNote | "whitespace" | "gone";
  rows: Row[];
  folded: boolean;
  foldReason?: "big" | "lockfile" | "minified" | "budget";
};
export type Item =
  | { kind: "file"; key: string; file: Section }
  | { kind: "show"; key: string; file: Section }
  | { kind: "row"; key: string; file: Section; at: number }
  | { kind: "comment"; key: string; comment: DraftComment; placed: Placement }
  | { kind: "note"; key: string; file: Section; text: string };

const LOCKFILES = new Set([
  "pnpm-lock.yaml",
  "package-lock.json",
  "npm-shrinkwrap.json",
  "yarn.lock",
  "bun.lock",
  "bun.lockb",
  "Cargo.lock",
  "Gemfile.lock",
  "composer.lock",
  "poetry.lock",
  "uv.lock",
  "Pipfile.lock",
  "go.sum",
  "flake.lock",
  "packages.lock.json",
  "pubspec.lock",
  "mix.lock",
  "Podfile.lock",
]);
const MINIFIED_NAME = /\.min\.(js|mjs|cjs|css)$/i;
const MINIFIED_ROW = 1000;

/** A lockfile by its file name, in any folder. */
export function isLockfile(path: string): boolean {
  return LOCKFILES.has(path.slice(path.lastIndexOf("/") + 1));
}

/** A minified file: a `.min.js` style name, or a row longer than 1,000 characters. */
export function isMinified(path: string, rows: Row[]): boolean {
  return MINIFIED_NAME.test(path) || rows.some((r) => r.text.length > MINIFIED_ROW);
}

/**
 * The rows of one file's diff section. They start at the first `@@` line: everything before it is header (mode lines, renames,
 * `Binary files … differ`), which `parseUnifiedDiff` would show as context. `\ No newline at end of file` lines are dropped before
 * parsing, so they count for neither side.
 */
function rowsOf(diff: string): Row[] {
  const lines = diff.split("\n");
  const from = lines.findIndex((l) => l.startsWith("@@"));
  if (from < 0) return [];
  const body = lines
    .slice(from)
    .filter((l) => !l.startsWith("\\"))
    .join("\n");
  const rows: Row[] = [];
  for (const l of parseUnifiedDiff(body).lines) {
    if (l.kind === "meta") continue;
    const row: Row = { kind: l.kind, text: l.text };
    if (l.oldNo !== undefined) row.oldNo = l.oldNo;
    if (l.newNo !== undefined) row.newNo = l.newNo;
    rows.push(row);
  }
  return rows;
}

function foldReasonOf(path: string, rows: Row[], openRows: number): Section["foldReason"] {
  if (rows.length > FOLD_LINES) return "big";
  if (isLockfile(path)) return "lockfile";
  if (isMinified(path, rows)) return "minified";
  if (openRows >= OPEN_BUDGET) return "budget";
  return undefined;
}

/**
 * One section per file of the reply, in its order, then a "gone" section (no rows) for each draft's path that has no file any more.
 * A file starts folded by the rules above (big, lockfile, minified, or too many open rows above it); `folds[path]` overrides.
 * A section with no rows is never folded: there is nothing to hide.
 */
/** Each file's part of the reply's diff, by path; the first part wins. */
function partsOf(reply: ChangesReply): Map<string, string> {
  const diffs = new Map<string, string>();
  for (const part of splitDiff(reply.diff)) if (!diffs.has(part.path)) diffs.set(part.path, part.diff);
  return diffs;
}

/**
 * The files of a cut diff that lie past the cut: the ones after the last file the diff holds or a note covers. The diff comes in the
 * files' order, so a file before that one with no part is one whose changes are only whitespace.
 */
function pastCut(reply: ChangesReply, diffs: Map<string, string>): Set<string> {
  if (!reply.truncated) return new Set();
  let last = -1;
  reply.files.forEach((f, i) => {
    if (diffs.has(f.path) || f.note !== null) last = i;
  });
  return new Set(reply.files.slice(last + 1).map((f) => f.path));
}

/** How many files a cut diff leaves out, for "The diff is cut at 4 MiB. {n} more files aren't shown."; 0 when it isn't cut. */
export function cutFiles(reply: ChangesReply): number {
  return pastCut(reply, partsOf(reply)).size;
}

/** `openPath` is the open comment box's file: like a comment's, it stays as a "gone" section when it leaves the diff. */
export function sectionsOf(reply: ChangesReply, folds: Record<string, boolean>, drafts: DraftComment[], hideWhitespace: boolean, openPath?: string): Section[] {
  const diffs = partsOf(reply);
  const cut = pastCut(reply, diffs);

  let openRows = 0;
  const sections: Section[] = reply.files.filter((f) => !cut.has(f.path)).map((f) => {
    const rows = rowsOf(diffs.get(f.path) ?? "");
    const section: Section = { path: f.path, status: f.status, added: 0, removed: 0, rows, folded: false };
    if (f.old_path !== null) section.oldPath = f.old_path;
    for (const r of rows) {
      if (r.kind === "add") section.added++;
      else if (r.kind === "del") section.removed++;
    }
    if (f.note !== null) section.note = f.note;
    else if (rows.length === 0 && hideWhitespace) section.note = "whitespace";
    if (rows.length > 0) {
      const reason = foldReasonOf(f.path, rows, openRows);
      const override = Object.hasOwn(folds, f.path) ? folds[f.path] : undefined;
      section.folded = override ?? reason !== undefined;
      if (section.folded && reason !== undefined) section.foldReason = reason;
      if (!section.folded) openRows += rows.length;
    }
    return section;
  });

  const known = new Set(reply.files.map((f) => f.path));
  const kept = openPath === undefined ? drafts.map((d) => d.path) : [...drafts.map((d) => d.path), openPath];
  const gone = [...new Set(kept.filter((p) => !known.has(p)))].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  for (const path of gone) sections.push({ path, status: "modified", added: 0, removed: 0, note: "gone", rows: [], folded: false });
  return sections;
}

const noteText = (note: NonNullable<Section["note"]>) => copy.notes[note];

/**
 * The panel as one flat list, for the keyboard and the virtual list: each file's header, then its "show" row when folded, else its note,
 * its comments that have no row (above the first) and its rows with each comment right under the row it is placed at.
 * A draft with no placement is shown above the first row, as changed.
 */
export function itemsOf(sections: Section[], drafts: DraftComment[], placements: Map<string, Placement>): Item[] {
  const items: Item[] = [];
  for (const file of sections) {
    items.push({ kind: "file", key: `file:${file.path}`, file });
    if (file.folded) {
      items.push({ kind: "show", key: `show:${file.path}`, file });
      continue;
    }
    if (file.note !== undefined) items.push({ kind: "note", key: `note:${file.path}`, file, text: noteText(file.note) });
    const placed = drafts
      .filter((d) => d.path === file.path)
      .map((comment, order) => ({ comment, order, placed: placements.get(comment.id) ?? { at: null, changed: true } }))
      .sort((a, b) => a.comment.start - b.comment.start || a.order - b.order);
    const commentItem = (p: (typeof placed)[number]): Item => ({ kind: "comment", key: `comment:${p.comment.id}`, comment: p.comment, placed: p.placed });
    for (const p of placed) if (p.placed.at === null) items.push(commentItem(p));
    // A row's key is its kind and text, and how many rows above it in the file have the same: rows that go in above it
    // never change it, so the keyboard, a range and a menu stay on the line they were on.
    const hashes = hashesOf(file.rows);
    const seen = new Map<string, number>();
    file.rows.forEach((_, at) => {
      const hash = hashes[at]!;
      const n = seen.get(hash) ?? 0;
      seen.set(hash, n + 1);
      items.push({ kind: "row", key: `row:${file.path}:${hash}:${n}`, file, at });
      for (const p of placed) if (p.placed.at === at) items.push(commentItem(p));
    });
  }
  return items;
}

/** The index the keyboard is on: the item with its key, else the index it was on before (its line changed or went), kept in the list. */
export function focusIndex(items: Item[], key: string | null, last: number): number {
  const found = key === null ? -1 : items.findIndex((i) => i.key === key);
  if (found >= 0) return found;
  return items.length === 0 ? 0 : Math.max(0, Math.min(last, items.length - 1));
}

function statusText(file: Section): string {
  switch (file.status) {
    case "modified":
      return "modified";
    case "added":
    case "untracked":
      return "new";
    case "deleted":
      return "deleted";
    case "renamed":
      return file.oldPath === undefined ? "renamed" : `renamed from ${file.oldPath}`;
    default: {
      const unreachable: never = file.status;
      return unreachable;
    }
  }
}

function rowLabel(row: Row | undefined): string {
  if (row === undefined) return "";
  const text = row.text === "" ? "blank line" : row.text;
  switch (row.kind) {
    case "hunk": {
      const start = /\+(\d+)/.exec(row.text)?.[1];
      return start === undefined ? "Changed block" : `Changed block at line ${start}`;
    }
    case "add":
      return `Added, line ${row.newNo}: ${text}`;
    case "del":
      return `Removed, line ${row.oldNo}: ${text}`;
    case "context":
      return `Line ${row.newNo ?? row.oldNo}: ${text}`;
    default: {
      const unreachable: never = row.kind;
      return unreachable;
    }
  }
}

/** What a screen reader says for an item. */
export function itemLabel(item: Item): string {
  switch (item.kind) {
    case "row":
      return rowLabel(item.file.rows[item.at]);
    case "file":
      return `${item.file.path}, ${statusText(item.file)}, ${item.file.added} added, ${item.file.removed} removed`;
    case "show": {
      const n = item.file.rows.length;
      return `Show ${n.toLocaleString("en-US")} ${n === 1 ? "line" : "lines"}`;
    }
    case "comment": {
      const c = renumbered(item.comment, item.placed);
      const range = c.start !== c.end;
      const where = range ? `lines ${c.start} to ${c.end}` : `line ${c.start}`;
      return `Comment on ${where}: ${c.text}${item.placed.changed ? `. ${copy.changed(range)}` : ""}`;
    }
    case "note":
      return item.text;
    default: {
      const unreachable: never = item;
      return unreachable;
    }
  }
}
