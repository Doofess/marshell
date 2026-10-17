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

/** Draws a page into the canvas at `scale`: sharp on a dense screen (the canvas has `scale × devicePixelRatio` pixels), laid out at `scale`. */
export async function renderPage(page: PDFPageProxy, canvas: HTMLCanvasElement, scale: number, signal?: AbortSignal): Promise<void> {
  const dpr = window.devicePixelRatio || 1;
  const viewport = page.getViewport({ scale: scale * dpr });
  canvas.width = Math.floor(viewport.width);
  canvas.height = Math.floor(viewport.height);
  canvas.style.inlineSize = `${(viewport.width / dpr).toFixed(2)}px`;
  canvas.style.blockSize = `${(viewport.height / dpr).toFixed(2)}px`;
  const task = page.render({ canvas, viewport });
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

/** The page's text as transparent, selectable spans over the canvas. */
export async function renderTextLayer(page: PDFPageProxy, container: HTMLElement, scale: number, signal?: AbortSignal): Promise<void> {
  const pdfjs = await lib();
  container.style.setProperty("--total-scale-factor", String(scale));
  const layer = new pdfjs.TextLayer({ textContentSource: page.streamTextContent(), container, viewport: page.getViewport({ scale }) });
  const stop = () => layer.cancel();
  signal?.addEventListener("abort", stop);
  try {
    await layer.render();
  } catch (e) {
    if (!cancelled(e)) throw e;
  } finally {
    signal?.removeEventListener("abort", stop);
  }
}

/** What the viewer says when a PDF won't open. */
export function pdfErrorMessage(e: unknown): string {
  return (e as { name?: string } | null)?.name === "PasswordException" ? "This PDF is password-protected." : "This PDF can't be shown.";
}
