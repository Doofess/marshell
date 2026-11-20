/** One terminal row as the layout needs it: how tall a row is, and the padding the terminal keeps above and below its rows. CSS pixels. */
export type RowCell = { height: number; pad: number };

/**
 * The row of a terminal that has been fitted: its screen's height over its rows. xterm draws a row a whole number of device pixels
 * tall, and reports its screen in rounded CSS pixels, so the quotient is snapped back to device pixels: the answer is then the same
 * whatever number of rows it was measured at, and a layout made from it can't change the measure it came from.
 * Null until there is a row to measure.
 */
export function rowCell(screenHeight: number, rows: number, pad: number, scale: number): RowCell | null {
  if (!Number.isFinite(screenHeight) || !Number.isFinite(rows) || screenHeight <= 0 || rows < 1) return null;
  const dpr = Number.isFinite(scale) && scale > 0 ? scale : 1;
  const device = Math.round((screenHeight / rows) * dpr);
  if (device < 1) return null;
  return { height: device / dpr, pad: Number.isFinite(pad) && pad > 0 ? pad : 0 };
}

export function sameCell(a: RowCell | null, b: RowCell | null): boolean {
  if (a === null || b === null) return a === b;
  return a.height === b.height && a.pad === b.pad;
}
