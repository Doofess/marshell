type Font = { family: string; size: number };

/** One cell's width in this font, read from a canvas. Without a canvas (no document) a cell is 0.6 of the size. */
function measureCell(font: Font): number {
  if (typeof document === "undefined") return font.size * 0.6;
  const ctx = document.createElement("canvas").getContext("2d");
  if (!ctx) return font.size * 0.6;
  ctx.font = `${font.size}px ${font.family}`;
  const w = ctx.measureText("M").width;
  return w > 0 ? w : font.size * 0.6;
}

/** The CSS width of `cols` terminal columns in this font. `measure` returns one cell's width; without a canvas it is 0.6 × size. */
export function columnsWidth(cols: number, font: Font, measure: (font: Font) => number = measureCell): number {
  return cols * measure(font);
}

/** Both sides of the gutter on xterm's element: `.terminal-host .xterm { padding: var(--space-2) }` in src/styles/app.css, 8 px each. */
export const TERMINAL_HOST_PADDING = 16;
/** The scrollbar FitAddon leaves out of the width while `scrollback` is above 0 and no `overviewRuler` width is set: xterm's default of 14 px. */
export const TERMINAL_SCROLLBAR = 14;

/** The least width of a terminal host that still fits `cols` columns: the columns, the host's padding and the scrollbar's gutter. */
export function terminalFloorWidth(cols: number, font: Font, measure?: (font: Font) => number): number {
  return Math.ceil(columnsWidth(cols, font, measure)) + TERMINAL_HOST_PADDING + TERMINAL_SCROLLBAR;
}
