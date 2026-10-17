import type { ILink, ILinkProvider } from "@xterm/xterm";
import type { Endpoint } from "../../generated/Endpoint";
import { findPathCandidates, type PathCandidate } from "../preview/pathCandidates";
import type { OpenPreview } from "../preview/PreviewContext";
import { resolvePaths } from "../preview/previewApi";

/** The part of xterm's `IBuffer` that `logicalLine` reads. The real buffer fits it. */
export type BufferLike = {
  getLine(y: number): { readonly isWrapped: boolean; readonly length: number; getCell(x: number): { getChars(): string; getWidth(): number } | undefined } | undefined;
};

/** A cell as xterm's `IBufferRange` wants it: `x` and `y` are 1-based. `width` is 2 for a wide character. */
export type LogicalCell = { x: number; y: number; width: number };

/**
 * The text of a buffer row together with the rows it wraps onto (and the ones it continues), and the cell each UTF-16 index of
 * that text came from. `y` is a 0-based buffer row, as `IBuffer.getLine` takes it.
 */
export function logicalLine(buffer: BufferLike, y: number): { text: string; cellOf(index: number): LogicalCell } {
  let first = y;
  while (buffer.getLine(first)?.isWrapped && first > 0) first--;
  let text = "";
  const cells: LogicalCell[] = [];
  for (let row = first; ; row++) {
    const line = buffer.getLine(row);
    if (!line || (row > first && !line.isWrapped)) break;
    for (let x = 0; x < line.length; x++) {
      const cell = line.getCell(x);
      if (!cell) break;
      const width = cell.getWidth();
      // The second half of a wide character has no text of its own.
      if (width === 0) continue;
      const chars = cell.getChars() || " ";
      for (let i = 0; i < chars.length; i++) cells.push({ x: x + 1, y: row + 1, width });
      text += chars;
    }
  }
  return { text, cellOf: (index) => cells[Math.max(0, Math.min(index, cells.length - 1))] ?? { x: 1, y: first + 1, width: 1 } };
}

/** The tooltip over a clickable path. */
export const previewHint = (platform: Endpoint["platform"]): string => (platform === "macos" ? "⌘+click to preview" : "Ctrl+click to preview");

/** The core refuses more candidates than this per resolve. */
const MAX_BATCH = 64;
/** A resolved path is remembered this long: the folder can change under it. */
const TTL_MS = 5000;

type Resolver = (tab: string, candidates: string[]) => Promise<Array<string | null>>;

/**
 * Makes the file paths in a terminal's output clickable. Only a candidate that names an existing file in the session's
 * project (the core decides) becomes a link, and only Ctrl+click (Cmd+click on macOS) opens it: a plain click is left to
 * selection and to a program that reads the mouse.
 */
export function pathLinkProvider(
  term: { buffer: { active: BufferLike }; element?: HTMLElement | undefined },
  tab: string,
  platform: Endpoint["platform"],
  openPreview: OpenPreview,
  resolve: Resolver = resolvePaths,
  now: () => number = Date.now,
): ILinkProvider {
  const cache = new Map<string, { at: number; path: Promise<string | null> }>();
  const mac = platform === "macos";

  /** The resolved path for each candidate, asking the core once for all the ones not asked about in the last 5 s. */
  const resolveAll = async (texts: string[]): Promise<Array<string | null>> => {
    const t = now();
    for (const [k, v] of cache) if (t - v.at >= TTL_MS) cache.delete(k);
    const fresh = [...new Set(texts)].filter((c) => !cache.has(c)).slice(0, MAX_BATCH);
    if (fresh.length > 0) {
      const batch = resolve(tab, fresh);
      fresh.forEach((c, i) => {
        const path = batch.then((paths) => paths[i] ?? null);
        cache.set(c, { at: t, path });
        // A failed request is not remembered; the next hover asks again.
        path.catch(() => cache.delete(c));
      });
    }
    return Promise.all(texts.map((c) => cache.get(c)?.path ?? null));
  };

  return {
    provideLinks(bufferLineNumber, callback) {
      const y = bufferLineNumber - 1;
      const ll = logicalLine(term.buffer.active, y);
      const found = findPathCandidates(ll.text).filter((c) => {
        const from = ll.cellOf(c.start).y;
        const to = ll.cellOf(c.end - 1).y;
        return from <= bufferLineNumber && bufferLineNumber <= to;
      });
      if (found.length === 0) return callback(undefined);
      const link = (c: PathCandidate, path: string): ILink => {
        const start = ll.cellOf(c.start);
        const last = ll.cellOf(c.end - 1);
        return {
          range: { start: { x: start.x, y: start.y }, end: { x: last.x + last.width - 1, y: last.y } },
          text: c.text,
          // Underlined on hover, with a pointer; the tooltip says it takes the modifier.
          decorations: { underline: true, pointerCursor: true },
          activate(event) {
            if (!(mac ? event.metaKey : event.ctrlKey)) return;
            openPreview({ path, ...(c.line !== undefined && { line: c.line }), ...(c.col !== undefined && { col: c.col }) });
          },
          hover: () => term.element?.setAttribute("title", previewHint(platform)),
          leave: () => term.element?.removeAttribute("title"),
        };
      };
      resolveAll(found.map((c) => c.text)).then(
        (paths) => {
          const links = found.flatMap((c, i) => (paths[i] === null ? [] : [link(c, paths[i]!)]));
          callback(links.length > 0 ? links : undefined);
        },
        // The core is down or refused: no links.
        () => callback(undefined),
      );
    },
  };
}
