import type { TextMapping } from "./pdfjs";

/**
 * Where one match lies in a drawn text layer: the span (`div`, an index into `TextMapping.divs`) and the character in
 * it where the match begins, and the same for where it ends (exclusive).
 */
export type Span = { begin: { div: number; offset: number }; end: { div: number; offset: number } };

/**
 * Maps a page's matches onto its text layer. pdf.js's find controller reports each match as an offset and a length in
 * the page's text items joined end to end (line ends not counted), which is `items.join("")` here, because the text
 * layer was built from the same items without normalization. A match begins in the item that holds its first
 * character (an empty item holds none) and ends in the item that holds its last. Matches come in order; one that runs
 * past the text is dropped.
 */
export function matchSpans(matches: readonly number[], lengths: readonly number[], items: readonly string[]): Span[] {
  const spans: Span[] = [];
  const total = items.reduce((n, s) => n + s.length, 0);
  const last = items.length - 1;
  let i = 0;
  let start = 0;
  for (let k = 0; k < matches.length; k++) {
    const from = matches[k]!;
    const to = from + (lengths[k] ?? 0);
    if (!(from >= 0 && to > from && to <= total)) continue;
    while (i < last && from >= start + items[i]!.length) start += items[i++]!.length;
    let j = i;
    let s = start;
    while (j < last && to > s + items[j]!.length) s += items[j++]!.length;
    spans.push({ begin: { div: i, offset: from - start }, end: { div: j, offset: to - s } });
  }
  return spans;
}

/** The spans each mapping has tinted, so they can be put back. */
const painted = new WeakMap<TextMapping, Set<number>>();

/**
 * Tints the matches on a drawn text layer: each matched run becomes `<span class="pdf__match">`, and the runs of the
 * selected match (`selected`, an index into `spans`, or -1) also get `pdf__match--current`. The text layer is invisible
 * over the canvas, so the tint is all that shows. The last paint is cleared first. Returns the first run of the
 * selected match, for the find controller to scroll to, or null.
 */
export function paint(m: TextMapping, spans: readonly Span[], selected: number): HTMLElement | null {
  unpaint(m);
  const runs = new Map<number, { from: number; to: number; current: boolean }[]>();
  const add = (div: number, from: number, to: number, current: boolean) => {
    if (to <= from || !m.divs[div]) return;
    const list = runs.get(div) ?? [];
    list.push({ from, to, current });
    runs.set(div, list);
  };
  spans.forEach((sp, k) => {
    const current = k === selected;
    if (sp.begin.div === sp.end.div) return add(sp.begin.div, sp.begin.offset, sp.end.offset, current);
    add(sp.begin.div, sp.begin.offset, m.items[sp.begin.div]?.length ?? 0, current);
    for (let d = sp.begin.div + 1; d < sp.end.div; d++) add(d, 0, m.items[d]?.length ?? 0, current);
    add(sp.end.div, 0, sp.end.offset, current);
  });

  let first: HTMLElement | null = null;
  const firstDiv = selected >= 0 ? spans[selected]?.begin.div : undefined;
  const done = new Set<number>();
  for (const [div, list] of runs) {
    const el = m.divs[div]!;
    const text = m.items[div] ?? "";
    const nodes: (string | HTMLElement)[] = [];
    let at = 0;
    for (const r of list) {
      if (r.from > at) nodes.push(text.slice(at, r.from));
      const mark = el.ownerDocument.createElement("span");
      mark.className = r.current ? "pdf__match pdf__match--current" : "pdf__match";
      mark.textContent = text.slice(r.from, r.to);
      nodes.push(mark);
      if (r.current && div === firstDiv && !first) first = mark;
      at = r.to;
    }
    if (at < text.length) nodes.push(text.slice(at));
    el.replaceChildren(...nodes);
    done.add(div);
  }
  painted.set(m, done);
  return first;
}

/** Puts the plain text back in every span the last paint tinted. */
export function unpaint(m: TextMapping): void {
  const done = painted.get(m);
  if (!done) return;
  for (const div of done) {
    const el = m.divs[div];
    if (el) el.textContent = m.items[div] ?? "";
  }
  painted.delete(m);
}
