/** What the core accepts per candidate (`files.rs`): longer ones are refused, so they are not offered. */
const MAX_CANDIDATE = 1024;

export type PathCandidate = {
  /** The path alone, as the core resolves it. */
  text: string;
  /** UTF-16 offsets into the searched text; `end` is exclusive and covers the path plus its `:line:col` or `(line,col)` suffix. */
  start: number;
  end: number;
  line?: number;
  col?: number;
};

const LEADING = new Set(["(", '"', "'", "`", "[", "<", "{"]);
// A `)` is handled apart: it closes a `(line,col)` suffix as well as a wrapper.
const TRAILING = new Set([".", ",", ";", ":", "'", '"', "`", "]", "}", ">"]);
const PAREN_SUFFIX = /\((\d+)(?:,(\d+))?\)$/;
const COLON_SUFFIX = /:(\d+)(?::(\d+))?$/;
// Not part of a file name in output: whitespace, quotes, and the characters that end a list or an expression.
const NOT_IN_PATH = /[\s"'`<>|*?,;]/;
const WINDOWS_ABS = /^[A-Za-z]:[\\/][^\\/]/;
// A file name with an extension that starts with a letter: `main.rs`, `a.test.ts`, `.env.local`, but not `1.2.3` or `1.5s`.
const BARE_NAME = /^\.?[\w@+-][\w@+.-]*\.[A-Za-z][A-Za-z0-9]*$/;

/** Whether a token (already stripped of wrappers and suffix) looks like a file path. Resolving against the project decides if it is one. */
function looksLikePath(p: string): boolean {
  if (p.length === 0 || p.length > MAX_CANDIDATE || NOT_IN_PATH.test(p)) return false;
  // Never a network or device path: `\\srv\x`, `//srv/x`, `\\?\C:\x`. The core refuses these too; they are not even offered.
  if (/^[\\/]{2}/.test(p)) return false;
  if (WINDOWS_ABS.test(p)) return !p.slice(2).includes(":");
  if (p.includes(":")) return false;
  if (/^\/[^/]/.test(p) || /^~[\\/][^\\/]/.test(p)) return true;
  if (/^\.{1,2}([\\/]|$)/.test(p)) return /^(?:\.{1,2}[\\/])+[^\\/]/.test(p);
  const parts = p.split(/[\\/]/);
  if (parts.length > 1) return parts.every((s) => s.length > 0) && !parts.every((s) => /^\d+$/.test(s));
  return BARE_NAME.test(p);
}

/** The paths in a piece of terminal output, with the `:line:col` or `(line,col)` that follows them. A path with a space in it is not found. */
export function findPathCandidates(text: string): PathCandidate[] {
  const out: PathCandidate[] = [];
  for (const m of text.matchAll(/\S+/g)) {
    const token = m[0];
    // A URL is not a path, and its `//host/…` must not be read as one.
    if (token.includes("://")) continue;
    let from = 0;
    while (from < token.length && LEADING.has(token.charAt(from))) from++;
    let to = token.length;
    let line: number | undefined;
    let col: number | undefined;
    let suffix = to;
    for (;;) {
      while (to > from && TRAILING.has(token.charAt(to - 1))) to--;
      const paren = PAREN_SUFFIX.exec(token.slice(from, to));
      if (paren) {
        line = Number(paren[1]);
        col = paren[2] === undefined ? undefined : Number(paren[2]);
        suffix = to;
        to -= paren[0].length;
        break;
      }
      if (token[to - 1] === ")") {
        to--;
        continue;
      }
      const colon = COLON_SUFFIX.exec(token.slice(from, to));
      if (colon) {
        line = Number(colon[1]);
        col = colon[2] === undefined ? undefined : Number(colon[2]);
        suffix = to;
        to -= colon[0].length;
      } else {
        suffix = to;
      }
      break;
    }
    const path = token.slice(from, to);
    if (!looksLikePath(path)) continue;
    const start = m.index + from;
    const c: PathCandidate = { text: path, start, end: m.index + suffix };
    if (line !== undefined) c.line = line;
    if (col !== undefined) c.col = col;
    out.push(c);
  }
  return out;
}
