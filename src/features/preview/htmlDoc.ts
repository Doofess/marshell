/** What a previewed page may load: nothing but its own inline styles and data: images. No script, font, frame, form target or fetch. */
export const CSP = "default-src 'none'; style-src 'unsafe-inline'; img-src data:";

/** The five characters an HTML parser ends a tag name on, besides ">" and "/". */
const isSpace = (c: number) => c === 0x20 || c === 0x09 || c === 0x0a || c === 0x0c || c === 0x0d;
const lower = (c: number) => (c >= 0x41 && c <= 0x5a ? c | 0x20 : c);

/** True when `word` (lower case ASCII) sits at `at`, whatever its case. */
function wordAt(html: string, at: number, word: string): boolean {
  if (at + word.length > html.length) return false;
  for (let k = 0; k < word.length; k++) if (lower(html.charCodeAt(at + k)) !== word.charCodeAt(k)) return false;
  return true;
}

/**
 * Where the tag whose name ends at `i` ends: the index after its ">", or the length of the text when it never closes. It reads
 * attributes the way a parser does: a name ends at whitespace, "/", "=" or ">"; a value after "=" is double-quoted,
 * single-quoted or bare, and a bare value ends at whitespace or ">" (so an apostrophe in it opens nothing).
 */
function tagEnd(html: string, i: number): number {
  const n = html.length;
  while (i < n) {
    let c = html.charCodeAt(i);
    if (isSpace(c) || c === 0x2f) {
      i++;
      continue;
    }
    if (c === 0x3e) return i + 1;
    // An attribute name; an "=" that starts it belongs to the name.
    i++;
    while (i < n) {
      c = html.charCodeAt(i);
      if (isSpace(c) || c === 0x2f || c === 0x3e || c === 0x3d) break;
      i++;
    }
    while (i < n && isSpace(html.charCodeAt(i))) i++;
    if (html.charCodeAt(i) !== 0x3d) continue;
    i++;
    while (i < n && isSpace(html.charCodeAt(i))) i++;
    c = html.charCodeAt(i);
    if (c === 0x22 || c === 0x27) {
      const close = html.indexOf(c === 0x22 ? '"' : "'", i + 1);
      if (close < 0) return n;
      i = close + 1;
    } else if (c !== 0x3e) {
      while (i < n && !isSpace(html.charCodeAt(i)) && html.charCodeAt(i) !== 0x3e) i++;
    }
  }
  return n;
}

/** Whether the text in [from, to) names an http-equiv anywhere, in or out of a value (the cautious reading). */
function hasHttpEquiv(html: string, from: number, to: number): boolean {
  // A manual walk: indexOf would look past `to` for a "-" that may never come, once per tag.
  for (let k = from + 4; k + 6 <= to; k++) {
    if (html.charCodeAt(k) === 0x2d && wordAt(html, k - 4, "http-equiv")) return true;
  }
  return false;
}

/**
 * One left-to-right pass: every `<base …>` and every `<meta …>` that names an http-equiv is cut out whole (a tag that never
 * closes is cut to the end of the text). Each character is looked at a bounded number of times, so the time is linear in the text.
 */
function dropTags(html: string): string {
  const n = html.length;
  const kept: string[] = [];
  let from = 0;
  let i = html.indexOf("<");
  while (i >= 0 && i < n) {
    const isMeta = wordAt(html, i + 1, "meta");
    if (isMeta || wordAt(html, i + 1, "base")) {
      const after = i + 5;
      const c = after < n ? html.charCodeAt(after) : 0x3e;
      if (isSpace(c) || c === 0x2f || c === 0x3e) {
        const end = tagEnd(html, after);
        if (!isMeta || hasHttpEquiv(html, i, end)) {
          kept.push(html.slice(from, i));
          from = end;
          i = html.indexOf("<", end);
          continue;
        }
        i = html.indexOf("<", end);
        continue;
      }
    }
    i = html.indexOf("<", i + 1);
  }
  if (from === 0) return html;
  kept.push(html.slice(from));
  return kept.join("");
}

/** How many passes may run before the cleanup stops being clever. */
const MAX_ROUNDS = 8;

/**
 * The blunt last resort: no `<meta` or `<base` is left anywhere, tag or not. The text is copied through a stack and a copy
 * that completes either word pops it again, so a join made by a removal is caught too. Still one linear pass.
 */
function dropEveryOpening(html: string): string {
  const n = html.length;
  const out = new Uint16Array(n);
  let len = 0;
  for (let k = 0; k < n; k++) {
    out[len++] = html.charCodeAt(k);
    if (len >= 5 && out[len - 5] === 0x3c) {
      const w = String.fromCharCode(lower(out.at(len - 4) ?? 0), lower(out.at(len - 3) ?? 0), lower(out.at(len - 2) ?? 0), lower(out.at(len - 1) ?? 0));
      if (w === "meta" || w === "base") len -= 5;
    }
  }
  const parts: string[] = [];
  for (let k = 0; k < len; k += 8192) parts.push(String.fromCharCode(...out.subarray(k, Math.min(len, k + 8192))));
  return parts.join("");
}

/**
 * Removing a tag can join the text around it into a new one (`<me<meta x>ta http-equiv=refresh>`), so the pass repeats until
 * the text stops changing. A file that still changes after MAX_ROUNDS is built to do that; it loses every `<meta` and `<base`
 * outright.
 */
function stripNavigation(html: string): string {
  let cur = html;
  for (let i = 0; i < MAX_ROUNDS; i++) {
    const next = dropTags(cur);
    if (next === cur) return cur;
    cur = next;
  }
  return dropEveryOpening(cur);
}

/**
 * The page for the preview iframe: the file's own text, behind a Content-Security-Policy meta that comes before anything of the
 * file's, so no tag of the file can load before the policy is in force. A `<meta http-equiv>` (a refresh would navigate the
 * frame) and a `<base>` (it would redirect every relative URL) are removed. The iframe's sandbox is the stronger wall; this
 * is the second.
 */
export function buildSrcdoc(html: string): string {
  return `<!doctype html><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${CSP}">${stripNavigation(html)}`;
}
