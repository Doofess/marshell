import type { Ref } from "react";
import type { FindQuery, FindStatus } from "./findText";

const NO_TEXT = "This PDF has no text to search. Its pages may be scanned images.";

/** What the count beside the field reads. It is not a live region: a step is announced through `stepText`. */
export function countText(s: FindStatus, query: string): string {
  if (query.trim() === "") return "";
  switch (s.state) {
    case "found":
      return `${s.current} of ${s.total}`;
    case "pending":
      return s.total > 0 ? `${s.current} of ${s.total}` : "";
    case "notFound":
      return "No matches";
    case "noText":
      return NO_TEXT;
    default:
      return "";
  }
}

/** What is said after a step. */
export function stepText(s: FindStatus): string {
  switch (s.state) {
    case "found":
      return `Match ${s.current} of ${s.total}, page ${s.page}.${s.wrapped === "start" ? " Continued from the start." : s.wrapped === "end" ? " Continued from the end." : ""}`;
    case "notFound":
      return "No matches.";
    case "noText":
      return NO_TEXT;
    default:
      return "";
  }
}

type KeyLike = { key: string; code: string; ctrlKey: boolean; altKey: boolean; metaKey: boolean; shiftKey: boolean };

/**
 * The keys a PDF tab answers: Ctrl+F, F3, Shift+F3 and F4, each with no other modifier. F is the physical key, as the app's
 * own shortcuts are (lib/keymap.ts), so Ctrl+F sits in the same place on a Hebrew, Cyrillic or Dvorak keyboard.
 */
export function pdfKey(e: KeyLike): "find" | "next" | "previous" | "sidebar" | null {
  if (e.altKey || e.metaKey) return null;
  if (e.ctrlKey) return !e.shiftKey && e.code === "KeyF" ? "find" : null;
  if (e.key === "F3") return e.shiftKey ? "previous" : "next";
  if (e.key === "F4" && !e.shiftKey) return "sidebar";
  return null;
}

/** What a key in the find field does: Enter steps (Shift+Enter back) and Esc closes, but not while an IME is composing. */
export function fieldKey(e: { key: string; shiftKey: boolean; isComposing: boolean }): "next" | "previous" | "close" | null {
  if (e.isComposing) return null;
  if (e.key === "Enter") return e.shiftKey ? "previous" : "next";
  if (e.key === "Escape") return "close";
  return null;
}

/** A selection inside the PDF that can fill the field: one line, 1 to 200 characters after trimming. */
export function selectionText(sel: { text: string; inside: boolean }): string | null {
  if (!sel.inside) return null;
  const t = sel.text.trim();
  if (t.length < 1 || t.length > 200 || /[\r\n]/.test(t)) return null;
  return t;
}

const chevron = (d: string) => (
  <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
    <path d={d} />
  </svg>
);

type Props = {
  query: FindQuery;
  status: FindStatus;
  onQuery: (q: FindQuery) => void;
  onStep: (previous: boolean) => void;
  onClose: () => void;
  field: Ref<HTMLInputElement>;
};

/** The find row under the toolbar. It wraps like the editor's: the two checkboxes drop to a second line when one line is too narrow. */
export function FindBar({ query, status, onQuery, onStep, onClose, field }: Props) {
  return (
    <div
      className="pdf__find"
      // Esc anywhere in the bar closes it, on its buttons and checkboxes as in the field. A handled Esc has its default prevented,
      // so the pane's Esc-to-terminal skips it.
      onKeyDown={(e) => {
        if (e.key !== "Escape" || e.shiftKey || e.ctrlKey || e.altKey || e.metaKey || e.nativeEvent.isComposing) return;
        e.preventDefault();
        onClose();
      }}
    >
      <input
        ref={field}
        className="pdf__findfield"
        type="text"
        autoComplete="off"
        spellCheck={false}
        placeholder="Find"
        aria-label="Find in PDF"
        value={query.text}
        onChange={(e) => onQuery({ ...query, text: e.target.value })}
        onKeyDown={(e) => {
          const k = fieldKey({ key: e.key, shiftKey: e.shiftKey, isComposing: e.nativeEvent.isComposing });
          // Esc goes on to the bar, which closes it.
          if (!k || k === "close") return;
          e.preventDefault();
          onStep(k === "previous");
        }}
      />
      <span className="pdf__count">{countText(status, query.text)}</span>
      <button type="button" className="pdf__button" aria-label="Previous" title="Previous match (Shift+Enter)" onClick={() => onStep(true)}>
        {chevron("M2.5 7.5 6 4l3.5 3.5")}
      </button>
      <button type="button" className="pdf__button" aria-label="Next" title="Next match (Enter)" onClick={() => onStep(false)}>
        {chevron("M2.5 4.5 6 8l3.5-3.5")}
      </button>
      <label className="pdf__check">
        <input type="checkbox" checked={query.caseSensitive} onChange={(e) => onQuery({ ...query, caseSensitive: e.target.checked })} />
        Match case
      </label>
      <label className="pdf__check">
        <input type="checkbox" checked={query.entireWord} onChange={(e) => onQuery({ ...query, entireWord: e.target.checked })} />
        Whole word
      </label>
      <button type="button" className="pdf__button pdf__close" aria-label="Close find bar" onClick={onClose}>
        {chevron("M3 3l6 6M9 3 3 9")}
      </button>
    </div>
  );
}
