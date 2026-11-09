// Spike 6q-0 (never merged): MainWindow's layout as numbers, so Rust can place the page for any window width inside the
// resize itself, jumps included (S12 finding 21).
import { DRAWER, drawerPlacement, sidebarMode, sidebarWidth, type SidebarPref } from "../window/layout";
import { clampPreviewRatio, previewLayout } from "../window/previewLayout";

/** What MainWindow's layout reads besides the window's width. */
export type RuleState = { pref: SidebarPref; sideReq?: number; ratio: number; drawer: boolean };

/** The preview column's left edge and width in a window `w` wide, as MainWindow lays it out (left-to-right only). */
export function paneColumn(w: number, s: RuleState): { x: number; width: number } {
  const mode = sidebarMode(w, s.pref);
  const side = sidebarWidth(mode, s.sideReq);
  const pushed = s.drawer && drawerPlacement(w, mode, DRAWER.default) === "push" ? DRAWER.default : 0;
  const main = w - side - pushed;
  if (previewLayout(main) === "full") return { x: side, width: main };
  // `previewTracks`: the session and the preview share what the 1 px hairline leaves, in fr.
  const r = clampPreviewRatio(s.ratio);
  const rest = Math.round((1 - r) * 1000) / 1000;
  const free = Math.max(0, main - 1);
  const session = (free * r) / (r + rest);
  return { x: side + session + 1, width: free - session };
}

/** One straight stretch: from window width `from` until the next piece, each value changes by its `PerPx` per pixel. */
export type Piece = { from: number; x: number; xPerPx: number; width: number; widthPerPx: number };

const same = (a: number, b: number) => Math.abs(a - b) < 1e-6;

/**
 * The layout as straight pieces over whole-pixel window widths up to `maxWidth`: a new piece starts wherever the line
 * so far stops matching, which is at every jump (the sidebar folding, the page taking the whole main area).
 */
export function paneRule(s: RuleState, maxWidth = 8192): Piece[] {
  const start = (w: number): Piece => {
    const a = paneColumn(w, s);
    const b = paneColumn(w + 1, s);
    return { from: w, x: a.x, xPerPx: b.x - a.x, width: a.width, widthPerPx: b.width - a.width };
  };
  let piece = start(0);
  const pieces = [piece];
  for (let w = 1; w <= maxWidth; w++) {
    const c = paneColumn(w, s);
    const d = w - piece.from;
    if (same(piece.x + piece.xPerPx * d, c.x) && same(piece.width + piece.widthPerPx * d, c.width)) continue;
    piece = start(w);
    pieces.push(piece);
  }
  return pieces;
}
