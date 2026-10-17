import type { Command } from "../../lib/keymap";

export type Project = { id: string; name: string; branch?: string; path: string; lastUsedMs: number };
/** Code-point indices into the text, end exclusive. */
export type Range = [start: number, end: number];
export type Hit<T> = { item: T; ranges: Range[] };
export type Segment = { text: string; hit: boolean };

const lower = (cp: string) => cp.toLocaleLowerCase();
const WORD_START = new Set([" ", "-", "_", "/", "\\", ".", "·"]);

/**
 * Case-insensitive subsequence match, with no regular expressions so no input can be read as a pattern.
 * Returns the matched ranges (merged when adjacent), [] for an empty query, or null for no match.
 */
export function matchRanges(query: string, text: string): Range[] | null {
  const q = Array.from(query.trim(), lower);
  if (q.length === 0) return [];
  const t = Array.from(text, lower);
  const ranges: Range[] = [];
  let qi = 0;
  for (let i = 0; i < t.length && qi < q.length; i++) {
    if (t[i] !== q[qi]) continue;
    const last = ranges[ranges.length - 1];
    if (last && last[1] === i) last[1] = i + 1;
    else ranges.push([i, i + 1]);
    qi++;
  }
  return qi === q.length ? ranges : null;
}

/** Longer runs and word-start matches score higher; the start of the text counts most; shorter text breaks near-ties. */
export function score(ranges: Range[], text: string): number {
  const t = Array.from(text);
  let s = 0;
  for (const [a, b] of ranges) {
    s += (b - a) * 2 + (b - a - 1) * 2;
    if (a === 0) s += 8;
    else if (WORD_START.has(t[a - 1] ?? "")) s += 6;
  }
  return s - t.length * 0.01;
}

function rank<T>(items: T[], query: string, label: (i: T) => string, recent: (i: T) => number): Hit<T>[] {
  const scored: Array<{ hit: Hit<T>; score: number; recent: number }> = [];
  for (const item of items) {
    const text = label(item);
    const ranges = matchRanges(query, text);
    if (ranges === null) continue;
    scored.push({ hit: { item, ranges }, score: ranges.length === 0 ? 0 : score(ranges, text), recent: recent(item) });
  }
  if (query.trim() === "") return scored.sort((a, b) => b.recent - a.recent).map((s) => s.hit);
  return scored.sort((a, b) => b.score - a.score || b.recent - a.recent).map((s) => s.hit);
}

export const projectLabel = (p: Project) => (p.branch ? `${p.name} · ${p.branch}` : p.name);

/** Most recently used first when the query is empty; best match first otherwise. */
export function filterProjects(query: string, projects: Project[]): Hit<Project>[] {
  return rank(projects, query, projectLabel, (p) => p.lastUsedMs);
}

/** Keeps the keymap's order for an empty query; best match first otherwise. */
export function filterCommands(query: string, commands: Command[]): Hit<Command>[] {
  if (query.trim() === "") return commands.map((item) => ({ item, ranges: [] }));
  return rank(commands, query, (c) => c.label, () => 0);
}

/** Splits text into hit and plain segments, by code point so emoji stay whole. */
export function highlight(text: string, ranges: Range[]): Segment[] {
  const cps = Array.from(text);
  const out: Segment[] = [];
  let at = 0;
  for (const [a, b] of ranges) {
    if (a > at) out.push({ text: cps.slice(at, a).join(""), hit: false });
    out.push({ text: cps.slice(a, b).join(""), hit: true });
    at = b;
  }
  if (at < cps.length || out.length === 0) out.push({ text: cps.slice(at).join(""), hit: false });
  return out;
}

/** Letter-to-value picking in a chip: the next option after `current` whose label starts with the key, wrapping. */
export function pickByLetter(labels: string[], key: string, current = -1): number {
  if (!/^\p{L}$/u.test(key)) return -1;
  const k = lower(key);
  for (let step = 1; step <= labels.length; step++) {
    const i = (current + step + labels.length) % labels.length;
    if (lower(Array.from(labels[i] ?? "")[0] ?? "") === k) return i;
  }
  return -1;
}

/** The part of a session the palette and the launcher need to find it: its stable number and where it runs. */
export type SessionRef = { id: string; number: number; name: string; project: string; branch?: string };
export type SessionHit<T extends SessionRef = SessionRef> = {
  item: T;
  /** Matched letters in the name. */
  ranges: Range[];
  /** Matched letters in `sessionDetail`, when the name itself did not match. */
  detailRanges: Range[];
  /** True for the session whose number the query spelled out (6, #6, session 6). */
  exact: boolean;
};

export const sessionDetail = (s: SessionRef) => (s.branch ? `${s.project} · ${s.branch}` : s.project);

const NUMBER_QUERY = /^(?:(?:go to )?session\s*)?#?(\d{1,3})$/i;

/**
 * The number a query asks for: "6", "#6", "session 6" and "go to session 6" all mean session 6. Plain ASCII digits only,
 * and never 0 (numbers start at 1). The pattern is fixed; the query is never used as one.
 */
export function parseSessionNumber(query: string): number | null {
  const m = NUMBER_QUERY.exec(query.trim());
  const n = m ? Number.parseInt(m[1]!, 10) : 0;
  return n > 0 ? n : null;
}

/**
 * Sessions for "Go to session": the lowest number first for an empty query; the session a number query names first,
 * then name and folder matches; otherwise best match first, equal matches by lower number. Matching is by name, then by
 * project and branch (a little lower).
 */
export function filterSessions<T extends SessionRef>(query: string, sessions: T[]): SessionHit<T>[] {
  const byNumber = [...sessions].sort((a, b) => a.number - b.number);
  if (query.trim() === "") return byNumber.map((item) => ({ item, ranges: [], detailRanges: [], exact: false }));

  const wanted = parseSessionNumber(query);
  const text = wanted === null ? query : String(wanted);
  const scored: Array<{ hit: SessionHit<T>; score: number }> = [];
  for (const item of byNumber) {
    const exact = wanted !== null && item.number === wanted;
    const name = matchRanges(text, item.name);
    const detail = name === null ? matchRanges(text, sessionDetail(item)) : null;
    if (!exact && name === null && detail === null) continue;
    const s = exact ? Infinity : name !== null ? score(name, item.name) : score(detail!, sessionDetail(item)) - 5;
    scored.push({ hit: { item, ranges: name ?? [], detailRanges: detail ?? [], exact }, score: s });
  }
  // Rounded so the tie-breaking length penalty never beats the lower number.
  return scored.sort((a, b) => (b.score === a.score ? 0 : Math.round(b.score) - Math.round(a.score)) || a.hit.item.number - b.hit.item.number).map((s) => s.hit);
}
