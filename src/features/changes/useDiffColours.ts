import { useEffect, useMemo, useRef, useState } from "react";
import { COLOUR_CAP } from "../editor/tokens";
import { type LineTokens, type Tokenizer, tokenizer } from "../preview/highlight";
import { languageOf } from "../preview/kinds";
import type { Row, Section } from "./model";

/** A section's colours with the hash of the rows they came from, by path. */
export type ColourCache = Map<string, { hash: string; rows: LineTokens[] }>;

/** A hash of a section's rows (FNV-1a over kind and text): equal rows give an equal hash. */
export function sectionHash(rows: Row[]): string {
  let h = 0x811c9dc5;
  for (const r of rows) {
    const s = `${r.kind}\n${r.text}\n`;
    for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 0x01000193);
  }
  return `${rows.length}:${(h >>> 0).toString(16)}`;
}

/** The colours of one section: its new side (context and add rows) and old side (context and del rows), each with its own state. */
function colourRows(rows: Row[], tokenize: Tokenizer): LineTokens[] {
  const out: LineTokens[] = rows.map(() => []);
  const side = (keep: (r: Row) => boolean, mine: (r: Row) => boolean) => {
    const at: number[] = [];
    rows.forEach((r, i) => keep(r) && at.push(i));
    const { tokens } = tokenize(
      at.map((i) => rows[i]!.text),
      undefined,
    );
    at.forEach((i, n) => {
      if (mine(rows[i]!)) out[i] = tokens[n] ?? [];
    });
  };
  side((r) => r.kind === "context" || r.kind === "add", (r) => r.kind !== "del");
  side((r) => r.kind === "context" || r.kind === "del", (r) => r.kind === "del");
  return out;
}

/**
 * The token classes of each open section's rows, by path, one entry per row (a hunk's has none). A section is left out when it is
 * folded, has no language, holds more than COLOUR_CAP of text, or its language's tokenizer (`tokenizerFor`) hasn't loaded; that last
 * one is not cached, so it is coloured once the tokenizer arrives. A section whose hash is cached keeps its colours.
 */
export function colourSections(sections: Section[], cache: ColourCache, tokenizerFor: (language: string) => Tokenizer | undefined): Map<string, LineTokens[]> {
  const out = new Map<string, LineTokens[]>();
  const seen = new Set<string>();
  for (const s of sections) {
    if (s.folded || s.rows.length === 0) continue;
    const lang = languageOf(s.path);
    if (lang === undefined || s.rows.reduce((n, r) => n + r.text.length, 0) > COLOUR_CAP) continue;
    const hash = sectionHash(s.rows);
    const hit = cache.get(s.path);
    if (hit?.hash === hash) {
      seen.add(s.path);
      out.set(s.path, hit.rows);
      continue;
    }
    const tokenize = tokenizerFor(lang);
    if (tokenize === undefined) continue;
    const rows = colourRows(s.rows, tokenize);
    cache.set(s.path, { hash, rows });
    seen.add(s.path);
    out.set(s.path, rows);
  }
  // A path that is no longer in the list, or is folded, drops out of the cache.
  for (const path of [...cache.keys()]) if (!sections.some((s) => s.path === path)) cache.delete(path);
  return out;
}

/** The colours of the open sections: loads each language's tokenizer once, then colours in one pass. */
export function useDiffColours(sections: Section[]): Map<string, LineTokens[]> {
  const [tokenizers, setTokenizers] = useState<ReadonlyMap<string, Tokenizer>>(new Map());
  const cache = useRef<ColourCache>(new Map());
  const asked = useRef(new Set<string>());
  useEffect(() => {
    for (const s of sections) {
      const lang = s.folded || s.rows.length === 0 ? undefined : languageOf(s.path);
      if (lang === undefined || asked.current.has(lang)) continue;
      asked.current.add(lang);
      void tokenizer(lang).then((t) => t && setTokenizers((m) => new Map(m).set(lang, t)));
    }
  }, [sections]);
  return useMemo(() => colourSections(sections, cache.current, (l) => tokenizers.get(l)), [sections, tokenizers]);
}
