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
