import { useCallback, useEffect, useId, useImperativeHandle, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode, type Ref } from "react";
import type { PDFDocumentProxy } from "pdfjs-dist";
import type { Place } from "../../pane/paneState";
import { afterLoadFailure } from "./loadFailure";
import { FindBar, pdfKey, selectionText, stepText } from "./FindBar";
import { PdfToolbar } from "./PdfToolbar";
import { usePdfFind } from "./findText";
import { placeOfDest } from "./dest";
import { handKeyboardOn, linksOf, PageLinks, type LinkGoal, type PageLink } from "./links";
import { bodyWidthChange, clampPlace, pageInView, placeAt, placeReady, placeText, scrollFor } from "./place";
import { ancestorsOf, loadOutline, pickSays, sectionAt, type OutlineNode } from "./outline";
import { PdfSidebar } from "./PdfSidebar";
import { Thumbnails } from "./Thumbnails";
import { loadPdf, renderPage, renderTextLayer, type TextMapping } from "./pdfjs";
import { keyboardLeft, setSidebarPref, sidebarPref, startOpen, startView, type SidebarView } from "./sidebarPref";
import { parsePageField, scaleFor, zoomIn, zoomOut, type Zoom } from "./zoom";
import { createNotesEditor, selectionLayer, type NoteTool, type NotesEditor } from "./notes/editor";
import { fromDocument, notesBlocked } from "./notes/blocked";
import { colorPref, setColorPref } from "./notes/notesPref";
import { SelectionBar, selectionBarShows } from "./notes/SelectionBar";
import { NotesBars } from "./notes/NotesBars";
import { notesEscape, onFocusMove, useNotes } from "./notes/useNotes";
import type { SaveCommands } from "../../editor/TextTab";
import { useOpenInPane } from "../../pane/PaneContext";
import "./PdfView.css";
import "./notes/notes.css";

type Props = {
  /** The file's path. The view is keyed by it: its notes, their draft and their saves belong to this file. */
  path: string;
  /** The file's bytes as last read. What is shown comes from the notes (`useNotes`): the file, or the draft its notes were kept in. */
  bytes: ArrayBuffer;
  /** The file's SHA-256 as last read, or null once it is deleted. */
  hash: string | null;
  readOnly: boolean;
  name: string;
  /** The tab's notes gained or lost unsaved changes: the window keeps the dot on the tab strip by it. */
  onDirty?: (path: string, dirty: boolean) => void;
  /** Gets the tab's save commands. */
  save?: Ref<SaveCommands | null>;
  onPages: (count: number) => void;
  /** The page in view, as the person scrolls or jumps. */
  onCurrentPage?: (page: number) => void;
  /** The loaded document after each load, and null once it is gone or failed. */
  onDocument?: (doc: PDFDocumentProxy | null) => void;
  /** Something laid over a page's box (a region being marked). The page wrapper is positioned and carries data-page. */
  renderOverlay?: (page: number) => ReactNode;
  /** Where the tab is in the file. The view scrolls there when `placeKey` changes, and at mount, once the document is laid out. */
  place?: Place | null;
  placeKey?: unknown;
  /** Every jump the person asks for (the Page field, and later the outline, the thumbnails and links). Without it the view scrolls itself. */
  onJump?: (to: Place, how: { keepFocus: boolean }) => void;
  /** Gets a function that reads the place at the top of the view, under the sticky head. */
  placeNow?: Ref<() => Place | null>;
  /** Gets the palette's commands for this PDF: null until the document has loaded, and again after an error. */
  commands?: Ref<PdfCommands | null>;
  /** Says a line through the window's announcer. */
  announce?: (text: string) => void;
  /** A web link on a page: the pane opens it in a browser tab, or asks. */
  onWebLink?: (url: string) => void;
  /** A Back or Forward action link: the tab's own history. */
  onHistory?: (by: -1 | 1) => void;
};

type Size = { w: number; h: number };

/** The width the body keeps free on each side of a page (--space-3). */
const SIDE = 12;
/** The sidebar's width (PdfView.css has the same number), and the body width under which it lies over the pages instead of beside them. */
const SIDEBAR_W = 200;
const OVERLAY_BELOW = 560;
/** The selection bar's size (notes.css has the same numbers), and its gap from the selection, for placing it inside the page. */
const BAR_W = 240;
const BAR_H = 28;
const BAR_GAP = 4;

/** The page in view in this scroller: the last page whose top has passed the upper third of the body. */
function pageInViewOf(scroller: HTMLElement, pages: readonly HTMLElement[]): number {
  const line = scroller.getBoundingClientRect().top + scroller.clientHeight / 3;
  return pageInView(line, pages.map((el) => ({ page: Number(el.dataset.page), top: el.getBoundingClientRect().top })));
}
/** How far under the sticky head a match lands when find brings it into view. */
const MATCH_GAP = 48;

type PageProps = {
  doc: PDFDocumentProxy;
  n: number;
  name: string;
  size: Size;
  scale: number;
  root: HTMLElement | null;
  overlay: ReactNode;
  /** Hands the drawn text layer to find; the function returned lets it go when the page is dropped or redrawn. */
  onText: (pageIndex: number, m: TextMapping) => () => void;
  /** A click or Enter on one of the page's links. */
  onFollow: (goal: LinkGoal) => void;
  /** The note editor, once it is ready: the canvas then leaves the notes it can change to it. */
  notes: NotesEditor | null;
  /** The selection bar, when the selection lies on this page: drawn over the page once the page is drawn. */
  bar: ReactNode;
};

/** One page: a placeholder of its final size until it is near the view, then the canvas and the text over it. */
function PdfPage({ doc, n, name, size, scale, root, overlay, onText, onFollow, notes, bar }: PageProps) {
  const holder = useRef<HTMLDivElement>(null);
  const [live, setLive] = useState(false);
  const [links, setLinks] = useState<PageLink[]>([]);

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
      handKeyboardOn(el.parentElement, document.activeElement, root);
      el.replaceChildren();
      setLinks([]);
      return;
    }
    const abort = new AbortController();
    let release: (() => void) | null = null;
    let detach: (() => void) | null = null;
    void (async () => {
      try {
        const page = await doc.getPage(n);
        // Drawn off to the side and swapped in whole, so a zoom or a reload never shows a blank page.
        const canvas = document.createElement("canvas");
        const text = document.createElement("div");
        text.className = "textLayer";
        const [, mapping] = await Promise.all([renderPage(page, canvas, scale, abort.signal, notes !== null), renderTextLayer(page, text, scale, abort.signal)]);
        if (abort.signal.aborted) return;
        el.replaceChildren(canvas, text);
        if (mapping) release = onText(n - 1, mapping);
        if (notes) detach = notes.attachPage(n, page, { holder: el, text }, scale);
        // The links are read once per drawn page; they are fractions of the page, so a zoom redraws them in place.
        const viewport = page.getViewport({ scale: 1 });
        const annotations = await page.getAnnotations({ intent: "display" });
        if (abort.signal.aborted) return;
        setLinks(linksOf(annotations, { width: viewport.width, height: viewport.height, toView: (r) => [...viewport.convertToViewportPoint(r[0]!, r[1]!), ...viewport.convertToViewportPoint(r[2]!, r[3]!)] as number[] }));
      } catch {
        // A page that fails to draw stays blank; the others go on.
      }
    })();
    return () => {
      abort.abort();
      detach?.();
      release?.();
    };
  }, [live, doc, n, scale, onText, notes]);

  return (
    <div className="pdf__page" role="group" aria-label={`${name}, page ${n}`} data-page={n} style={{ inlineSize: size.w * scale, blockSize: size.h * scale }}>
      <div ref={holder} className="pdf__sheet" />
      <PageLinks links={links} onFollow={onFollow} />
      {overlay}
      {live && bar}
    </div>
  );
}

/** What the palette can ask of a loaded PDF. */
export type PdfCommands = { find(): void; outline(): void; pages(): void };

/** A PDF as a column of pages: a toolbar, then every page at its final size, drawn as they come into view. */
export function PdfView({ path, bytes: fileBytes, hash, readOnly, name, onDirty, save, onPages, onCurrentPage, onDocument, renderOverlay, place, placeKey, onJump, placeNow, announce, onWebLink, onHistory, commands }: Props) {
  const root = useRef<HTMLDivElement>(null);
  const [scroller, setScroller] = useState<HTMLElement | null>(null);
  const [doc, setDoc] = useState<PDFDocumentProxy | null>(null);
  const docRef = useRef<PDFDocumentProxy | null>(null);
  const [sizes, setSizes] = useState<Size[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState<Zoom>({ kind: "fit" });
  // The body's width. What the pages may take of it follows: less the sidebar's, while the sidebar sits beside them.
  const [bodyW, setBodyW] = useState(0);
  const [current, setCurrent] = useState(1);
  const [draft, setDraft] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [view, setView] = useState<SidebarView>("outline");
  const [outline, setOutline] = useState<OutlineNode[] | null>(null);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set());
  const [sidebarTick, setSidebarTick] = useState(0);
  const sidebarId = useId();
  const sidebarButton = useRef<HTMLButtonElement>(null);
  // Whether the sidebar lies over the pages: it does while the body is narrower than OVERLAY_BELOW.
  const over = bodyW > 0 && bodyW < OVERLAY_BELOW;
  const beside = open && !over;
  const available = bodyW === 0 ? 0 : Math.max(0, bodyW - SIDE * 2 - (beside ? SIDEBAR_W : 0));

  const cb = useRef({ onPages, onCurrentPage, onDocument });
  cb.current = { onPages, onCurrentPage, onDocument };

  // The place to scroll to, and whether to say so: the one the view mounts with is silent, a later one is announced. It waits for
  // the document of the current bytes to be laid out, which may be a file still loading.
  const pending = useRef<{ place: Place; say: boolean } | null>(place ? { place, say: false } : null);
  const seenKey = useRef(placeKey);
  // The bytes and version the document on screen was loaded from.
  const loadedFor = useRef<{ bytes: ArrayBuffer; version: number } | null>(null);
  // The place read just before a change of scale, to be put back once the pages have their new sizes.
  const keep = useRef<Place | null>(null);
  const reading = useRef<() => Place | null>(() => null);
  const bodyWNow = useRef(0);
  const latest = useRef({ place, announce });
  latest.current = { place, announce };

  // Whether the PDF can take notes, asked of each loaded document and again when the file turns read-only or writable. Undefined until known.
  const [verdict, setVerdict] = useState<{ doc: PDFDocumentProxy; reason: string | null } | null>(null);
  const reason = verdict?.doc === doc ? verdict.reason : undefined;
  useEffect(() => {
    if (!doc) return;
    let dead = false;
    void notesBlocked(doc, readOnly, name).then(
      (r) => !dead && setVerdict({ doc, reason: r }),
      () => !dead && setVerdict({ doc, reason: null }),
    );
    return () => {
      dead = true;
    };
  }, [doc, readOnly, name]);
  // A reason from the document leaves the pages with no editor at all; read-only keeps it, so unsaved notes stay on screen.
  const noEditor = reason === undefined || fromDocument(reason, name);

  // The notes: what is shown is the file, or its draft, once the draft has been looked for.
  const openInPane = useOpenInPane();
  const notesNow = useNotes({ path, name, hash, bytes: fileBytes, readOnly, blocked: !!reason, announce, onDirty, openCopy: (p) => openInPane({ path: p }), save, focus: () => scroller?.focus() });
  const bytes = notesNow.shown?.bytes ?? null;
  const version = notesNow.shown?.version ?? 0;
  const runner = notesNow.runner;

  // The preview body is the scroll area and the root for what is in view.
  useLayoutEffect(() => {
    setScroller(root.current?.closest<HTMLElement>(".preview__body") ?? null);
  }, []);

  // Load on the bytes. The document on screen stays until the new one is ready, so a reload keeps its place.
  useEffect(() => {
    if (bytes === null) return;
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
        loadedFor.current = { bytes, version };
        setDoc(loaded);
        void old?.loadingTask.destroy();
        setError(null);
        cb.current.onPages(loaded.numPages);
        cb.current.onDocument?.(loaded);
      } catch (e) {
        if (dead) return;
        const after = afterLoadFailure(docRef.current !== null, e);
        if (after.keepDocument) return;
        const old = docRef.current;
        docRef.current = null;
        setDoc(null);
        void old?.loadingTask.destroy();
        setSizes([]);
        setError(after.error);
        cb.current.onDocument?.(null);
      }
    })();
    return () => {
      dead = true;
    };
  }, [bytes, version]);

  // The note editor, one per loaded document, over the pages in the body.
  const [notes, setNotes] = useState<NotesEditor | null>(null);
  useEffect(() => {
    if (!doc || !scroller || noEditor) return;
    let dead = false;
    let made: NotesEditor | null = null;
    void createNotesEditor(doc, { container: scroller, onComment: (req) => req.done(null), onNews: (line) => latest.current.announce?.(line) }).then(
      (e) => {
        if (dead) return e.destroy();
        made = e;
        notesNow.editor.current = e;
        e.setColor(colorPref());
        e.onChange(() => runner.dispatch({ type: "edited" }));
        setNotes(e);
      },
      () => {},
    );
    return () => {
      dead = true;
      setNotes(null);
      const gone = made;
      if (!gone) return;
      // A draft waiting is taken from this editor before it goes; the document waits for that too (below).
      void runner.unload().then(() => gone.destroy());
      if (notesNow.editor.current === gone) notesNow.editor.current = null;
    };
  }, [doc, scroller, noEditor, runner, notesNow.editor]);

  // The tool the person chose, handed to the editor whenever either changes; with notes blocked it is off, though the choice is kept.
  const [tool, setTool] = useState<NoteTool>("none");
  const effectiveTool: NoteTool = reason ? "none" : tool;
  useEffect(() => {
    notes?.setTool(effectiveTool);
  }, [notes, effectiveTool]);

  // The selection bar: after a pointer selection in one page's text ends, never with the Highlight tool on.
  const [selBar, setSelBar] = useState<{ page: number; style: CSSProperties } | null>(null);
  const [color, setColor] = useState(colorPref);
  useEffect(() => {
    if (!scroller || !selectionBarShows(notes !== null, effectiveTool, reason)) return setSelBar(null);
    let later = 0;
    const place = () => {
      const sel = window.getSelection();
      const layer = selectionLayer(sel, scroller);
      const page = layer?.closest<HTMLElement>(".pdf__page");
      const rects = sel && layer ? sel.getRangeAt(0).getClientRects() : null;
      if (!layer || !page || !rects || rects.length === 0) return setSelBar(null);
      const box = page.getBoundingClientRect();
      const first = rects[0]!;
      const last = rects[rects.length - 1]!;
      const left = Math.max(0, Math.min(last.right - box.left, box.width - BAR_W));
      // Under the selection's last line, or above its first when that would pass the page's bottom.
      const style: CSSProperties =
        last.bottom - box.top + BAR_GAP + BAR_H > box.height
          ? { insetInlineStart: left, insetBlockEnd: box.height - (first.top - box.top) + BAR_GAP }
          : { insetInlineStart: left, insetBlockStart: last.bottom - box.top + BAR_GAP };
      setSelBar({ page: Number(page.dataset.page), style });
    };
    const up = (e: PointerEvent) => {
      if (e.target instanceof Element && e.target.closest(".pdf-selbar")) return;
      clearTimeout(later);
      later = window.setTimeout(place, 0);
    };
    const change = () => {
      if (!selectionLayer(window.getSelection(), scroller)) setSelBar(null);
    };
    document.addEventListener("pointerup", up, true);
    document.addEventListener("selectionchange", change);
    return () => {
      clearTimeout(later);
      setSelBar(null);
      document.removeEventListener("pointerup", up, true);
      document.removeEventListener("selectionchange", change);
    };
  }, [notes, scroller, effectiveTool, reason]);
  // The editor announces the highlight it makes (`onNews`), as it does for the Highlight tool's and find's.
  const highlightBySelection = (comment: boolean) => {
    notes?.highlightSelection(comment);
    setSelBar(null);
    scroller?.focus({ preventScroll: true });
  };
  const pickColor = (hex: string) => {
    setColorPref(hex);
    setColor(hex);
    notes?.setColor(hex);
  };
  const bar = (
    <SelectionBar
      color={color}
      onColor={pickColor}
      onHighlight={() => highlightBySelection(false)}
      onComment={() => highlightBySelection(true)}
      onClose={() => {
        setSelBar(null);
        scroller?.focus({ preventScroll: true });
      }}
      style={selBar?.style}
    />
  );

  // pdf.js takes keys only while the keyboard is in the tab and the window has focus; a focus move is read a tick later.
  useEffect(() => {
    if (!notes || !scroller) return;
    let later = 0;
    const check = () => {
      clearTimeout(later);
      later = window.setTimeout(() => onFocusMove(notes, scroller, document.activeElement, document.hasFocus()), 0);
    };
    check();
    scroller.addEventListener("focusin", check);
    scroller.addEventListener("focusout", check);
    window.addEventListener("focus", check);
    window.addEventListener("blur", check);
    return () => {
      clearTimeout(later);
      notes.setLive(false);
      scroller.removeEventListener("focusin", check);
      scroller.removeEventListener("focusout", check);
      window.removeEventListener("focus", check);
      window.removeEventListener("blur", check);
    };
  }, [notes, scroller]);

  // Gone with the view, once a draft waiting has been taken from its notes.
  useEffect(
    () => () => {
      cb.current.onDocument?.(null);
      const gone = docRef.current;
      docRef.current = null;
      void runner.unload().then(() => gone?.loadingTask.destroy());
    },
    [runner],
  );

  // Fit width follows the body.
  useEffect(() => {
    if (!scroller) return;
    const measure = () => {
      const next = bodyWidthChange(bodyWNow.current, scroller.clientWidth);
      if (next === null) return;
      // The pages are about to take new sizes: the place at the top of the view is read before they do.
      keep.current = reading.current();
      bodyWNow.current = next;
      setBodyW(next);
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(scroller);
    return () => ro.disconnect();
  }, [scroller]);

  // Only the pages: the sidebar's thumbnails are not pages of the column.
  const pageEls = useCallback(() => Array.from(root.current?.querySelectorAll<HTMLElement>(".pdf__pagelist [data-page]") ?? []), []);

  // The page in view is the last one whose top has passed the upper third of the body.
  useEffect(() => {
    if (!scroller || sizes.length === 0) return;
    let frame = 0;
    const update = () => {
      frame = 0;
      const page = pageInViewOf(scroller, pageEls());
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
  useEffect(() => {
    notes?.setPage(current, shownScale);
  }, [notes, current, shownScale]);

  // What is stuck above the pages: the toolbar (with the find bar), and the "deleted" notice when it is there.
  const headHeight = () => (root.current?.querySelector<HTMLElement>(".pdf__bar")?.offsetHeight ?? 0) + (scroller?.querySelector<HTMLElement>(".preview__notice")?.offsetHeight ?? 0);
  // Where the pages' top is read and put: under the head, one SIDE down.
  const viewTop = () => (scroller?.getBoundingClientRect().top ?? 0) + headHeight() + SIDE;

  const readPlace = (): Place | null => {
    if (!scroller) return null;
    const boxes = pageEls().map((el) => {
      const r = el.getBoundingClientRect();
      return { page: Number(el.dataset.page), top: r.top, height: r.height };
    });
    return placeAt(viewTop(), boxes);
  };
  reading.current = readPlace;
  useImperativeHandle(placeNow, () => readPlace);

  // Scrolls so the place is at the top of the view, and gives the page in view then (null when it can not scroll yet). It joins no
  // history: the jumps that do go through `onJump`.
  const applyPlace = (p: Place): number | null => {
    const to = clampPlace(p, count);
    const el = pageEls()[to.page - 1];
    if (!el || !scroller) return null;
    const r = el.getBoundingClientRect();
    scroller.scrollTop += scrollFor(to, { page: to.page, top: r.top, height: r.height }, viewTop());
    const page = pageInViewOf(scroller, pageEls());
    setCurrent(page);
    return page;
  };

  // The tab went to a place: scroll there once the document of these bytes is laid out, and say so unless it is the first place.
  useLayoutEffect(() => {
    if (seenKey.current !== placeKey) {
      seenKey.current = placeKey;
      if (latest.current.place) pending.current = { place: latest.current.place, say: true };
    }
    const next = pending.current;
    if (!next || !placeReady(loadedFor.current, { bytes, version }, doc !== null && scroller !== null && available > 0 && count > 0)) return;
    pending.current = null;
    keep.current = null;
    // What is said is the page the page field then shows: a place near the end of a page reads as the next one, which fills the view.
    const shown = applyPlace(next.place);
    if (next.say && shown !== null) latest.current.announce?.(placeText(shown, count));
  }, [placeKey, doc, scroller, available, count, bytes, version]);

  // A change of scale keeps the reading place: the zoom, and the width fit width follows, read it before they change.
  const scaleKey = zoom.kind === "scale" ? `scale ${zoom.value}` : `fit ${available}`;
  useLayoutEffect(() => {
    const was = keep.current;
    keep.current = null;
    if (was && !pending.current && count > 0) applyPlace(was);
  }, [scaleKey]);
  const setScale = (z: Zoom) => {
    keep.current = readPlace();
    setZoom(z);
  };

  // Find's scrolls stay in the body: a match goes MATCH_GAP under the head, and to the middle when it is off to a side.
  const find = usePdfFind(doc, {
    page: () => current,
    pages: () => count,
    // A step of find is a scroll, not a jump: it joins no history.
    showPage: (n) => applyPlace({ page: n, top: 0 }),
    isPageVisible: (n) => {
      const el = pageEls()[n - 1];
      if (!el || !scroller) return false;
      const r = el.getBoundingClientRect();
      const s = scroller.getBoundingClientRect();
      return r.bottom > s.top + headHeight() && r.top < s.bottom;
    },
    showMatch: (el) => {
      if (!scroller) return;
      const r = el.getBoundingClientRect();
      const s = scroller.getBoundingClientRect();
      scroller.scrollTop += r.top - (s.top + headHeight() + MATCH_GAP);
      if (r.left < s.left || r.right > s.left + scroller.clientWidth) scroller.scrollLeft += r.left + r.width / 2 - (s.left + scroller.clientWidth / 2);
    },
  });

  // Highlight or Comment on find's current match: its letters become the selection, and the editor highlights that.
  const noteFromFind = (comment: boolean) => {
    const range = find.currentRange();
    const sel = window.getSelection();
    if (!range || !sel || !notes) return;
    sel.removeAllRanges();
    sel.addRange(range);
    notes.highlightSelection(comment);
  };

  const commit = () => {
    const n = draft === null ? null : parsePageField(draft, count);
    setDraft(null);
    if (n) {
      const to = { page: n, top: 0 };
      if (onJump) onJump(to, { keepFocus: false });
      else applyPlace(to);
    }
    scroller?.focus({ preventScroll: true });
  };

  // Follows a link. A jump goes through `onJump` as the Page field's does, so it joins the tab's history.
  const follow = (goal: LinkGoal, how: { keepFocus: boolean }) => {
    const go = (to: Place) => (onJump ? onJump(to, how) : applyPlace(to));
    switch (goal.kind) {
      case "dest":
        void placeOfDest(docRef.current!, goal.dest).then((to) => {
          if (to) go(to);
          else latest.current.announce?.("This link leads nowhere in the file.");
        });
        return;
      case "action": {
        const last = Math.max(count, 1);
        if (goal.action === "GoBack") onHistory?.(-1);
        else if (goal.action === "GoForward") onHistory?.(1);
        else if (goal.action === "NextPage") go({ page: Math.min(current + 1, last), top: 0 });
        else if (goal.action === "PrevPage") go({ page: Math.max(current - 1, 1), top: 0 });
        else if (goal.action === "FirstPage") go({ page: 1, top: 0 });
        else go({ page: last, top: 0 });
        return;
      }
      case "web":
        onWebLink?.(goal.url);
        return;
      case "other":
        latest.current.announce?.("Only links within the file and web links open here.");
    }
  };

  // The sidebar sticks under the head and is as tall as the body below it; PdfView.css reads these two off the root.
  useLayoutEffect(() => {
    const el = root.current;
    if (!scroller || !el || error) return;
    const bar = el.querySelector<HTMLElement>(".pdf__bar");
    const sync = () => {
      el.style.setProperty("--pdf-bar", `${bar?.offsetHeight ?? 0}px`);
      el.style.setProperty("--pdf-view", `${scroller.clientHeight}px`);
    };
    sync();
    const ro = new ResizeObserver(sync);
    ro.observe(scroller);
    if (bar) ro.observe(bar);
    return () => ro.disconnect();
  }, [scroller, error]);

  // The outline is read once per document. A document that gives none, or fails to, reads as having none.
  useEffect(() => {
    if (!doc) return setOutline(null);
    let dead = false;
    void loadOutline(doc).then((o) => {
      if (!dead) setOutline(o);
    });
    return () => {
      dead = true;
    };
  }, [doc]);

  // The section that holds the page in view: its parents open so it shows, and nothing is ever collapsed here.
  const section = outline ? sectionAt(outline, current) : null;
  useEffect(() => {
    if (!section) return;
    const above = ancestorsOf(section);
    setExpanded((was) => (above.every((id) => was.has(id)) ? was : new Set([...was, ...above])));
  }, [section, outline]);
  const toggleEntry = (id: string, next: boolean) =>
    setExpanded((was) => {
      const set = new Set(was);
      if (next) set.add(id);
      else set.delete(id);
      return set;
    });

  const focusPages = () => scroller?.focus({ preventScroll: true });
  // Opening or closing the sidebar beside the pages changes the width fit width gives them: the place is read before.
  const setSidebarOpen = (next: boolean) => {
    if (zoom.kind === "fit" && !over) keep.current = readPlace();
    setOpen(next);
  };
  const showSidebar = () => {
    setSidebarOpen(true);
    setSidebarPref({ open: true });
    setSidebarTick((t) => t + 1);
  };
  // `record` is whether the person chose this: a close after a pick, by Esc, or by a click outside records nothing.
  const hideSidebar = (record: boolean) => {
    setSidebarOpen(false);
    if (record) setSidebarPref({ open: false });
  };
  const toggleSidebar = () => {
    if (open) {
      hideSidebar(true);
      focusPages();
    } else showSidebar();
  };

  // The tab opens as the person last left the sidebar, once the document, its outline and the body's width are known.
  const started = useRef(false);
  useEffect(() => {
    if (started.current || !doc || outline === null || bodyW === 0) return;
    started.current = true;
    const pref = sidebarPref();
    setView(startView(pref, outline.length > 0));
    if (startOpen(pref, over)) setSidebarOpen(true);
  }, [doc, outline, bodyW]);

  // Opened by the person, the sidebar takes the keyboard: the section's entry, or else the view's first stop, or else the tab.
  useLayoutEffect(() => {
    if (sidebarTick === 0) return;
    const side = root.current?.querySelector<HTMLElement>(".pdf-sidebar");
    const panel = side?.querySelector<HTMLElement>('[role="tabpanel"]');
    const target =
      panel?.querySelector<HTMLElement>('[aria-current="true"]') ??
      panel?.querySelector<HTMLElement>('[tabindex="0"]') ??
      panel?.querySelector<HTMLElement>("button") ??
      side?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]');
    target?.focus({ preventScroll: true });
  }, [sidebarTick]);

  // Lying over the pages, the sidebar closes when a pointer goes down, or the keyboard goes, anywhere outside it and its button.
  // The keyboard stays where it went. Where it went is read a moment later, once it has landed, so a window losing the focus
  // (Alt+Tab) isn't taken for the keyboard going: the focus comes back to the sidebar with the window.
  useEffect(() => {
    const side = root.current?.querySelector<HTMLElement>(".pdf-sidebar");
    if (!open || !over || !side) return;
    const outside = (t: EventTarget | null) => !(t instanceof Node) || !(side.contains(t) || sidebarButton.current?.contains(t));
    const down = (e: PointerEvent) => {
      if (outside(e.target)) setOpen(false);
    };
    let later = 0;
    const out = () => {
      clearTimeout(later);
      later = window.setTimeout(() => {
        if (keyboardLeft(document.hasFocus(), document.activeElement, (t) => !outside(t as EventTarget | null))) setOpen(false);
      }, 0);
    };
    document.addEventListener("pointerdown", down, true);
    side.addEventListener("focusout", out);
    return () => {
      clearTimeout(later);
      document.removeEventListener("pointerdown", down, true);
      side.removeEventListener("focusout", out);
    };
  }, [open, over]);

  // A pick jumps. Beside the pages the keyboard stays on the entry; lying over them the sidebar closes and the keyboard goes to the pages.
  // An entry with no destination says so, and the sidebar and the keyboard stay.
  const pick = (node: OutlineNode) => {
    const say = pickSays(node);
    if (say !== null || !node.goal) return void latest.current.announce?.(say ?? "");
    if (over) {
      follow(node.goal, { keepFocus: false });
      hideSidebar(false);
      focusPages();
    } else follow(node.goal, { keepFocus: true });
  };
  // A thumbnail picked jumps to the top of its page, with the keyboard and the closing as for an outline pick.
  const pickPage = (n: number) => {
    const to: Place = { page: n, top: 0 };
    if (over) {
      if (onJump) onJump(to, { keepFocus: false });
      else applyPlace(to);
      hideSidebar(false);
      focusPages();
    } else if (onJump) onJump(to, { keepFocus: true });
    else applyPlace(to);
  };
  const leaveSidebar = () => {
    if (over) hideSidebar(false);
    focusPages();
  };

  const [findOpen, setFindOpen] = useState(false);
  const [focusTick, setFocusTick] = useState(0);
  const field = useRef<HTMLInputElement>(null);
  // What had the keyboard when the bar opened; it gets it back on close.
  const returnTo = useRef<Element | null>(null);
  // Set by a step: the next result (found, none, or no text) is announced once.
  const sayNext = useRef(false);

  // Opens the bar, or focuses it when it is open; the field's text is selected, refilled from a selection inside the PDF.
  const openFind = () => {
    const sel = window.getSelection();
    const picked = selectionText({ text: sel?.toString() ?? "", inside: !!sel?.anchorNode && !!root.current?.contains(sel.anchorNode) });
    if (!findOpen) {
      returnTo.current = document.activeElement;
      setFindOpen(true);
    }
    const text = picked ?? find.query.text;
    if (text.trim() && (!findOpen || picked !== null)) find.search({ ...find.query, text });
    setFocusTick((t) => t + 1);
  };
  // The handle calls through a ref, so it always runs this render's closures; it exists only once the document has loaded.
  const paletteRef = useRef<PdfCommands | null>(null);
  const showSidebarView = (v: SidebarView) => {
    setView(v);
    setSidebarPref({ view: v });
    showSidebar();
  };
  paletteRef.current = { find: openFind, outline: () => showSidebarView("outline"), pages: () => showSidebarView("pages") };
  const loaded = doc !== null && !error;
  useImperativeHandle<PdfCommands | null, PdfCommands | null>(
    commands,
    () => (loaded ? { find: () => paletteRef.current?.find(), outline: () => paletteRef.current?.outline(), pages: () => paletteRef.current?.pages() } : null),
    [loaded],
  );
  useLayoutEffect(() => {
    if (focusTick === 0) return;
    field.current?.focus();
    field.current?.select();
  }, [focusTick]);

  const closeFind = () => {
    find.close();
    sayNext.current = false;
    setFindOpen(false);
    const back = returnTo.current;
    returnTo.current = null;
    if (back instanceof HTMLElement && back.isConnected && back !== field.current) back.focus({ preventScroll: true });
    else scroller?.focus({ preventScroll: true });
  };
  const step = (previous: boolean) => {
    sayNext.current = true;
    find.step(previous);
  };
  useEffect(() => {
    if (!sayNext.current) return;
    const s = find.status;
    if (s.state !== "found" && s.state !== "notFound" && s.state !== "noText") return;
    sayNext.current = false;
    latest.current.announce?.(stepText(s));
  }, [find.status]);

  // Ctrl+F, F3 and Shift+F3 work anywhere in the body, as Ctrl+= / Ctrl+- / Ctrl+0 do; so the listener is on the body.
  const keys = useRef<(e: globalThis.KeyboardEvent) => void>(() => {});
  keys.current = (e) => {
    notesEscape(e, notes);
    const k = pdfKey(e);
    if (k === "find" || k === "next" || k === "previous") {
      // F3 with no search going does what Ctrl+F does; it does not step as well.
      if (k === "find" || !findOpen || !find.query.text.trim()) openFind();
      else step(k === "previous");
      e.preventDefault();
      return;
    }
    if (k === "sidebar") {
      if (doc) toggleSidebar();
      e.preventDefault();
      return;
    }
    if (!e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === "=" || e.key === "+") setScale(zoomIn(zoom, shownScale));
    else if (e.key === "-") setScale(zoomOut(zoom, shownScale));
    else if (e.key === "0") setScale({ kind: "fit" });
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
    <div ref={root} className="pdf" data-sidebar={open ? (over ? "over" : "beside") : "closed"}>
      <div className="pdf__bar" role="toolbar" aria-label="PDF controls">
        <PdfToolbar
          page={draft ?? String(current)}
          count={count}
          percent={Math.round(shownScale * 100)}
          fit={zoom.kind === "fit"}
          findOpen={findOpen}
          tool={effectiveTool}
          blocked={reason ?? null}
          onTool={setTool}
          sidebar={
            <button
              ref={sidebarButton}
              type="button"
              className="pdf__button pdf__sidebarbutton"
              disabled={!doc}
              aria-expanded={open}
              aria-controls={sidebarId}
              aria-keyshortcuts="F4"
              aria-label="Outline and pages"
              title="Outline and pages (F4)"
              onClick={toggleSidebar}
            >
              {/* A panel with its left part filled: the sidebar. Three lines are the app's Menu. */}
              <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
                <rect x="1.5" y="2.5" width="11" height="9" rx="1.5" />
                <path d="M3 2.5h2v9H3a1.5 1.5 0 0 1-1.5-1.5V4A1.5 1.5 0 0 1 3 2.5Z" fill="currentColor" stroke="none" />
              </svg>
            </button>
          }
          onPageDraft={setDraft}
          onPageCommit={commit}
          onPageCancel={() => setDraft(null)}
          onZoomOut={() => setScale(zoomOut(zoom, shownScale))}
          onZoomIn={() => setScale(zoomIn(zoom, shownScale))}
          onFit={() => setScale({ kind: "fit" })}
          // The button shows and hides the bar, as its aria-expanded says; Ctrl+F only ever opens it.
          onFind={() => (findOpen ? closeFind() : openFind())}
          findRow={findOpen ? <FindBar query={find.query} status={find.status} onQuery={find.search} onStep={step} onNote={noteFromFind} notesOff={!notes || !!reason} onClose={closeFind} field={field} /> : null}
        />
        <NotesBars
          name={name}
          conflict={notesNow.state.conflict === null ? null : notesNow.state.conflict.hash === null ? "deleted" : "changed"}
          line={notesNow.state.line}
          undo={notesNow.state.lastSave !== null && !notesNow.state.saving}
          copy={notesNow.state.refusedSave}
          onUndo={() => runner.dispatch({ type: "unsave" })}
          onCopy={() => runner.dispatch({ type: "copy" })}
          onReload={() => runner.dispatch({ type: "reload" })}
          onOverwrite={() => runner.dispatch({ type: "overwrite" })}
          onSave={() => runner.dispatch({ type: "overwrite" })}
        />
      </div>
      <div className="pdf__main">
        {open && (
          <PdfSidebar
            id={sidebarId}
            name={name}
            view={view}
            onView={(v, takeKeyboard) => {
              setView(v);
              setSidebarPref({ view: v });
              if (takeKeyboard) setSidebarTick((t) => t + 1);
            }}
            outline={outline}
            section={section}
            expanded={expanded}
            onToggle={toggleEntry}
            onPick={pick}
            onLeave={leaveSidebar}
            pages={doc ? <Thumbnails doc={doc} sizes={sizes} current={current} name={name} onPick={pickPage} /> : null}
          />
        )}
        <div className="pdf__pagelist">
          {doc && sizes.map((size, i) => <PdfPage key={i} doc={doc} n={i + 1} name={name} size={size} scale={scaleOf(i)} root={scroller} overlay={renderOverlay?.(i + 1)} onText={find.attach} onFollow={(g) => follow(g, { keepFocus: false })} notes={notes} bar={selBar?.page === i + 1 ? bar : null} />)}
          {!doc && <p className="preview__message">Loading…</p>}
        </div>
      </div>
    </div>
  );
}
