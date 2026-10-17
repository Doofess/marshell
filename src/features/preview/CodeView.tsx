import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { HIGHLIGHT_CAP, highlightLines, type Token } from "./highlight";
import type { PreviewEntry } from "./previewState";

/** The text's lines. A final newline ends the last line instead of starting an empty one. */
export function splitLines(text: string): string[] {
  const lines = text.split(/\r\n|\r|\n/);
  if (lines.length > 1 && lines[lines.length - 1] === "") lines.pop();
  return lines;
}

function tokenStyle(t: Token): CSSProperties | undefined {
  if (!t.color && !t.bold && !t.italic && !t.underline) return undefined;
  return { color: t.color, fontWeight: t.bold ? 600 : undefined, fontStyle: t.italic ? "italic" : undefined, textDecoration: t.underline ? "underline" : undefined };
}

type Props = {
  text: string;
  /** The Shiki language; left out for plain text. */
  language?: string;
  /** The entry that asked for a line: scrolls its line to about a third from the top, once per entry. A refetch does not scroll. */
  target?: PreviewEntry;
};

/**
 * The file as numbered rows. The plain text is on screen at once; when Shiki has loaded and coloured it, the same rows
 * gain their colours. Tokens are React spans, never markup from a string, so nothing in a file can become an element.
 */
export function CodeView({ text, language, target }: Props) {
  const plain = useMemo(() => splitLines(text), [text]);
  const big = text.length > HIGHLIGHT_CAP;
  const [coloured, setColoured] = useState<{ text: string; lines: Token[][] } | null>(null);
  useEffect(() => {
    if (!language || big) return;
    let stale = false;
    void highlightLines(text, language).then((lines) => {
      if (!stale && lines) setColoured({ text, lines });
    });
    return () => {
      stale = true;
    };
  }, [text, language, big]);
  // Colours that belong to an older version of the file are not used for the new text.
  const tokens = coloured && coloured.text === text ? coloured.lines : null;
  const rows = tokens && tokens.length >= plain.length ? tokens : null;

  const root = useRef<HTMLDivElement>(null);
  const line = target?.line;
  useEffect(() => {
    const row = line === undefined ? null : root.current?.querySelector<HTMLElement>('[data-target="true"]');
    const body = root.current?.closest<HTMLElement>(".preview__body");
    if (row && body) body.scrollTop = Math.max(0, row.offsetTop - body.clientHeight / 3);
    // Keyed on the entry, so a refetch of the same file keeps the reader's place.
  }, [target, line]);

  const digits = String(plain.length).length;
  return (
    <>
      {big && <p className="preview__note">Syntax colours are off for files over 300 KB.</p>}
      <div ref={root} className="code" style={{ "--digits": digits } as CSSProperties}>
        {plain.map((raw, i) => (
          <div key={i} className="code__row" data-target={line === i + 1 ? "true" : undefined}>
            <span className="code__no" aria-hidden="true">
              {i + 1}
            </span>
            <span className="code__text">{rows ? rows[i]!.map((t, j) => <span key={j} style={tokenStyle(t)}>{t.content}</span>) : raw}</span>
          </div>
        ))}
      </div>
    </>
  );
}
