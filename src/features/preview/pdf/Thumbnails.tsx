import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import type { PDFDocumentProxy } from "pdfjs-dist";
import { selectedProps } from "../../../components/a11y/SelectedMark";
import { renderPage } from "./pdfjs";
import { listKey, nextToDraw } from "./thumbs";

/** A thumbnail's width in px. PdfView.css has the same number. */
const THUMB_W = 160;

type Props = {
  doc: PDFDocumentProxy;
  /** Each page's size at scale 1. */
  sizes: readonly { w: number; h: number }[];
  /** The page in view. */
  current: number;
  name: string;
  onPick: (page: number) => void;
  /** Kept for the view's call. The view focuses the page in view itself, and this list has scrolled it into its own view by then, so it is unused. */
  focusOnShow: boolean;
};

const idle = (fn: () => void): (() => void) => {
  if (typeof requestIdleCallback === "function") {
    const h = requestIdleCallback(fn);
    return () => cancelIdleCallback(h);
  }
  const h = setTimeout(fn, 50);
  return () => clearTimeout(h);
};

/** Scrolls the list (never an ancestor) just enough to show the button. */
function reveal(list: HTMLElement, button: HTMLElement) {
  const top = button.offsetTop;
  const bottom = top + button.offsetHeight;
  if (top < list.scrollTop) list.scrollTop = top;
  else if (bottom > list.scrollTop + list.clientHeight) list.scrollTop = bottom - list.clientHeight;
}

/** The pages of a PDF as a list of small paper plates, drawn one at a time as they come near the list's view. */
export function Thumbnails({ doc, sizes, current, name, onPick }: Props) {
  const list = useRef<HTMLDivElement>(null);
  const inList = useRef(false);
  // The roving stop; null while the keyboard isn't in the list, when it follows the page in view.
  const [at, setAt] = useState<number | null>(null);
  const count = sizes.length;
  const rove = Math.min(Math.max(at ?? current, 1), Math.max(count, 1));
  const currentRef = useRef(current);
  currentRef.current = current;
  const sizesRef = useRef(sizes);
  sizesRef.current = sizes;
  const kick = useRef<() => void>(() => {});

  const button = (n: number) => list.current?.querySelector<HTMLElement>(`[data-thumb="${n}"]`) ?? null;

  // The page in view is kept in the list's view while the keyboard is elsewhere; at mount too, so the focus the view gives it lands on a visible button.
  useLayoutEffect(() => {
    const el = list.current;
    const b = button(current);
    if (el && b && !inList.current) reveal(el, b);
  }, [current, count]);

  // Drawing: one thumbnail at a time, at low priority. What is wanted is measured from the list's scroll position and height against each
  // button's offset: within a view of the visible area is wanted, and a canvas more than three views out is dropped.
  useEffect(() => {
    const el = list.current;
    if (!el) return;
    const abort = new AbortController();
    const drawn = new Set<number>();
    let running = false;
    let cancelWait: (() => void) | null = null;
    const plate = (n: number) => button(n)?.querySelector<HTMLElement>(".pdf-thumbs__plate") ?? null;

    const step = async () => {
      cancelWait = null;
      if (abort.signal.aborted || running) return;
      const view = el.clientHeight;
      if (view === 0) return;
      const top = el.scrollTop;
      const wanted = new Set<number>();
      for (let n = 1; n <= sizesRef.current.length; n++) {
        const b = button(n);
        if (!b) continue;
        const y = b.offsetTop;
        const bottom = y + b.offsetHeight;
        if (drawn.has(n) && (bottom < top - 3 * view || y > top + 4 * view)) {
          const c = plate(n)?.querySelector("canvas");
          if (c) {
            c.width = 0;
            c.height = 0;
            c.remove();
          }
          drawn.delete(n);
        } else if (bottom >= top - view && y <= top + 2 * view) wanted.add(n);
      }
      const n = nextToDraw(wanted, drawn, currentRef.current);
      if (n === null) return;
      running = true;
      try {
        const page = await doc.getPage(n);
        const size = sizesRef.current[n - 1];
        if (abort.signal.aborted || !size) return;
        const canvas = document.createElement("canvas");
        await renderPage(page, canvas, THUMB_W / size.w, abort.signal);
        if (abort.signal.aborted) return;
        const p = plate(n);
        if (p) {
          p.replaceChildren(canvas);
          drawn.add(n);
        }
      } catch {
        // A thumbnail that fails to draw stays blank; it counts as done so the others go on.
        drawn.add(n);
      } finally {
        running = false;
      }
      if (!abort.signal.aborted) schedule();
    };
    const schedule = () => {
      if (abort.signal.aborted || running || cancelWait) return;
      cancelWait = idle(() => void step());
    };
    kick.current = schedule;
    schedule();
    el.addEventListener("scroll", schedule, { passive: true });
    return () => {
      abort.abort();
      cancelWait?.();
      el.removeEventListener("scroll", schedule);
      kick.current = () => {};
      for (const c of el.querySelectorAll(".pdf-thumbs__plate canvas")) c.remove();
    };
  }, [doc]);

  // The page in view is nearest to draw first: a change of page or of the sizes asks again.
  useEffect(() => kick.current(), [current, count]);

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey || e.nativeEvent.isComposing) return;
    if (!["ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(e.key)) return;
    const from = Number((e.target as HTMLElement).closest<HTMLElement>("[data-thumb]")?.dataset.thumb);
    if (!from) return;
    e.preventDefault();
    const el = list.current;
    const first = button(1);
    const perView = el && first ? Math.floor(el.clientHeight / Math.max(1, first.offsetHeight)) : 1;
    const to = listKey(from - 1, count, e.key, perView);
    if (to === null) return;
    setAt(to + 1);
    const b = button(to + 1);
    if (el && b) {
      b.focus({ preventScroll: true });
      reveal(el, b);
    }
  };

  return (
    <div
      ref={list}
      className="pdf-thumbs"
      role="list"
      aria-label={`Pages of ${name}`}
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
      {sizes.map((s, i) => {
        const n = i + 1;
        const h = s.w > 0 ? (THUMB_W * s.h) / s.w : THUMB_W;
        return (
          <div key={n} role="listitem" className="pdf-thumbs__item">
            <button type="button" data-thumb={n} aria-label={`Page ${n}`} tabIndex={n === rove ? 0 : -1} {...selectedProps(n === current, "current", "pdf-thumbs__button")} onFocus={() => setAt(n)} onClick={() => onPick(n)}>
              <span className="pdf-thumbs__plate" aria-hidden="true" style={{ inlineSize: THUMB_W, blockSize: h } as CSSProperties} />
              <span className="pdf-thumbs__num">{n}</span>
            </button>
          </div>
        );
      })}
    </div>
  );
}
