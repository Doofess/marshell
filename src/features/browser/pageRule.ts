import { paneColumn, type ColumnInputs } from "../window/paneColumn";
import type { Piece } from "./browserApi";

export type { Piece };

/** How far a line may stray from the column at a whole width and still be the same piece. */
const TOLERANCE = 1e-9;

/**
 * The page's slot as straight pieces over whole window widths: the pane's column, less the slot's distance from the column's
 * left edge (`start`) and right edge (`end`). A new piece starts wherever the line so far stops matching. The shell follows
 * the rule while the window moves, before the app has laid out the new size.
 */
export function pageRule(i: ColumnInputs, inset: { start: number; end: number }, maxWidth = 8192): Piece[] {
  const slot = (w: number) => {
    const col = paneColumn(w, i);
    return { x: col.x + inset.start, width: Math.max(0, col.width - inset.start - inset.end) };
  };
  const pieces: Piece[] = [];
  let from = 0;
  while (from <= maxWidth) {
    const a = slot(from);
    const b = from < maxWidth ? slot(from + 1) : a;
    const piece: Piece = { from, x: a.x, xPerPx: b.x - a.x, width: a.width, widthPerPx: b.width - a.width };
    pieces.push(piece);
    let w = from + 1;
    for (; w <= maxWidth; w++) {
      const s = slot(w);
      const d = w - from;
      if (Math.abs(s.x - (piece.x + piece.xPerPx * d)) > TOLERANCE || Math.abs(s.width - (piece.width + piece.widthPerPx * d)) > TOLERANCE) break;
    }
    from = w;
  }
  return pieces;
}
