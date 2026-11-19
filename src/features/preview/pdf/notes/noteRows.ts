import { winAnsiSafe } from "./notesDoc";
import { HIGHLIGHT_COLORS, type Note } from "./editor";

/** What the Notes list shows of one note. */
export type NoteRow = {
  id: string;
  page: number;
  top: number;
  editable: boolean;
  kind: Note["kind"];
  /** The colour the swatch takes, or null. */
  swatch: string | null;
  /** "Highlight, Yellow", "Free text", or "{kind}, read-only". */
  kindText: string;
  /** The line shown: the note's text (a highlight's in quotes), or else the kind. */
  text: string;
  /** The comment, shown under the text; empty when there is none. */
  comment: string;
  /** A line for free text the editor made with a letter outside WinAnsi, else null. */
  warning: string | null;
  /** The row's accessible name. */
  label: string;
};

const BLANK = "May show blank in Edge and Chrome.";

/** A colour's name when it is one of the five pdf.js colours, whatever its case; another colour has no name (no guessed values). */
export function colourName(hex: string | null): string | null {
  if (!hex) return null;
  const up = hex.toUpperCase();
  return HIGHLIGHT_COLORS.find((c) => c.value.toUpperCase() === up)?.name ?? null;
}

const join = (a: string, b: string) => (b === "" ? a : /[.!?]$/.test(a) ? `${a} ${b}` : `${a}. ${b}`);

/** The rows of a Notes list: by page, then by how far down the page. */
export function noteRows(notes: readonly Note[], name: (hex: string | null) => string | null): NoteRow[] {
  return [...notes]
    .sort((a, b) => a.page - b.page || a.top - b.top)
    .map((n) => {
      const colour = n.kind === "highlight" ? name(n.color) : null;
      const kindText = n.kind === "highlight" ? (colour ? `Highlight, ${colour}` : "Highlight") : n.kind === "freetext" ? "Free text" : `${n.kindName}, read-only`;
      const warning = n.kind === "freetext" && n.added && !winAnsiSafe(n.text) ? BLANK : null;
      let label = n.text === "" ? `${kindText}, page ${n.page}` : `${kindText}, page ${n.page}: ${n.text}`;
      label = join(label, n.comment);
      if (warning) label = join(label, warning);
      return {
        id: n.id,
        page: n.page,
        top: n.top,
        editable: n.editable,
        kind: n.kind,
        swatch: n.kind === "highlight" ? n.color : null,
        kindText,
        text: n.text === "" ? kindText : n.kind === "highlight" ? `“${n.text}”` : n.text,
        comment: n.comment,
        warning,
        label,
      };
    });
}

/** The note the list marks as current: the first one on the page in view. */
export function currentNoteId(rows: readonly NoteRow[], page: number): string | null {
  return rows.find((r) => r.page === page)?.id ?? null;
}
