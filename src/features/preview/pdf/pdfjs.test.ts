import type { PDFPageProxy } from "pdfjs-dist";
import { describe, expect, it, vi } from "vitest";
import { canvasScale, MAX_CANVAS_PIXELS, MAX_CANVAS_SIDE, pdfErrorMessage, renderTextLayer, viewerLib } from "./pdfjs";

const fake = vi.hoisted(() => {
  const layers: { options: unknown; textDivs: unknown[]; textContentItemsStr: string[] }[] = [];
  class TextLayer {
    textDivs = [{ id: "a" }, { id: "b" }];
    textContentItemsStr = ["Hello ", "world"];
    constructor(public options: unknown) {
      layers.push(this);
    }
    render() {
      return Promise.resolve();
    }
    cancel() {}
  }
  return { layers, lib: { TextLayer, GlobalWorkerOptions: { workerSrc: "" } } };
});
vi.mock("pdfjs-dist", () => fake.lib);
vi.mock("pdfjs-dist/build/pdf.worker.min.mjs?url", () => ({ default: "/worker.mjs" }));
// The viewer reads globalThis.pdfjsLib as it loads; this stand-in keeps what it saw then.
vi.mock("pdfjs-dist/web/pdf_viewer.mjs", () => ({ sawLib: (globalThis as { pdfjsLib?: unknown }).pdfjsLib }));

const page = () => {
  const streamTextContent = vi.fn(() => "stream");
  return { page: { streamTextContent, getViewport: () => ({ width: 1, height: 1 }) } as unknown as PDFPageProxy, streamTextContent };
};
type Listener = (e?: unknown) => void;
/** A document that keeps its listeners, and a window under it, for the layer's selection marks. */
const fakeDoc = () => {
  const on: Record<string, Listener[]> = {};
  const win: Record<string, Listener[]> = {};
  const add = (to: Record<string, Listener[]>) => (type: string, fn: Listener) => void (to[type] ??= []).push(fn);
  return {
    on,
    win,
    addEventListener: add(on),
    defaultView: { addEventListener: add(win) },
    createElement: () => ({ className: "" }),
    fire: (type: string) => on[type]?.forEach((fn) => fn()),
    blur: () => win.blur?.forEach((fn) => fn()),
  };
};
const docs = [fakeDoc()];
/** A layer's container: its children, its classes and its listeners. */
const container = (doc = docs[0]!) => {
  const children: { className: string }[] = [];
  const classes = new Set<string>();
  const on: Record<string, Listener[]> = {};
  const el = {
    style: { setProperty() {} },
    ownerDocument: doc,
    children,
    append: (c: { className: string }) => void children.push(c),
    classList: { add: (c: string) => void classes.add(c), remove: (c: string) => void classes.delete(c), contains: (c: string) => classes.has(c) },
    addEventListener: (type: string, fn: Listener) => void (on[type] ??= []).push(fn),
    press: () => on.mousedown?.forEach((fn) => fn()),
  };
  return el as unknown as HTMLElement & { children: { className: string }[]; press(): void };
};

describe("renderTextLayer", () => {
  it("streams the text without normalization and with marked content", async () => {
    const { page: p, streamTextContent } = page();
    await renderTextLayer(p, container(), 1);
    expect(streamTextContent).toHaveBeenCalledWith({ includeMarkedContent: true, disableNormalization: true });
  });

  it("hands back the layer's spans and strings", async () => {
    const m = await renderTextLayer(page().page, container(), 1);
    const layer = fake.layers.at(-1)!;
    expect(m).toEqual({ divs: layer.textDivs, items: layer.textContentItemsStr });
    expect(m!.divs).toBe(layer.textDivs);
  });

  // pdf.js's own viewer ends each text layer with this block. Without it a drag let go in the blank past a line's end
  // landed on the layer itself, and the selection ran on to the page's last lines: a highlight made from it took
  // fourteen lines for three (live check 6q-6).
  it("ends the layer with a block for a drag let go past the text", async () => {
    const c = container();
    await renderTextLayer(page().page, c, 1);
    expect(c.children.map((k) => k.className)).toEqual(["endOfContent"]);
  });

  it("marks the layer as being selected from a press in it until the pointer comes up", async () => {
    const doc = fakeDoc();
    const a = container(doc);
    const b = container(doc);
    await renderTextLayer(page().page, a, 1);
    await renderTextLayer(page().page, b, 1);
    a.press();
    expect(a.classList.contains("selecting")).toBe(true);
    expect(b.classList.contains("selecting")).toBe(false);
    doc.fire("pointerup");
    expect(a.classList.contains("selecting")).toBe(false);
    // One listener per document, not one per page.
    expect(doc.on.pointerup).toHaveLength(1);
  });

  it("drops the mark when the window loses focus mid-drag", async () => {
    const doc = fakeDoc();
    const a = container(doc);
    await renderTextLayer(page().page, a, 1);
    a.press();
    expect(a.classList.contains("selecting")).toBe(true);
    doc.blur();
    expect(a.classList.contains("selecting")).toBe(false);
  });

  it("adds no block to a layer whose drawing was cancelled", async () => {
    const abort = new AbortController();
    abort.abort();
    const c = container();
    await renderTextLayer(page().page, c, 1, abort.signal);
    expect(c.children).toEqual([]);
  });

  it("is null when it was cancelled", async () => {
    const abort = new AbortController();
    abort.abort();
    const render = vi.spyOn(fake.lib.TextLayer.prototype, "render").mockRejectedValueOnce(Object.assign(new Error("x"), { name: "AbortException" }));
    expect(await renderTextLayer(page().page, container(), 1, abort.signal)).toBeNull();
    render.mockRestore();
  });
});

describe("viewerLib", () => {
  it("sets pdfjsLib before it loads the viewer, and loads it once", async () => {
    const a = (await viewerLib()) as unknown as { sawLib: unknown };
    expect(a.sawLib).toBeDefined();
    expect((a.sawLib as { TextLayer: unknown }).TextLayer).toBe(fake.lib.TextLayer);
    expect(await viewerLib()).toBe(a);
  });
});

describe("pdfErrorMessage", () => {
  it("says a locked PDF is password-protected and any other failure can't be shown", () => {
    expect(pdfErrorMessage({ name: "PasswordException" })).toBe("This PDF is password-protected.");
    expect(pdfErrorMessage({ name: "InvalidPDFException" })).toBe("This PDF can't be shown.");
    expect(pdfErrorMessage(new Error("x"))).toBe("This PDF can't be shown.");
  });
});

describe("canvasScale", () => {
  it("is scale x dpr while the canvas stays small", () => {
    expect(canvasScale(612, 792, 1.5, 2)).toBe(3);
  });

  it("is clamped so the canvas has at most 16 777 216 pixels", () => {
    const f = canvasScale(612, 792, 3, 3);
    expect(f).toBeLessThan(9);
    expect(Math.floor(612 * f) * Math.floor(792 * f)).toBeLessThanOrEqual(MAX_CANVAS_PIXELS);
    expect(MAX_CANVAS_PIXELS).toBe(16_777_216);
  });

  it("is clamped so no side passes 32767 pixels", () => {
    const f = canvasScale(10, 14400, 3, 3);
    expect(Math.floor(14400 * f)).toBeLessThanOrEqual(MAX_CANVAS_SIDE);
    expect(Math.floor(10 * f) * Math.floor(14400 * f)).toBeLessThanOrEqual(MAX_CANVAS_PIXELS);
  });

  it("falls back to scale x dpr for a degenerate page", () => {
    expect(canvasScale(0, 0, 2, 2)).toBe(4);
  });
});
