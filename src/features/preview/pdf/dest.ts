import type { PDFDocumentProxy } from "pdfjs-dist";
import type { Place } from "../../pane/paneState";

export type Dest = string | readonly unknown[];

const clamp01 = (n: number) => Math.min(Math.max(n, 0), 1);

/**
 * How far down the page a destination points, as a fraction of its height. `explicit` is the destination without its page:
 * the kind first (`{ name: "XYZ" }`), then its arguments. XYZ takes the second argument (left, top, zoom), FitH and FitBH the
 * first. Anything else, a missing top, or a page with no height gives the top of the page.
 */
export function topOf(explicit: readonly unknown[], toViewY: (y: number) => number, height: number): number {
  const kind = (explicit[0] as { name?: unknown } | null | undefined)?.name;
  const y = kind === "XYZ" ? explicit[2] : kind === "FitH" || kind === "FitBH" ? explicit[1] : null;
  if (typeof y !== "number" || !Number.isFinite(y) || !(height > 0)) return 0;
  return clamp01(toViewY(y) / height);
}

/** Where a destination is in the file, or null when it names nothing the file has. */
export async function placeOfDest(doc: PDFDocumentProxy, dest: Dest): Promise<Place | null> {
  try {
    const explicit = typeof dest === "string" ? await doc.getDestination(dest) : dest;
    if (!Array.isArray(explicit) || explicit.length === 0) return null;
    const first: unknown = explicit[0];
    let index: number;
    if (typeof first === "number") index = first;
    else if (first !== null && typeof first === "object") index = await doc.getPageIndex(first as Parameters<PDFDocumentProxy["getPageIndex"]>[0]);
    else return null;
    if (!Number.isInteger(index) || index < 0 || index >= doc.numPages) return null;
    const viewport = (await doc.getPage(index + 1)).getViewport({ scale: 1 });
    const toViewY = (y: number) => viewport.convertToViewportPoint(0, y)[1] ?? 0;
    return { page: index + 1, top: topOf(explicit.slice(1), toViewY, viewport.height) };
  } catch {
    return null;
  }
}
