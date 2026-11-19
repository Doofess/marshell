import { useCallback, useEffect, useRef, useState } from "react";
import type { PDFDocumentProxy } from "pdfjs-dist";
import { matchSpans, paint, unpaint } from "./highlight";
import { viewerLib, type TextMapping } from "./pdfjs";

export type FindQuery = { text: string; caseSensitive: boolean; entireWord: boolean };

/**
 * Where a search stands. `current` and `total` count matches from 1 (0 when there are none); `page` is the selected
 * match's page; `wrapped` says the last step went round, from the start going forward or from the end going back.
 */
export type FindStatus = {
  state: "idle" | "pending" | "found" | "notFound" | "noText";
  current: number;
  total: number;
  page: number | null;
  wrapped: "start" | "end" | null;
};

export const IDLE: FindStatus = { state: "idle", current: 0, total: 0, page: null, wrapped: null };

/** pdf.js's `FindState`. */
const FOUND = 0;
const NOT_FOUND = 1;
const WRAPPED = 2;
const PENDING = 3;

/** pdf.js's find state, as the bar shows it. A PDF whose pages hold no text at all says so rather than "No matches". */
export function statusOf(state: number, matches: { current: number; total: number }, previous: boolean, page: number | null, textless: boolean): FindStatus {
  const count = { current: matches.current, total: matches.total };
  switch (state) {
    case PENDING:
      return { state: "pending", ...count, page, wrapped: null };
    case WRAPPED:
      return { state: "found", ...count, page, wrapped: previous ? "end" : "start" };
    case NOT_FOUND:
      return { state: textless ? "noText" : "notFound", current: 0, total: 0, page: null, wrapped: null };
    case FOUND:
    default:
      return { state: "found", ...count, page, wrapped: null };
  }
}

/**
 * What find needs from the view: the page in view, how many there are, a way to bring a page into view (the find
 * controller does this to reach a match on a page not drawn yet), whether a page shows, and a way to bring a tinted
 * match into view. Neither scroll joins the tab's history.
 */
export type FindView = {
  page(): number;
  pages(): number;
  showPage(n: number): void;
  isPageVisible(n: number): boolean;
  showMatch(el: HTMLElement): void;
};

export type PdfFind = {
  setDocument(doc: PDFDocumentProxy | null): void;
  /** Searches as the person types; a blank query clears the search. */
  search(q: FindQuery): void;
  step(previous: boolean): void;
  close(): void;
  /** A drawn page's text layer, to be tinted; the function returned lets it go. */
  attach(pageIndex: number, m: TextMapping): () => void;
  onStatus(cb: (s: FindStatus) => void): () => void;
  destroy(): void;
};

type ViewerLib = Pick<typeof import("pdfjs-dist/web/pdf_viewer.mjs"), "EventBus" | "PDFFindController">;
type FindEvent = { state: number; previous: boolean; matchesCount: { current: number; total: number } };

/**
 * pdf.js's find controller over the tab's own pages. A small stand-in for pdf.js's link service gives it the view's
 * page and page count, and its "updatetextlayermatches" events tint the attached text layers with Marshell's own
 * highlighter. `lib` is pdf.js's viewer module, loaded on first use unless a test passes one.
 */
export async function createPdfFind(view: FindView, lib?: ViewerLib): Promise<PdfFind> {
  const { EventBus, PDFFindController } = lib ?? (await viewerLib());
  const bus = new EventBus();
  const linkService = {
    get page() {
      return view.page();
    },
    set page(n: number) {
      view.showPage(n);
    },
    get pagesCount() {
      return view.pages();
    },
  };
  const controller = new PDFFindController({ linkService: linkService as never, eventBus: bus, updateMatchesCountOnProgress: true });
  controller.onIsPageVisible = (n: number) => view.isPageVisible(n);

  const pages = new Map<number, TextMapping>();
  const listeners = new Set<(s: FindStatus) => void>();
  let status = IDLE;
  /** The search the bar holds open; null while it is closed or empty. */
  let query: FindQuery | null = null;
  let doc: PDFDocumentProxy | null = null;

  const emit = (s: FindStatus) => {
    status = s;
    for (const l of listeners) l(s);
  };
  const selected = () => controller.selected ?? { pageIdx: -1, matchIdx: -1 };
  const selectedPage = () => (selected().pageIdx >= 0 ? selected().pageIdx + 1 : null);
  // The page texts pdf.js read for the search (a field of its own, in the pinned pdfjs-dist): all blank means no text.
  const textless = () => {
    const contents = (controller as unknown as { _pageContents?: (string | undefined)[] })._pageContents ?? [];
    return contents.length > 0 && Array.from(contents).every((t) => !t?.trim());
  };

  const repaint = (i: number) => {
    const m = pages.get(i);
    if (!m) return;
    if (!controller.highlightMatches) return unpaint(m);
    const spans = matchSpans(controller.pageMatches?.[i] ?? [], controller.pageMatchesLength?.[i] ?? [], m.items);
    const sel = selected();
    const current = sel.pageIdx === i ? sel.matchIdx : -1;
    const mark = paint(m, spans, current);
    // pdf.js scrolls with element.scrollIntoView, which also scrolls clipped ancestors of the pane; the view scrolls instead.
    if (mark) controller.scrollMatchIntoView({ element: { scrollIntoView: () => view.showMatch(mark) } as unknown as HTMLElement, pageIndex: i, matchIndex: current });
  };

  bus.on("updatetextlayermatches", (e: { pageIndex: number }) => {
    if (e.pageIndex === -1) for (const i of pages.keys()) repaint(i);
    else repaint(e.pageIndex);
  });
  bus.on("updatefindcontrolstate", (e: FindEvent) => {
    if (query) emit(statusOf(e.state, e.matchesCount, e.previous, selectedPage(), e.state === NOT_FOUND && textless()));
  });
  bus.on("updatefindmatchescount", (e: FindEvent) => {
    if (query) emit({ ...status, current: e.matchesCount.current, total: e.matchesCount.total });
  });

  const find = (type: "" | "again", q: FindQuery, previous: boolean) =>
    bus.dispatch("find", {
      source: null,
      type,
      query: q.text,
      caseSensitive: q.caseSensitive,
      entireWord: q.entireWord,
      highlightAll: true,
      findPrevious: previous,
      matchDiacritics: false,
    });
  const clear = () => {
    query = null;
    bus.dispatch("findbarclose", { source: null });
    for (const m of pages.values()) unpaint(m);
    emit(IDLE);
  };

  return {
    setDocument(next) {
      if (next === doc) return;
      doc = next;
      controller.setDocument(next as PDFDocumentProxy);
      if (next && query) find("", query, false);
    },
    search(q) {
      if (!q.text.trim()) return clear();
      query = q;
      find("", q, false);
    },
    step(previous) {
      if (query) find("again", query, previous);
    },
    close: clear,
    attach(i, m) {
      pages.set(i, m);
      repaint(i);
      return () => {
        if (pages.get(i) === m) pages.delete(i);
      };
    },
    onStatus(cb) {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    destroy() {
      query = null;
      listeners.clear();
      pages.clear();
      doc = null;
      controller.setDocument(null as unknown as PDFDocumentProxy);
    },
  };
}

/**
 * Find for `PdfView`. pdf.js's viewer module loads on the first search. Pages attach their text layers from the start,
 * so a search tints the pages already drawn.
 */
export function usePdfFind(
  doc: PDFDocumentProxy | null,
  view: FindView,
): { status: FindStatus; query: FindQuery; search(q: FindQuery): void; step(previous: boolean): void; close(): void; attach: PdfFind["attach"] } {
  const viewRef = useRef(view);
  viewRef.current = view;
  const docRef = useRef(doc);
  docRef.current = doc;
  const engine = useRef<Promise<PdfFind> | null>(null);
  const attached = useRef(new Map<number, { m: TextMapping; detach: (() => void) | null }>());
  const [status, setStatus] = useState<FindStatus>(IDLE);
  const [query, setQuery] = useState<FindQuery>({ text: "", caseSensitive: false, entireWord: false });

  const start = useCallback(() => {
    engine.current ??= (async () => {
      const f = await createPdfFind({
        page: () => viewRef.current.page(),
        pages: () => viewRef.current.pages(),
        showPage: (n) => viewRef.current.showPage(n),
        isPageVisible: (n) => viewRef.current.isPageVisible(n),
        showMatch: (el) => viewRef.current.showMatch(el),
      });
      f.onStatus(setStatus);
      f.setDocument(docRef.current);
      for (const [i, a] of attached.current) a.detach = f.attach(i, a.m);
      return f;
    })();
    return engine.current;
  }, []);
  const withEngine = (fn: (f: PdfFind) => void) => void engine.current?.then(fn);

  useEffect(() => {
    void engine.current?.then((f) => f.setDocument(doc));
  }, [doc]);

  useEffect(
    () => () => {
      const e = engine.current;
      engine.current = null;
      void e?.then((f) => f.destroy());
    },
    [],
  );

  const search = useCallback(
    (q: FindQuery) => {
      setQuery(q);
      void start().then((f) => f.search(q));
    },
    [start],
  );
  const step = useCallback((previous: boolean) => withEngine((f) => f.step(previous)), []);
  const close = useCallback(() => {
    withEngine((f) => f.close());
    setStatus(IDLE);
  }, []);
  const attach = useCallback((i: number, m: TextMapping) => {
    const entry = { m, detach: null as (() => void) | null };
    attached.current.set(i, entry);
    void engine.current?.then((f) => {
      if (attached.current.get(i) === entry) entry.detach = f.attach(i, m);
    });
    return () => {
      entry.detach?.();
      if (attached.current.get(i) === entry) attached.current.delete(i);
    };
  }, []);

  return { status, query, search, step, close, attach };
}
