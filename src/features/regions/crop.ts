import type { PDFDocumentProxy } from "pdfjs-dist";
import type { Region } from "./region";

type Size = { w: number; h: number };

const MAX_PIXELS = 16_777_216;
const MAX_SCALE = 3;
const MAX_SIDE = 4096;

/** How far to render a page: sharp (up to 3x), but never more than 16 Mpx for the whole page. `page1` is its size at scale 1. */
export function cropScale(page1: Size): number {
  return Math.min(MAX_SCALE, Math.sqrt(MAX_PIXELS / (page1.w * page1.h)));
}

/** The region in whole source pixels, at least 1 each way and inside the source. */
export function cropRect(r: Region, src: Size): { sx: number; sy: number; sw: number; sh: number } {
  const sx = Math.min(src.w - 1, Math.max(0, Math.round(r.x * src.w)));
  const sy = Math.min(src.h - 1, Math.max(0, Math.round(r.y * src.h)));
  const sw = Math.min(src.w - sx, Math.max(1, Math.round(r.w * src.w)));
  const sh = Math.min(src.h - sy, Math.max(1, Math.round(r.h * src.h)));
  return { sx, sy, sw, sh };
}

/** The crop's size in the PNG: the longest side at most 4096, the ratio kept. */
export function outputSize(sw: number, sh: number): Size {
  const k = Math.min(1, MAX_SIDE / Math.max(sw, sh));
  return { w: Math.max(1, Math.round(sw * k)), h: Math.max(1, Math.round(sh * k)) };
}

function makeCanvas(w: number, h: number): HTMLCanvasElement | OffscreenCanvas {
  if (typeof OffscreenCanvas !== "undefined") return new OffscreenCanvas(w, h);
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  return c;
}

function release(c: HTMLCanvasElement | OffscreenCanvas) {
  c.width = 0;
  c.height = 0;
}

function toBlob(c: HTMLCanvasElement | OffscreenCanvas): Promise<Blob> {
  if ("convertToBlob" in c) return c.convertToBlob({ type: "image/png" });
  return new Promise((resolve, reject) => c.toBlob((b) => (b ? resolve(b) : reject(new Error("The crop could not be encoded."))), "image/png"));
}

/** Crops `r` out of a source `src` pixels large and encodes it as a PNG. */
export async function cropToPng(source: CanvasImageSource, src: Size, r: Region): Promise<Blob> {
  const { sx, sy, sw, sh } = cropRect(r, src);
  const out = outputSize(sw, sh);
  const canvas = makeCanvas(out.w, out.h);
  try {
    const ctx = canvas.getContext("2d") as CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D | null;
    if (!ctx) throw new Error("The crop could not be drawn.");
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(source, sx, sy, sw, sh, 0, 0, out.w, out.h);
    return await toBlob(canvas);
  } finally {
    release(canvas);
  }
}

/** Crops `r` out of the image at `url`, at its natural size. */
export async function cropImage(url: string, r: Region): Promise<Blob> {
  const img = new Image();
  img.src = url;
  await img.decode();
  return cropToPng(img, { w: img.naturalWidth, h: img.naturalHeight }, r);
}

/** Renders the region's page at `cropScale` into an offscreen canvas, then crops it. */
export async function cropPdfPage(doc: PDFDocumentProxy, r: Region): Promise<Blob> {
  const page = await doc.getPage(r.page);
  const base = page.getViewport({ scale: 1 });
  const viewport = page.getViewport({ scale: cropScale({ w: base.width, h: base.height }) });
  const w = Math.max(1, Math.floor(viewport.width));
  const h = Math.max(1, Math.floor(viewport.height));
  const canvas = makeCanvas(w, h);
  try {
    // pdf.js types want an HTMLCanvasElement; an OffscreenCanvas draws the same.
    await page.render({ canvas: canvas as HTMLCanvasElement, viewport }).promise;
    return await cropToPng(canvas, { w, h }, r);
  } finally {
    release(canvas);
    page.cleanup();
  }
}
