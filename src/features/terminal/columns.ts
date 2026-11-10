import { fontStack } from "./terminalFont";

type Font = { family: string; size: number; wtFamily?: string };

/**
 * The canvas font a glyph is measured in: the stack the terminal draws with (terminalFont.ts), not the family alone. A family
 * that isn't installed is then measured in the font that takes its place in the terminal; alone it fell to the canvas's own
 * default, a wider sans-serif, and the side pane's 80-column stop came far too early.
 */
export const canvasFont = (font: Font): string => `${font.size}px ${fontStack(font.family, font.wtFamily)}`;

/** One glyph's width in this font, read from a canvas. Without a canvas (no document) it is 0.6 of the size. */
function measureCell(font: Font): number {
  if (typeof document === "undefined") return font.size * 0.6;
  const ctx = document.createElement("canvas").getContext("2d");
  if (!ctx) return font.size * 0.6;
  ctx.font = canvasFont(font);
  const w = ctx.measureText("M").width;
  return w > 0 ? w : font.size * 0.6;
}

/** The screen's scale: device pixels per CSS pixel. 1 without a window. */
export const screenScale = (): number => (typeof window === "undefined" ? 1 : window.devicePixelRatio || 1);

/**
 * The CSS width of `cols` terminal columns in this font. `measure` returns one glyph's width; without a canvas it is 0.6 × size.
 * A cell is that glyph in whole device pixels, rounded down, which is how xterm's WebGL renderer sizes it: the bundled font's
 * 7.8 px glyph is a 7 px cell at 100%. Counting the glyph's own width instead put the pane's 80-column stop at 89 columns.
 */
export function columnsWidth(cols: number, font: Font, measure: (font: Font) => number = measureCell, scale: number = screenScale()): number {
  const dpr = Number.isFinite(scale) && scale > 0 ? scale : 1;
  const cell = Math.floor(measure(font) * dpr) / dpr;
  return cols * cell;
}

/** Both sides of the gutter on xterm's element: `.terminal-host .xterm { padding: var(--space-2) }` in src/styles/app.css, 8 px each. */
export const TERMINAL_HOST_PADDING = 16;
/** The scrollbar FitAddon leaves out of the width while `scrollback` is above 0 and no `overviewRuler` width is set: xterm's default of 14 px. */
export const TERMINAL_SCROLLBAR = 14;

/** The least width of a terminal host that still fits `cols` columns: the columns, the host's padding and the scrollbar's gutter. */
export function terminalFloorWidth(cols: number, font: Font, measure?: (font: Font) => number, scale?: number): number {
  return Math.ceil(columnsWidth(cols, font, measure, scale)) + TERMINAL_HOST_PADDING + TERMINAL_SCROLLBAR;
}
