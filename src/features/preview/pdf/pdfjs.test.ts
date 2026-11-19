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
const container = () => ({ style: { setProperty() {} } }) as unknown as HTMLElement;

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
