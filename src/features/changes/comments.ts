import type { DraftComment } from "../../generated/DraftComment";
import type { Row, Section } from "./model";

export const MAX_COMMENTS = 50;
export const MAX_COMMENT_CHARS = 2000;
export const QUOTE_LINES = 3;
export const QUOTE_CHARS = 200;

// Control characters are C0 (0x00-0x1F), DEL and C1 (0x80-0x9F). Not tab and line feed:
const CONTROL_NOT_TAB_LF = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/g;
// Not tab:
const CONTROL_NOT_TAB = /[\u0000-\u0008\u000a-\u001f\u007f-\u009f]/g;

const cut = (text: string, max: number) => {
  const points = Array.from(text);
  return points.length <= max ? text : points.slice(0, max).join("");
};

/** What a comment may hold: control characters other than \n and \t removed, \r\n and \r turned into \n, cut at MAX_COMMENT_CHARS code points. CommentBox runs it on every change. */
export function cleanComment(text: string): string {
  return cut(text.replace(/\r\n?/g, "\n").replace(CONTROL_NOT_TAB_LF, ""), MAX_COMMENT_CHARS);
}

/** A quoted line: control characters other than \t (C0, DEL and C1) become U+FFFD, cut at QUOTE_CHARS code points, never inside a surrogate pair. */
export function quoteLine(text: string): string {
  return cut(text.replace(CONTROL_NOT_TAB, "�"), QUOTE_CHARS);
}

/**
 * Where a comment sits now: the index of the row it goes under (null: above the first row), and whether the lines under it changed.
 * When its rows were found unchanged, `start` and `end` are their line numbers now (lines above may have moved them).
 */
export type Placement = { at: number | null; changed: boolean; start?: number; end?: number };

/** The comment (or box) with the line numbers its rows have now, when `place` found them; else as it was written. */
export function renumbered<T extends { start: number; end: number }>(c: T, placed: Placement | undefined): T {
  return placed?.start === undefined || placed.end === undefined ? c : { ...c, start: placed.start, end: placed.end };
}

/** The 53-bit cyrb53 hash (seed 0) as 14 lowercase hex characters. Not a security hash: it only recognises a line again. */
export function cyrb53(s: string): string {
  let h1 = 0xdeadbeef;
  let h2 = 0x41c6ce57;
  for (let i = 0; i < s.length; i++) {
    const ch = s.charCodeAt(i);
    h1 = Math.imul(h1 ^ ch, 2654435761);
    h2 = Math.imul(h2 ^ ch, 1597334677);
  }
  h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507);
  h1 ^= Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507);
  h2 ^= Math.imul(h1 ^ (h1 >>> 13), 3266489909);
  return (4294967296 * (2097151 & h2) + (h1 >>> 0)).toString(16).padStart(14, "0");
}

const keyOf = (r: Row) => `${r.kind}\u0000${r.text}`;
const SIGNS: Record<Row["kind"], string> = { hunk: "", context: " ", add: "+", del: "-" };

/**
 * The anchor of the rows `from` to `to` (either way round) of a file's section, which must lie in one hunk and hold no hunk row.
 * The caller adds the id and the text.
 */
export function anchorOf(file: Section, from: number, to: number): Omit<DraftComment, "id" | "text"> {
  const rows = file.rows.slice(Math.min(from, to), Math.max(from, to) + 1);
  const side = rows.some((r) => r.newNo !== undefined) ? "new" : "old";
  const numbers = rows.flatMap((r) => {
    const no = side === "new" ? r.newNo : r.oldNo;
    return no === undefined ? [] : [no];
  });
  const signed = new Set(rows.map((r) => r.kind)).size > 1;
  return {
    path: file.path,
    side,
    start: Math.min(...numbers),
    end: Math.max(...numbers),
    rows: rows.length,
    first: cyrb53(keyOf(rows[0]!)),
    hash: cyrb53(rows.map(keyOf).join("\n")),
    quote: rows.slice(0, QUOTE_LINES).map((r) => quoteLine((signed ? SIGNS[r.kind] : "") + r.text)),
  };
}

const keyHashes = new WeakMap<Row[], string[]>();
/** Each row's cyrb53 of its kind and text, kept per rows array. */
export const hashesOf = (rows: Row[]) => {
  let h = keyHashes.get(rows);
  if (!h) keyHashes.set(rows, (h = rows.map((r) => cyrb53(keyOf(r)))));
  return h;
};

/** Finds a comment's rows again in the file's current section. */
export function place(c: DraftComment, file: Section | undefined): Placement {
  const rows = file?.rows ?? [];
  if (rows.length === 0) return { at: null, changed: true };
  const no = (r: Row) => (c.side === "new" ? r.newNo : r.oldNo);

  // 1. The same rows, wherever they moved to: the window of c.rows rows (no hunk row) that begins like the comment and hashes the same.
  const firsts = hashesOf(rows);
  let best: Placement | null = null;
  let bestDistance = Infinity;
  for (let i = 0; i + c.rows <= rows.length; i++) {
    if (firsts[i] !== c.first) continue;
    const window = rows.slice(i, i + c.rows);
    if (window.some((r) => r.kind === "hunk")) continue;
    if (cyrb53(window.map(keyOf).join("\n")) !== c.hash) continue;
    const numbers = window.map(no).filter((v): v is number => v !== undefined);
    const distance = numbers.length === 0 ? Infinity : Math.abs(numbers[0]! - c.start);
    if (best === null || distance < bestDistance) {
      best = { at: i + c.rows - 1, changed: false, ...(numbers.length > 0 ? { start: Math.min(...numbers), end: Math.max(...numbers) } : {}) };
      bestDistance = distance;
    }
  }
  if (best !== null) return best;

  // 2. The text changed: the row numbered c.end on its side, or the nearest smaller one.
  let at: number | null = null;
  let atNo = -Infinity;
  for (let i = 0; i < rows.length; i++) {
    const v = no(rows[i]!);
    if (v !== undefined && v <= c.end && v > atNo) {
      at = i;
      atNo = v;
    }
  }
  return { at, changed: true };
}

/** "src/a.ts:42", "src/a.ts:10-14", "src/old.ts:7 (removed line)", "src/old.ts:7-9 (removed lines)", plus " (changed since)". */
export function citation(c: DraftComment, changed: boolean): string {
  const range = c.start === c.end;
  const lines = range ? `${c.start}` : `${c.start}-${c.end}`;
  const removed = c.side === "old" ? (range ? " (removed line)" : " (removed lines)") : "";
  return `${c.path}:${lines}${removed}${changed ? " (changed since)" : ""}`;
}

/** The text handed to the agent. `folder` names the checkout when it isn't the one the session shows now. */
export function messageOf(drafts: DraftComment[], placements: Map<string, Placement>, sections: Section[], base: string | null, folder?: string): string {
  const order = new Map(sections.map((s, i) => [s.path, i]));
  const rank = (c: DraftComment) => order.get(c.path) ?? sections.length;
  // Cited at the lines the rows are on now: the agent reads the file as it is.
  const now = drafts.map((c) => renumbered(c, placements.get(c.id)));
  const sorted = now.sort((a, b) => rank(a) - rank(b) || a.start - b.start);
  const where = folder === undefined ? "" : ` in ${folder}`;
  const header = base === null ? `Comments on the uncommitted changes${where}:` : `Comments on the changes${where} (compared with ${base}):`;
  const blocks = sorted.map((c) => {
    const lines = [citation(c, placements.get(c.id)?.changed ?? false), ...c.quote.map((q) => `> ${q}`)];
    if (c.rows > QUOTE_LINES) lines.push("> …");
    lines.push(c.text);
    return lines.join("\n");
  });
  return [header, ...blocks].join("\n\n").replace(/\n+$/, "");
}
