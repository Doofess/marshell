import { useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import { selectedProps } from "../../../../components/a11y/SelectedMark";
import { listKey } from "../thumbs";
import type { NoteRow } from "./noteRows";

type Props = {
  name: string;
  /** The notes, or null while they are read. */
  rows: readonly NoteRow[] | null;
  /** The note the page in view starts with. */
  current: string | null;
  /** A note picked, by a click or by Enter. */
  onPick: (row: NoteRow) => void;
};

/** Scrolls the list (never an ancestor) just enough to show the row. */
function reveal(list: HTMLElement, row: HTMLElement) {
  const top = row.offsetTop;
  const bottom = top + row.offsetHeight;
  if (top < list.scrollTop) list.scrollTop = top;
  else if (bottom > list.scrollTop + list.clientHeight) list.scrollTop = bottom - list.clientHeight;
}

const icon = (
  <svg className="pdf-notes__icon" width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
    <path d="M2 2.5h8v5.5H6.5L4 10V8H2Z" />
  </svg>
);

/** The notes of a PDF as a list, in page order. The keyboard works as in the thumbnails: Up, Down, Home, End, Page Up and Page Down. */
export function NotesList({ name, rows, current, onPick }: Props) {
  const list = useRef<HTMLDivElement>(null);
  const inList = useRef(false);
  // The roving stop; null while the keyboard isn't in the list, when it follows the current note.
  const [at, setAt] = useState<string | null>(null);

  const row = (id: string) => list.current?.querySelector<HTMLElement>(`[data-note="${CSS.escape(id)}"]`) ?? null;

  // The current note is kept in the list's view while the keyboard is elsewhere.
  useLayoutEffect(() => {
    const el = list.current;
    const b = current ? row(current) : null;
    if (el && b && !inList.current) reveal(el, b);
  }, [current, rows]);

  if (rows === null) return null;
  if (rows.length === 0) {
    return (
      <div className="pdf-sidebar__empty">
        <p className="pdf-sidebar__none">This PDF has no notes yet.</p>
        <p className="pdf-sidebar__none">Select text to highlight it, or use the Free text tool.</p>
      </div>
    );
  }

  const ids = rows.map((r) => r.id);
  const rove = at !== null && ids.includes(at) ? at : current !== null && ids.includes(current) ? current : ids[0]!;

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey || e.nativeEvent.isComposing) return;
    if (!["ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(e.key)) return;
    const from = ids.indexOf((e.target as HTMLElement).closest<HTMLElement>("[data-note]")?.dataset.note ?? "");
    if (from < 0) return;
    e.preventDefault();
    const el = list.current;
    const first = row(ids[0]!);
    const perView = el && first ? Math.floor(el.clientHeight / Math.max(1, first.offsetHeight)) : 1;
    const to = listKey(from, ids.length, e.key, perView);
    if (to === null) return;
    const id = ids[to]!;
    setAt(id);
    const b = row(id);
    if (el && b) {
      b.focus({ preventScroll: true });
      reveal(el, b);
    }
  };

  return (
    <div
      ref={list}
      className="pdf-notes"
      role="list"
      aria-label={`Notes in ${name}`}
      onKeyDown={onKey}
      onFocus={() => {
        inList.current = true;
      }}
      onBlur={(e) => {
        if (e.relatedTarget instanceof Node && e.currentTarget.contains(e.relatedTarget)) return;
        inList.current = false;
        setAt(null);
      }}
    >
      {rows.map((r) => (
        <div key={r.id} role="listitem" className="pdf-notes__item">
          <button type="button" data-note={r.id} aria-label={r.label} tabIndex={r.id === rove ? 0 : -1} {...selectedProps(r.id === current, "current", "pdf-notes__row")} onFocus={() => setAt(r.id)} onClick={() => onPick(r)}>
            {r.kind === "freetext" ? (
              <span className="pdf-notes__mark pdf-notes__tee" aria-hidden="true">
                T
              </span>
            ) : r.swatch ? (
              <span className="pdf-notes__mark" aria-hidden="true">
                <span className="pdf-notes__swatch" style={{ backgroundColor: r.swatch } as CSSProperties} />
              </span>
            ) : (
              <span className="pdf-notes__mark" aria-hidden="true">
                {icon}
              </span>
            )}
            <span className="pdf-notes__body" aria-hidden="true">
              <span className="pdf-notes__text">{r.text}</span>
              {r.comment ? <span className="pdf-notes__comment">{r.comment}</span> : null}
              {r.warning ? <span className="pdf-notes__comment">{r.warning}</span> : null}
            </span>
            <span className="pdf-notes__page" aria-hidden="true">
              {r.page}
            </span>
          </button>
        </div>
      ))}
    </div>
  );
}
