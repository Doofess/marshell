import type { PDFDocumentProxy } from "pdfjs-dist";
import { describe, expect, it, vi } from "vitest";
import { createPdfFind, statusOf, type FindStatus, type FindView } from "./findText";
import type { TextMapping } from "./pdfjs";

describe("statusOf", () => {
  it("statusOf_found", () => {
    expect(statusOf(0, { current: 2, total: 5 }, false, 3, false)).toEqual({ state: "found", current: 2, total: 5, page: 3, wrapped: null });
  });

  it("statusOf_wrapped_forward_is_from_the_start", () => {
    expect(statusOf(2, { current: 1, total: 5 }, false, 1, false)).toEqual({ state: "found", current: 1, total: 5, page: 1, wrapped: "start" });
  });

  it("statusOf_wrapped_back_is_from_the_end", () => {
    expect(statusOf(2, { current: 5, total: 5 }, true, 9, false)).toEqual({ state: "found", current: 5, total: 5, page: 9, wrapped: "end" });
  });

  it("statusOf_pending_keeps_the_count", () => {
    expect(statusOf(3, { current: 1, total: 4 }, false, 2, false)).toEqual({ state: "pending", current: 1, total: 4, page: 2, wrapped: null });
  });

  it("statusOf_no_matches", () => {
    expect(statusOf(1, { current: 0, total: 0 }, false, null, false)).toEqual({ state: "notFound", current: 0, total: 0, page: null, wrapped: null });
  });

  it("statusOf_no_text", () => {
    expect(statusOf(1, { current: 0, total: 0 }, false, null, true)).toEqual({ state: "noText", current: 0, total: 0, page: null, wrapped: null });
  });
});

/** Stand-ins for pdf.js's event bus and find controller: the bus delivers at once, the controller only records. */
class FakeBus {
  sent: [string, Record<string, unknown>][] = [];
  private handlers = new Map<string, ((e: Record<string, unknown>) => void)[]>();
  on(name: string, f: (e: Record<string, unknown>) => void) {
    this.handlers.set(name, [...(this.handlers.get(name) ?? []), f]);
  }
  off() {}
  dispatch(name: string, data: Record<string, unknown>) {
    this.sent.push([name, data]);
    for (const f of this.handlers.get(name) ?? []) f(data);
  }
}
class FakeController {
  static last: FakeController;
  pageMatches: number[][] = [];
  pageMatchesLength: number[][] = [];
  selected = { pageIdx: -1, matchIdx: -1 };
  highlightMatches = false;
  onIsPageVisible: ((n: number) => boolean) | null = null;
  docs: unknown[] = [];
  scrolled: { element: unknown; pageIndex: number; matchIndex: number }[] = [];
  _pageContents: string[] = [];
  constructor(public opts: { linkService: { page: number; pagesCount: number }; eventBus: FakeBus }) {
    FakeController.last = this;
  }
  setDocument(d: unknown) {
    this.docs.push(d);
  }
  scrollMatchIntoView(a: { element: unknown; pageIndex: number; matchIndex: number }) {
    this.scrolled.push(a);
  }
}
const lib = { EventBus: FakeBus, PDFFindController: FakeController } as never;

const view = (): FindView & { shown: number[]; matches: unknown[] } => {
  const shown: number[] = [];
  const matches: unknown[] = [];
  return {
    shown,
    matches,
    page: () => 4,
    pages: () => 10,
    showPage: (n: number) => void shown.push(n),
    isPageVisible: (n: number) => n === 4,
    showMatch: (el: HTMLElement) => void matches.push(el),
  };
};
const setup = async () => {
  const v = view();
  const f = await createPdfFind(v, lib);
  const c = FakeController.last;
  return { v, f, c, bus: c.opts.eventBus };
};
const finds = (bus: FakeBus) => bus.sent.filter(([n]) => n === "find").map(([, d]) => d);
const doc = (n: number) => ({ numPages: n }) as unknown as PDFDocumentProxy;

/** A text layer of plain objects: each span's text, and what paint put in it. */
type FakeSpan = { children: unknown[]; textContent: string; ownerDocument: { createElement(): FakeSpan }; replaceChildren(...n: unknown[]): void; className?: string };
function span(text: string): FakeSpan {
  const e: FakeSpan = {
    children: [text],
    get textContent() {
      return e.children.map((c) => (typeof c === "string" ? c : (c as FakeSpan).textContent)).join("");
    },
    set textContent(v: string) {
      e.children = [v];
    },
    ownerDocument: { createElement: () => span("") },
    replaceChildren(...n) {
      e.children = n;
    },
  };
  return e;
}
const mapping = (...items: string[]): TextMapping => ({ divs: items.map(span) as unknown as HTMLElement[], items });
const tinted = (m: TextMapping) => m.divs.some((d) => (d as unknown as FakeSpan).children.some((c) => typeof c !== "string"));

describe("createPdfFind", () => {
  it("stands in for pdf.js's link service with the view", async () => {
    const { v, c } = await setup();
    const link = c.opts.linkService;
    expect(link.page).toBe(4);
    expect(link.pagesCount).toBe(10);
    link.page = 7;
    expect(v.shown).toEqual([7]);
    expect(c.onIsPageVisible?.(4)).toBe(true);
    expect(c.onIsPageVisible?.(5)).toBe(false);
  });

  it("search dispatches find with highlightAll and no diacritics", async () => {
    const { f, bus } = await setup();
    f.search({ text: "abc", caseSensitive: true, entireWord: false });
    expect(finds(bus)).toEqual([expect.objectContaining({ type: "", query: "abc", caseSensitive: true, entireWord: false, highlightAll: true, findPrevious: false, matchDiacritics: false })]);
  });

  it("step dispatches again with findPrevious", async () => {
    const { f, bus } = await setup();
    f.search({ text: "abc", caseSensitive: false, entireWord: true });
    f.step(true);
    expect(finds(bus).at(-1)).toEqual(expect.objectContaining({ type: "again", query: "abc", entireWord: true, findPrevious: true, highlightAll: true }));
  });

  it("step does nothing before a search", async () => {
    const { f, bus } = await setup();
    f.step(false);
    expect(finds(bus)).toEqual([]);
  });

  it("a blank query searches nothing and clears the tints", async () => {
    const { f, bus } = await setup();
    const states: string[] = [];
    f.onStatus((s: FindStatus) => states.push(s.state));
    f.search({ text: "   ", caseSensitive: false, entireWord: false });
    expect(finds(bus)).toEqual([]);
    expect(bus.sent.map(([n]) => n)).toContain("findbarclose");
    expect(states.at(-1)).toBe("idle");
  });

  it("a new document runs the last search again", async () => {
    const { f, c, bus } = await setup();
    f.search({ text: "abc", caseSensitive: false, entireWord: false });
    f.setDocument(doc(3));
    expect(c.docs.at(-1)).toEqual(doc(3));
    expect(finds(bus)).toHaveLength(2);
    expect(finds(bus).at(-1)).toEqual(expect.objectContaining({ query: "abc", type: "" }));
  });

  // The file changed on disk with the bar open: the tints come back, and the view stays at the place the person was reading.
  it("a_search_run_again_for_a_new_document_tints_without_scrolling", async () => {
    const { v, f, c, bus } = await setup();
    f.search({ text: "abc", caseSensitive: false, entireWord: false });
    f.setDocument(doc(3));
    const m = mapping("abc");
    f.attach(0, m);
    c.highlightMatches = true;
    c.pageMatches[0] = [0];
    c.pageMatchesLength[0] = [3];
    c.selected = { pageIdx: 0, matchIdx: 0 };
    bus.dispatch("updatetextlayermatches", { pageIndex: 0 });
    expect(tinted(m)).toBe(true);
    expect(c.scrolled).toEqual([]);
    expect(v.matches).toEqual([]);
  });

  // pdf.js also turns to the selected match's page through the link service's `page` (pdf_viewer.mjs, #updatePage).
  it("a_search_run_again_for_a_new_document_does_not_turn_the_page", async () => {
    const { v, f, c } = await setup();
    f.search({ text: "abc", caseSensitive: false, entireWord: false });
    c.opts.linkService.page = 2;
    expect(v.shown).toEqual([2]);
    f.setDocument(doc(3));
    c.opts.linkService.page = 20;
    expect(v.shown).toEqual([2]);
    f.step(false);
    c.opts.linkService.page = 20;
    expect(v.shown).toEqual([2, 20]);
  });

  // pdf.js chose its match from the page in view without showing it, so the first step searches from the view again and shows that one.
  it("the_first_step_after_a_new_document_searches_from_the_view_and_shows_its_match", async () => {
    const { f, c, bus } = await setup();
    f.search({ text: "abc", caseSensitive: false, entireWord: false });
    f.setDocument(doc(3));
    f.step(true);
    expect(finds(bus).at(-1)).toEqual(expect.objectContaining({ type: "", query: "abc", findPrevious: true }));
    const m = mapping("abc");
    f.attach(0, m);
    c.highlightMatches = true;
    c.pageMatches[0] = [0];
    c.pageMatchesLength[0] = [3];
    c.selected = { pageIdx: 0, matchIdx: 0 };
    bus.dispatch("updatetextlayermatches", { pageIndex: 0 });
    expect(c.scrolled).toHaveLength(1);
    f.step(false);
    expect(finds(bus).at(-1)).toEqual(expect.objectContaining({ type: "again" }));
  });

  it("a new document after the bar closed searches nothing", async () => {
    const { f, bus } = await setup();
    f.search({ text: "abc", caseSensitive: false, entireWord: false });
    f.close();
    f.setDocument(doc(3));
    expect(finds(bus)).toHaveLength(1);
  });

  it("paints an attached page when pdf.js says its matches changed, and scrolls to the selected one", async () => {
    const { v, f, c, bus } = await setup();
    const m = mapping("abc abc");
    f.attach(0, m);
    c.highlightMatches = true;
    c.pageMatches[0] = [0, 4];
    c.pageMatchesLength[0] = [3, 3];
    c.selected = { pageIdx: 0, matchIdx: 1 };
    bus.dispatch("updatetextlayermatches", { pageIndex: 0 });
    expect(tinted(m)).toBe(true);
    expect(c.scrolled).toEqual([{ element: expect.anything(), pageIndex: 0, matchIndex: 1 }]);
    // pdf.js scrolls with element.scrollIntoView, which would scroll the pane's clipped ancestors too; the view scrolls instead.
    (c.scrolled[0]!.element as { scrollIntoView(): void }).scrollIntoView();
    expect(v.matches).toHaveLength(1);
    expect((v.matches[0] as { className: string }).className).toBe("pdf__match pdf__match--current");
  });

  it("paints a page as it is attached, and every page on pageIndex -1", async () => {
    const { f, c, bus } = await setup();
    c.highlightMatches = true;
    c.pageMatches[1] = [0];
    c.pageMatchesLength[1] = [1];
    const m = mapping("x");
    f.attach(1, m);
    expect(tinted(m)).toBe(true);
    c.highlightMatches = false;
    bus.dispatch("updatetextlayermatches", { pageIndex: -1 });
    expect(tinted(m)).toBe(false);
  });

  it("a detached page is painted no more", async () => {
    const { f, c, bus } = await setup();
    const m = mapping("x");
    const detach = f.attach(0, m);
    detach();
    c.highlightMatches = true;
    c.pageMatches[0] = [0];
    c.pageMatchesLength[0] = [1];
    bus.dispatch("updatetextlayermatches", { pageIndex: 0 });
    expect(tinted(m)).toBe(false);
  });

  it("close dispatches findbarclose and clears every attached page", async () => {
    const { f, c, bus } = await setup();
    c.highlightMatches = true;
    c.pageMatches[0] = [0];
    c.pageMatchesLength[0] = [1];
    c.pageMatches[1] = [0];
    c.pageMatchesLength[1] = [1];
    const a = mapping("a");
    const b = mapping("b");
    f.attach(0, a);
    f.attach(1, b);
    f.search({ text: "a", caseSensitive: false, entireWord: false });
    f.close();
    expect(bus.sent.map(([n]) => n)).toContain("findbarclose");
    expect(tinted(a) || tinted(b)).toBe(false);
  });

  it("reports pdf.js's state and count, with the selected match's page", async () => {
    const { f, c, bus } = await setup();
    const got: unknown[] = [];
    f.onStatus((s: FindStatus) => got.push(s));
    f.search({ text: "a", caseSensitive: false, entireWord: false });
    c.selected = { pageIdx: 2, matchIdx: 0 };
    bus.dispatch("updatefindcontrolstate", { state: 0, previous: false, matchesCount: { current: 3, total: 7 } });
    expect(got.at(-1)).toEqual({ state: "found", current: 3, total: 7, page: 3, wrapped: null });
    bus.dispatch("updatefindmatchescount", { matchesCount: { current: 3, total: 9 } });
    expect(got.at(-1)).toEqual({ state: "found", current: 3, total: 9, page: 3, wrapped: null });
  });

  it("says a PDF has no text when nothing is found and every page is empty", async () => {
    const { f, c, bus } = await setup();
    const got: { state: string }[] = [];
    f.onStatus((s: FindStatus) => got.push(s));
    f.search({ text: "a", caseSensitive: false, entireWord: false });
    c._pageContents = ["", " ", ""];
    bus.dispatch("updatefindcontrolstate", { state: 1, previous: false, matchesCount: { current: 0, total: 0 } });
    expect(got.at(-1)?.state).toBe("noText");
    c._pageContents = ["", "word"];
    bus.dispatch("updatefindcontrolstate", { state: 1, previous: false, matchesCount: { current: 0, total: 0 } });
    expect(got.at(-1)?.state).toBe("notFound");
  });

  it("ignores pdf.js's state once the bar is closed", async () => {
    const { f, bus } = await setup();
    const got: { state: string }[] = [];
    f.onStatus((s: FindStatus) => got.push(s));
    f.search({ text: "a", caseSensitive: false, entireWord: false });
    f.close();
    bus.dispatch("updatefindcontrolstate", { state: 0, previous: false, matchesCount: { current: 1, total: 1 } });
    expect(got.at(-1)?.state).toBe("idle");
  });

  it("the same document again changes nothing", async () => {
    const { f, c, bus } = await setup();
    const d = doc(3);
    f.setDocument(d);
    f.search({ text: "abc", caseSensitive: false, entireWord: false });
    f.setDocument(d);
    expect(c.docs).toEqual([d]);
    expect(finds(bus)).toHaveLength(1);
  });

  it("destroy lets go of the document", async () => {
    const { f, c } = await setup();
    const spy = vi.spyOn(c, "setDocument");
    f.destroy();
    expect(spy).toHaveBeenCalledWith(null);
  });
});

/** A text layer with real text nodes, as the DOM has them: enough for a range to be placed in. */
type FakeNode = { nodeType: number; textContent: string; childNodes: FakeNode[] };
const text = (s: string): FakeNode => ({ nodeType: 3, textContent: s, childNodes: [] });
const el = (...kids: FakeNode[]): FakeNode => ({ nodeType: 1, textContent: kids.map((k) => k.textContent).join(""), childNodes: kids });
class FakeRange {
  start: [FakeNode, number] | null = null;
  end: [FakeNode, number] | null = null;
  setStart(n: FakeNode, o: number) {
    this.start = [n, o];
  }
  setEnd(n: FakeNode, o: number) {
    this.end = [n, o];
  }
}

describe("currentRange", () => {
  const setupRange = async (divs: FakeNode[], items: string[], match: [number, number], selected = { pageIdx: 0, matchIdx: 0 }) => {
    const { f, c } = await setup();
    c.selected = selected;
    c.pageMatches = [[match[0]]];
    c.pageMatchesLength = [[match[1]]];
    f.attach(0, { divs: divs as unknown as HTMLElement[], items });
    return f;
  };
  const make = () => new FakeRange() as unknown as Range;

  it("currentRange_in_one_span", async () => {
    const t = text("hello world");
    const f = await setupRange([el(t)], ["hello world"], [6, 5]);
    const r = f.currentRange(make) as unknown as FakeRange;
    expect(r.start).toEqual([t, 6]);
    expect(r.end).toEqual([t, 11]);
  });
  it("currentRange_across_two_spans", async () => {
    const a = text("hello ");
    const b = text("world");
    const f = await setupRange([el(a), el(b)], ["hello ", "world"], [3, 6]);
    const r = f.currentRange(make) as unknown as FakeRange;
    expect(r.start).toEqual([a, 3]);
    expect(r.end).toEqual([b, 3]);
  });
  it("currentRange_goes_through_a_tint", async () => {
    const a = text("he");
    const m = text("llo");
    const b = text(" world");
    const f = await setupRange([el(a, el(m), b)], ["hello world"], [3, 5]);
    const r = f.currentRange(make) as unknown as FakeRange;
    expect(r.start).toEqual([m, 1]);
    expect(r.end).toEqual([b, 3]);
  });
  it("currentRange_is_null_when_its_page_is_not_drawn", async () => {
    const f = await setupRange([el(text("abc"))], ["abc"], [0, 2], { pageIdx: 4, matchIdx: 0 });
    expect(f.currentRange(make)).toBeNull();
  });
  it("currentRange_is_null_with_no_match", async () => {
    const f = await setupRange([el(text("abc"))], ["abc"], [0, 2], { pageIdx: -1, matchIdx: -1 });
    expect(f.currentRange(make)).toBeNull();
  });
});
