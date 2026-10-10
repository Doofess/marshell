/**
 * WCAG 2.5.3 "label in name", the way axe's label-content-name-mismatch rule checks it: the text a sighted person
 * reads on a control, with punctuation and case ignored, must appear in its accessible name. Text nodes next to each
 * other with no whitespace between them are read as one word, so "10" beside "docs-site" is "10docssite" and a name
 * that says "10 docs-site" does not contain it. For tests of one server-rendered control (the root of the markup).
 */
const ENTITIES: Record<string, string> = { "&amp;": "&", "&lt;": "<", "&gt;": ">", "&quot;": '"', "&#x27;": "'", "&#39;": "'" };

const normalise = (s: string) =>
  s
    .replace(/&(?:amp|lt|gt|quot|#x27|#39);/g, (m) => ENTITIES[m] ?? m)
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, "")
    .replace(/\s+/g, " ")
    .trim();

/** What a sighted person reads: the markup's text, aria-hidden or not, minus drawings. Tags add no whitespace. */
export function visibleText(html: string): string {
  return normalise(html.replace(/<svg[\s\S]*?<\/svg>/g, "").replace(/<[^>]*>/g, ""));
}

/** The aria-label of the first element in the markup, normalised, or null when it has none. */
export function labelOf(html: string): string | null {
  const m = /^<[^>]*?\saria-label="([^"]*)"/.exec(html.trim());
  return m ? normalise(m[1]!) : null;
}

/** True when the control's accessible name contains everything it shows. */
export function nameContainsLabel(html: string): boolean {
  const name = labelOf(html);
  if (name === null) return true; // named from its content: always contains it
  return name.includes(visibleText(html));
}
