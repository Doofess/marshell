const OPEN = "\u0002";
const CLOSE = "\u0003";

/** Splits a search snippet at its hit marks (U+0002 opens, U+0003 closes). An unclosed hit runs to the end; stray closes are dropped. */
export function splitSnippet(s: string): { text: string; hit: boolean }[] {
  const out: { text: string; hit: boolean }[] = [];
  let hit = false;
  let text = "";
  const flush = () => {
    if (text) out.push({ text, hit });
    text = "";
  };
  for (const ch of s) {
    if (ch === OPEN) {
      flush();
      hit = true;
    } else if (ch === CLOSE) {
      if (hit) {
        flush();
        hit = false;
      }
    } else text += ch;
  }
  flush();
  return out;
}
