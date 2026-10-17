/** What a previewed page may load: nothing but its own inline styles and data: images. No script, font, frame, form target or fetch. */
export const CSP = "default-src 'none'; style-src 'unsafe-inline'; img-src data:";

// A tag whose attributes may hold a quoted ">" ("a>b"), so a plain [^>]* would stop in the middle of it.
const TAG_BODY = `(?:[^>"']|"[^"]*"|'[^']*')*`;
const META = new RegExp(`<meta\\b${TAG_BODY}>`, "gi");
const BASE = new RegExp(`<base\\b${TAG_BODY}>`, "gi");
// The cruder reading, as a second net: a meta that names http-equiv before any ">" at all.
const HTTP_EQUIV = /<meta\b[^>]*http-equiv[^>]*>/gi;

/**
 * The page for the preview iframe: the file's own text, behind a Content-Security-Policy meta that comes before anything of the
 * file's, so no tag of the file can load before the policy is in force. A `<meta http-equiv>` (a refresh would navigate the
 * frame) and a `<base>` (it would redirect every relative URL) are removed. The iframe's sandbox is the stronger wall; this
 * is the second.
 */
export function buildSrcdoc(html: string): string {
  const cleaned = html
    .replace(META, (tag) => (/http-equiv/i.test(tag) ? "" : tag))
    .replace(HTTP_EQUIV, "")
    .replace(BASE, "");
  return `<!doctype html><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${CSP}">${cleaned}`;
}
