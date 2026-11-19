import type { PDFDocumentProxy, PDFPageProxy } from "pdfjs-dist";

/**
 * pdf.js, loaded on first use: importing this file loads neither the library nor its worker, so the main bundle and the
 * tests stay free of them. The worker comes from the app's own origin (the CSP allows no other), and the data pdf.js
 * fetches (character maps, standard fonts) is served from /pdfjs/ by the pdfjsAssets plugin in vite.config.ts.
 * WebAssembly and eval are switched off: the CSP has neither 'wasm-unsafe-eval' nor 'unsafe-eval'.
 */
let ready: Promise<typeof import("pdfjs-dist")> | null = null;
function lib() {
  ready ??= (async () => {
    const [pdfjs, worker] = await Promise.all([import("pdfjs-dist"), import("pdfjs-dist/build/pdf.worker.min.mjs?url")]);
    pdfjs.GlobalWorkerOptions.workerSrc = worker.default;
    return pdfjs;
  })();
  return ready;
}

export async function loadPdf(bytes: ArrayBuffer): Promise<PDFDocumentProxy> {
  const pdfjs = await lib();
  // pdf.js takes over the buffer it is given; a copy keeps the caller's bytes whole for a reload or a crop.
  return pdfjs.getDocument({
    data: new Uint8Array(bytes.slice(0)),
    cMapUrl: "/pdfjs/cmaps/",
    cMapPacked: true,
    standardFontDataUrl: "/pdfjs/standard_fonts/",
    wasmUrl: "/pdfjs/wasm/",
    useWasm: false,
  }).promise;
}

const cancelled = (e: unknown) => {
  const name = (e as { name?: string } | null)?.name;
  return name === "RenderingCancelledException" || name === "AbortException";
};

export const MAX_CANVAS_PIXELS = 16_777_216;
export const MAX_CANVAS_SIDE = 32_767;

/**
 * The device-pixel factor a page is rasterised at: `scale × dpr`, lowered when the canvas would pass 16 777 216 pixels
 * or 32 767 on a side (a canvas past either renders blank or costs the memory of the whole view). `pageW1`/`pageH1` are
 * the page's size at scale 1.
 */
export function canvasScale(pageW1: number, pageH1: number, scale: number, dpr: number): number {
  const wanted = scale * dpr;
  if (!(pageW1 > 0 && pageH1 > 0)) return wanted;
  const byArea = Math.sqrt(MAX_CANVAS_PIXELS / (pageW1 * pageH1));
  const bySide = MAX_CANVAS_SIDE / Math.max(pageW1, pageH1);
  return Math.min(wanted, byArea, bySide);
}

/**
 * Draws a page into the canvas at `scale`: sharp on a dense screen (the canvas has `scale × devicePixelRatio` pixels,
 * fewer when that would pass the canvas limits, and is then stretched), laid out at `scale`. With `isEditing` the notes
 * the editor can change (highlights, free text) are left off the canvas: the editor draws them.
 */
export async function renderPage(page: PDFPageProxy, canvas: HTMLCanvasElement, scale: number, signal?: AbortSignal, isEditing = false): Promise<void> {
  const dpr = window.devicePixelRatio || 1;
  const base = page.getViewport({ scale: 1 });
  const viewport = page.getViewport({ scale: canvasScale(base.width, base.height, scale, dpr) });
  canvas.width = Math.floor(viewport.width);
  canvas.height = Math.floor(viewport.height);
  canvas.style.inlineSize = `${(base.width * scale).toFixed(2)}px`;
  canvas.style.blockSize = `${(base.height * scale).toFixed(2)}px`;
  const task = page.render({ canvas, viewport, isEditing });
  const stop = () => task.cancel();
  signal?.addEventListener("abort", stop);
  try {
    await task.promise;
  } catch (e) {
    if (!cancelled(e)) throw e;
  } finally {
    signal?.removeEventListener("abort", stop);
  }
}

/**
 * pdf.js's viewer components (the find controller and its event bus), loaded on first use. `pdf_viewer.mjs` reads the
 * library from `globalThis.pdfjsLib` as it loads, so that is set first, to the same copy the pages are drawn with.
 */
let viewer: Promise<typeof import("pdfjs-dist/web/pdf_viewer.mjs")> | null = null;
export function viewerLib(): Promise<typeof import("pdfjs-dist/web/pdf_viewer.mjs")> {
  viewer ??= (async () => {
    (globalThis as { pdfjsLib?: unknown }).pdfjsLib = await lib();
    return import("pdfjs-dist/web/pdf_viewer.mjs");
  })();
  return viewer;
}

/** pdf.js's note editor: the library's editor classes, and the viewer's event bus and built-in English strings. */
export type EditorLib = Pick<
  typeof import("pdfjs-dist"),
  "AnnotationEditorUIManager" | "AnnotationEditorLayer" | "AnnotationLayer" | "DrawLayer" | "AnnotationEditorType" | "AnnotationEditorParamsType" | "PixelsPerInch"
> &
  Pick<typeof import("pdfjs-dist/web/pdf_viewer.mjs"), "EventBus" | "GenericL10n">;

/** The note editor's pieces, loaded on first use from the same copies of pdf.js the pages are drawn with. */
let editing: Promise<EditorLib> | null = null;
export function editorLib(): Promise<EditorLib> {
  editing ??= (async () => {
    const [pdfjs, web] = await Promise.all([lib(), viewerLib()]);
    const { AnnotationEditorUIManager, AnnotationEditorLayer, AnnotationLayer, DrawLayer, AnnotationEditorType, AnnotationEditorParamsType, PixelsPerInch } = pdfjs;
    return { AnnotationEditorUIManager, AnnotationEditorLayer, AnnotationLayer, DrawLayer, AnnotationEditorType, AnnotationEditorParamsType, PixelsPerInch, EventBus: web.EventBus, GenericL10n: web.GenericL10n };
  })();
  return editing;
}

/**
 * A drawn text layer's spans and the strings they hold, in content order: what find needs to tint a match on the
 * letters it was found in. A string can be empty (marked content), and its span then holds no text.
 */
export type TextMapping = { divs: HTMLElement[]; items: string[] };

/**
 * The page's text as transparent, selectable spans over the canvas. It is built as pdf.js's own viewer builds it, with
 * marked content and without normalization, so a match the find controller reports lands on the same characters here.
 * Null when it was cancelled.
 */
export async function renderTextLayer(page: PDFPageProxy, container: HTMLElement, scale: number, signal?: AbortSignal): Promise<TextMapping | null> {
  const pdfjs = await lib();
  container.style.setProperty("--total-scale-factor", String(scale));
  const textContentSource = page.streamTextContent({ includeMarkedContent: true, disableNormalization: true });
  const layer = new pdfjs.TextLayer({ textContentSource, container, viewport: page.getViewport({ scale }) });
  const stop = () => layer.cancel();
  signal?.addEventListener("abort", stop);
  try {
    await layer.render();
  } catch (e) {
    if (!cancelled(e)) throw e;
    return null;
  } finally {
    signal?.removeEventListener("abort", stop);
  }
  return signal?.aborted ? null : { divs: layer.textDivs, items: layer.textContentItemsStr };
}

/** What the viewer says when a PDF won't open. */
export function pdfErrorMessage(e: unknown): string {
  return (e as { name?: string } | null)?.name === "PasswordException" ? "This PDF is password-protected." : "This PDF can't be shown.";
}
