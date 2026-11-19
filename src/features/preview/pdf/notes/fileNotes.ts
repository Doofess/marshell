import type { PDFDocumentProxy } from "pdfjs-dist";
import type { Note } from "./editor";

/**
 * The markup notes a PDF file already holds, read from its pages' annotations: what the Notes view lists besides the editor's
 * own adds, edits and deletions. Each page is read once per document.
 */

/** A note's text for a highlight is what lies under it, at most this many characters. */
const TEXT_MAX = 200;

const NAMES: Readonly<Record<string, string>> = {
  Highlight: "Highlight",
  FreeText: "Free text",
  Text: "Sticky note",
  Square: "Square",
  Circle: "Circle",
  Underline: "Underline",
  StrikeOut: "Strikeout",
  Squiggly: "Squiggly underline",
  Ink: "Drawing",
  Stamp: "Stamp",
  Line: "Line",
  Polygon: "Polygon",
  PolyLine: "Polyline",
  Caret: "Caret",
  FileAttachment: "Attachment",
};

/** Annotations that are no notes: a popup belongs to its note, a link and a form field to the page. */
const NOT_NOTES = new Set(["Popup", "Link", "Widget"]);

/** A sentence-case name for an annotation subtype; one with no name here keeps its own. */
export function subtypeName(subtype: string): string {
  return NAMES[subtype] ?? subtype;
}

/** A box as [x1, y1, x2, y2] with x1 <= x2 and y1 <= y2, in the page's own units. */
export type Box = [number, number, number, number];

const isPoint = (p: unknown): p is { x: number; y: number } => typeof p === "object" && p !== null && typeof (p as { x?: unknown }).x === "number" && typeof (p as { y?: unknown }).y === "number";

/** The boxes round a highlight's quads: pdf.js gives the points flat, eight numbers a quad, or as lists of {x, y} points. */
export function quadBoxes(quadPoints: unknown): Box[] {
  if (!quadPoints || typeof quadPoints !== "object" || !("length" in quadPoints)) return [];
  const list = Array.from(quadPoints as ArrayLike<unknown>);
  const boxes: Box[] = [];
  const add = (xs: number[], ys: number[]) => {
    if (xs.length > 0) boxes.push([Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)]);
  };
  if (list.every((v) => typeof v === "number")) {
    for (let i = 0; i + 7 < list.length; i += 8) {
      const q = (list as number[]).slice(i, i + 8);
      add([q[0]!, q[2]!, q[4]!, q[6]!], [q[1]!, q[3]!, q[5]!, q[7]!]);
    }
    return boxes;
  }
  for (const quad of list) {
    const points = Array.from((quad ?? []) as ArrayLike<unknown>).filter(isPoint);
    add(
      points.map((p) => p.x),
      points.map((p) => p.y),
    );
  }
  return boxes;
}

export function inBoxes(x: number, y: number, boxes: readonly Box[]): boolean {
  return boxes.some((b) => x >= b[0] && x <= b[2] && y >= b[1] && y <= b[3]);
}

export type TextItem = { str?: string; transform?: number[]; width?: number; height?: number };

/** The page's text under the boxes: the words whose middle lies in one, in reading order, joined and trimmed. */
export function textUnder(items: readonly TextItem[], boxes: readonly Box[]): string {
  if (boxes.length === 0) return "";
  const picked: string[] = [];
  for (const it of items) {
    const t = it.transform;
    if (typeof it.str !== "string" || it.str === "" || !t || t.length < 6) continue;
    // A word's middle, placed along the item's own direction from its share of the characters (one item can be a whole line).
    const scale = Math.hypot(t[0]!, t[1]!);
    const [dx, dy] = scale > 0 ? [t[0]! / scale, t[1]! / scale] : [1, 0];
    const width = it.width ?? 0;
    const up = (it.height ?? 0) / 2;
    for (const m of it.str.matchAll(/\S+/g)) {
      const along = ((m.index + m[0].length / 2) / it.str.length) * width;
      if (inBoxes(t[4]! + dx * along - dy * up, t[5]! + dy * along + dx * up, boxes)) picked.push(m[0]);
    }
  }
  return picked.join(" ").replace(/\s+/g, " ").trim().slice(0, TEXT_MAX).trim();
}

/** A colour as `#RRGGBB`, or null when the annotation has none (or not three components). */
export function hexOf(color: ArrayLike<number> | null | undefined): string | null {
  if (!color || color.length < 3) return null;
  return `#${[0, 1, 2].map((i) => Math.max(0, Math.min(255, Math.round(color[i]!))).toString(16).padStart(2, "0")).join("")}`.toUpperCase();
}

type Annotation = {
  id?: string;
  subtype?: string;
  rect?: number[];
  quadPoints?: unknown;
  color?: ArrayLike<number> | null;
  contentsObj?: { str?: string } | null;
  contents?: string;
};

const cache = new WeakMap<PDFDocumentProxy, Map<number, Promise<Note[]>>>();
const texts = new WeakMap<PDFDocumentProxy, Map<number, Promise<TextItem[]>>>();

/** A page's text items, read once per document: what lies under a highlight is looked up in them. */
export function pageText(doc: PDFDocumentProxy, n: number): Promise<TextItem[]> {
  let byPage = texts.get(doc);
  if (!byPage) texts.set(doc, (byPage = new Map()));
  let read = byPage.get(n);
  if (!read) {
    const made = doc
      .getPage(n)
      .then((page) => page.getTextContent())
      .then((c) => ((c as { items?: TextItem[] }).items ?? []));
    byPage.set(n, made);
    made.catch(() => byPage!.delete(n));
    read = made;
  }
  return read;
}

/**
 * Every markup note in the file, in page order, at most `limit`. A highlight's text is the page's text under its quads; a
 * free text's text is its contents; a comment is a highlight's or another note's contents. Only a highlight and a free text
 * are `editable`. The result is kept for the document, so a page's annotations are read once.
 */
export function readFileNotes(doc: PDFDocumentProxy, limit = 2000): Promise<Note[]> {
  let byLimit = cache.get(doc);
  if (!byLimit) cache.set(doc, (byLimit = new Map()));
  let read = byLimit.get(limit);
  if (!read) {
    const made = readAll(doc, limit);
    byLimit.set(limit, made);
    // A failed read is tried again next time.
    made.catch(() => byLimit!.delete(limit));
    read = made;
  }
  return read;
}

async function readAll(doc: PDFDocumentProxy, limit: number): Promise<Note[]> {
  const out: Note[] = [];
  for (let n = 1; n <= doc.numPages && out.length < limit; n++) {
    const page = await doc.getPage(n);
    const annotations = ((await page.getAnnotations()) as Annotation[]).filter((a) => typeof a.subtype === "string" && !NOT_NOTES.has(a.subtype));
    if (annotations.length === 0) continue;
    const viewport = page.getViewport({ scale: 1 });
    const items = annotations.some((a) => a.subtype === "Highlight") ? await pageText(doc, n) : [];
    for (const a of annotations) {
      if (out.length >= limit) break;
      const subtype = a.subtype!;
      const kind = subtype === "Highlight" ? "highlight" : subtype === "FreeText" ? "freetext" : "other";
      const contents = a.contentsObj?.str ?? a.contents ?? "";
      let top = 0;
      if (a.rect && a.rect.length >= 4) {
        const from = viewport.convertToViewportPoint(a.rect[0]!, a.rect[1]!);
        const to = viewport.convertToViewportPoint(a.rect[2]!, a.rect[3]!);
        top = viewport.height > 0 ? Math.min(1, Math.max(0, Math.min(from[1], to[1]) / viewport.height)) : 0;
      }
      out.push({
        id: String(a.id ?? `${n}-${out.length}`),
        page: n,
        kind,
        kindName: subtypeName(subtype),
        color: hexOf(a.color),
        text: kind === "freetext" ? contents : kind === "highlight" ? textUnder(items, quadBoxes(a.quadPoints)) : "",
        comment: kind === "freetext" ? "" : contents,
        editable: kind !== "other",
        top,
      });
    }
  }
  return out;
}
