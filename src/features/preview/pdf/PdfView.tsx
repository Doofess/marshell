import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { PDFDocumentProxy } from "pdfjs-dist";
import { loadPdf, pdfErrorMessage, renderPage, renderTextLayer } from "./pdfjs";
import { parsePageField, scaleFor, zoomIn, zoomOut, type Zoom } from "./zoom";
import "./PdfView.css";

type Props = {
  bytes: ArrayBuffer;
  /** Counts refetches; a new value reloads the document and keeps the scroll position. */
  version: number;
  name: string;
  onPages: (count: number) => void;
  /** The page in view, as the person scrolls or jumps. */
  onCurrentPage?: (page: number) => void;
  /** The loaded document after each load, and null once it is gone or failed. */
  onDocument?: (doc: PDFDocumentProxy | null) => void;
  /** Something laid over a page's box (a region being marked). The page wrapper is positioned and carries data-page. */
  renderOverlay?: (page: number) => ReactNode;
};

type Size = { w: number; h: number };

/** The width the body keeps free on each side of a page (--space-3). */
const SIDE = 12;

type PageProps = {
  doc: PDFDocumentProxy;
  n: number;
  name: string;
  size: Size;
  scale: number;
  root: HTMLElement | null;
  overlay: ReactNode;
};

/** One page: a placeholder of its final size until it is near the view, then the canvas and the text over it. */
function PdfPage({ doc, n, name, size, scale, root, overlay }: PageProps) {
  const holder = useRef<HTMLDivElement>(null);
  const [live, setLive] = useState(false);

  // Near the view (one screen out) it is drawn; more than three screens away its canvas is dropped.
  useEffect(() => {
    const el = holder.current;
    if (!el || !root) return;
    const near = new IntersectionObserver((es) => es.forEach((e) => e.isIntersecting && setLive(true)), { root, rootMargin: "100% 0px" });
    const far = new IntersectionObserver((es) => es.forEach((e) => !e.isIntersecting && setLive(false)), { root, rootMargin: "300% 0px" });
    near.observe(el);
    far.observe(el);
    return () => {
      near.disconnect();
      far.disconnect();
    };
  }, [root]);

  useEffect(() => {
    const el = holder.current;
    if (!el) return;
    if (!live) {
      el.replaceChildren();
      return;
    }
    const abort = new AbortController();
    void (async () => {
      try {
        const page = await doc.getPage(n);
        // Drawn off to the side and swapped in whole, so a zoom or a reload never shows a blank page.
        const canvas = document.createElement("canvas");
        const text = document.createElement("div");
        text.className = "textLayer";
        await Promise.all([renderPage(page, canvas, scale, abort.signal), renderTextLayer(page, text, scale, abort.signal)]);
        if (!abort.signal.aborted) el.replaceChildren(canvas, text);
      } catch {
        // A page that fails to draw stays blank; the others go on.
      }
    })();
    return () => abort.abort();
  }, [live, doc, n, scale]);

  return (
    <div className="pdf__page" role="group" aria-label={`${name}, page ${n}`} data-page={n} style={{ inlineSize: size.w * scale, blockSize: size.h * scale }}>
      <div ref={holder} className="pdf__sheet" />
      {overlay}
    </div>
  );
}

/** A PDF as a column of pages: a toolbar, then every page at its final size, drawn as they come into view. */
export function PdfView({ bytes, version, name, onPages, onCurrentPage, onDocument, renderOverlay }: Props) {
  const root = useRef<HTMLDivElement>(null);
  const [scroller, setScroller] = useState<HTMLElement | null>(null);
  const [doc, setDoc] = useState<PDFDocumentProxy | null>(null);
  const docRef = useRef<PDFDocumentProxy | null>(null);
  const [sizes, setSizes] = useState<Size[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState<Zoom>({ kind: "fit" });
  const [available, setAvailable] = useState(0);
  const [current, setCurrent] = useState(1);
  const [draft, setDraft] = useState<string | null>(null);

  const cb = useRef({ onPages, onCurrentPage, onDocument });
  cb.current = { onPages, onCurrentPage, onDocument };

  // The preview body is the scroll area and the root for what is in view.
  useLayoutEffect(() => {
    setScroller(root.current?.closest<HTMLElement>(".preview__body") ?? null);
  }, []);

  // Load on the bytes. The document on screen stays until the new one is ready, so a reload keeps its place.
  useEffect(() => {
    let dead = false;
    let loaded: PDFDocumentProxy | null = null;
    void (async () => {
      try {
        loaded = await loadPdf(bytes);
        const pages = await Promise.all(Array.from({ length: loaded.numPages }, async (_, i) => (await loaded!.getPage(i + 1)).getViewport({ scale: 1 })));
        if (dead) return void loaded.loadingTask.destroy();
        setSizes(pages.map((v) => ({ w: v.width, h: v.height })));
        const old = docRef.current;
        docRef.current = loaded;
        setDoc(loaded);
        void old?.loadingTask.destroy();
        setError(null);
        cb.current.onPages(loaded.numPages);
        cb.current.onDocument?.(loaded);
      } catch (e) {
        if (dead) return;
        const old = docRef.current;
        docRef.current = null;
        setDoc(null);
        void old?.loadingTask.destroy();
        setSizes([]);
        setError(pdfErrorMessage(e));
        cb.current.onDocument?.(null);
      }
    })();
    return () => {
      dead = true;
    };
  }, [bytes, version]);

  // Gone with the view.
  useEffect(
    () => () => {
      cb.current.onDocument?.(null);
      void docRef.current?.loadingTask.destroy();
      docRef.current = null;
    },
    [],
  );

  // Fit width follows the body.
  useEffect(() => {
    if (!scroller) return;
    const measure = () => setAvailable(Math.max(0, scroller.clientWidth - SIDE * 2));
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(scroller);
    return () => ro.disconnect();
  }, [scroller]);

  const pageEls = useCallback(() => Array.from(root.current?.querySelectorAll<HTMLElement>("[data-page]") ?? []), []);

  // The page in view is the last one whose top has passed the upper third of the body.
  useEffect(() => {
    if (!scroller || sizes.length === 0) return;
    let frame = 0;
    const update = () => {
      frame = 0;
      const top = scroller.getBoundingClientRect().top;
      const line = top + scroller.clientHeight / 3;
      let page = 1;
      for (const el of pageEls()) {
        if (el.getBoundingClientRect().top <= line) page = Number(el.dataset.page);
        else break;
      }
      setCurrent((c) => (c === page ? c : page));
    };
    const onScroll = () => {
      if (!frame) frame = requestAnimationFrame(update);
    };
    update();
    scroller.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      scroller.removeEventListener("scroll", onScroll);
      if (frame) cancelAnimationFrame(frame);
    };
  }, [scroller, sizes, zoom, available, pageEls]);

  useEffect(() => {
    if (doc) cb.current.onCurrentPage?.(current);
  }, [current, doc]);

  const count = sizes.length;
  const scaleOf = (i: number) => scaleFor(zoom, sizes[i]?.w ?? 0, available);
  const shownScale = scaleOf(current - 1);

  const jump = (n: number) => {
    const el = pageEls()[n - 1];
    if (!el || !scroller) return;
    // What is stuck above the pages: the toolbar, and the "deleted" notice when it is there.
    const bar = (root.current?.querySelector<HTMLElement>(".pdf__bar")?.offsetHeight ?? 0) + (scroller.querySelector<HTMLElement>(".preview__notice")?.offsetHeight ?? 0);
    scroller.scrollTop += el.getBoundingClientRect().top - scroller.getBoundingClientRect().top - bar - SIDE;
    setCurrent(n);
  };

  const commit = () => {
    const n = draft === null ? null : parsePageField(draft, count);
    setDraft(null);
    if (n) jump(n);
    scroller?.focus({ preventScroll: true });
  };

  // Ctrl+= / Ctrl+- / Ctrl+0 work anywhere in the body, which is where focus sits, not only inside the viewer; so the listener is on the body.
  const keys = useRef<(e: globalThis.KeyboardEvent) => void>(() => {});
  keys.current = (e) => {
    if (!e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === "=" || e.key === "+") setZoom(zoomIn(zoom, shownScale));
    else if (e.key === "-") setZoom(zoomOut(zoom, shownScale));
    else if (e.key === "0") setZoom({ kind: "fit" });
    else return;
    e.preventDefault();
  };
  useEffect(() => {
    if (!scroller) return;
    const on = (e: globalThis.KeyboardEvent) => keys.current(e);
    scroller.addEventListener("keydown", on);
    return () => scroller.removeEventListener("keydown", on);
  }, [scroller]);

  if (error) return <p className="preview__message">{error}</p>;

  return (
    <div ref={root} className="pdf">
      <div className="pdf__bar" role="toolbar" aria-label="PDF controls">
        <label className="pdf__pages">
          Page
          <input
            className="pdf__field"
            inputMode="numeric"
            autoComplete="off"
            spellCheck={false}
            disabled={count === 0}
            value={draft ?? String(current)}
            aria-label={`Page, of ${count}`}
            onFocus={(e) => e.currentTarget.select()}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={() => setDraft(null)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                commit();
              }
            }}
          />
          <span>of {count}</span>
        </label>
        <span className="pdf__spacer" />
        <button type="button" className="pdf__button" disabled={count === 0} onClick={() => setZoom(zoomOut(zoom, shownScale))} title="Zoom out (Ctrl+-)">
          Zoom out
        </button>
        <span className="pdf__percent">
          {Math.round(shownScale * 100)}%
        </span>
        <button type="button" className="pdf__button" disabled={count === 0} onClick={() => setZoom(zoomIn(zoom, shownScale))} title="Zoom in (Ctrl+=)">
          Zoom in
        </button>
        <button type="button" className="pdf__button" disabled={count === 0} aria-pressed={zoom.kind === "fit"} onClick={() => setZoom({ kind: "fit" })} title="Fit width (Ctrl+0)">
          Fit width
        </button>
      </div>
      <div className="pdf__pagelist">
        {doc && sizes.map((size, i) => <PdfPage key={i} doc={doc} n={i + 1} name={name} size={size} scale={scaleOf(i)} root={scroller} overlay={renderOverlay?.(i + 1)} />)}
        {!doc && <p className="preview__message">Loading…</p>}
      </div>
    </div>
  );
}
