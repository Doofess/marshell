/** A box on a page or image, in fractions of it (0..1), so it survives zoom and reload. `page` is 1 for an image. */
export type Region = { page: number; x: number; y: number; w: number; h: number };

type Size = { w: number; h: number };
type Point = { x: number; y: number };

const MIN_PX = 8;
const STEP_PX = 10;
const FINE_STEP_PX = 1;
const MAX_TEXT = 2000;

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** 40% x 25% of the page, centred in the part of it that is on screen (`visible` is a fraction of the page height). */
export function initialRegion(page: number, visible: { top: number; bottom: number }): Region {
  const w = 0.4;
  const h = 0.25;
  const centre = (visible.top + visible.bottom) / 2;
  return { page, x: (1 - w) / 2, y: clamp(centre - h / 2, 0, 1 - h), w, h };
}

/** The box a drag from `a` to `b` (CSS px within the page) draws, or null when either side is under 8 px. */
export function fromDrag(page: number, a: Point, b: Point, size: Size): Region | null {
  const x0 = clamp(Math.min(a.x, b.x), 0, size.w);
  const x1 = clamp(Math.max(a.x, b.x), 0, size.w);
  const y0 = clamp(Math.min(a.y, b.y), 0, size.h);
  const y1 = clamp(Math.max(a.y, b.y), 0, size.h);
  if (x1 - x0 < MIN_PX || y1 - y0 < MIN_PX) return null;
  return { page, x: x0 / size.w, y: y0 / size.h, w: (x1 - x0) / size.w, h: (y1 - y0) / size.h };
}

/** An arrow key moves the box 10 px (1 px with Ctrl); with Shift it resizes from the bottom-right corner (at least 8 px). Always inside the page. */
export function nudge(
  r: Region,
  key: "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown",
  mods: { shift: boolean; ctrl: boolean },
  size: Size,
): Region {
  const step = mods.ctrl ? FINE_STEP_PX : STEP_PX;
  const sign = key === "ArrowLeft" || key === "ArrowUp" ? -1 : 1;
  const horizontal = key === "ArrowLeft" || key === "ArrowRight";
  const dx = horizontal ? (sign * step) / size.w : 0;
  const dy = horizontal ? 0 : (sign * step) / size.h;
  if (mods.shift) {
    return {
      ...r,
      w: clamp(r.w + dx, Math.min(MIN_PX / size.w, 1 - r.x), 1 - r.x),
      h: clamp(r.h + dy, Math.min(MIN_PX / size.h, 1 - r.y), 1 - r.y),
    };
  }
  return { ...r, x: clamp(r.x + dx, 0, 1 - r.w), y: clamp(r.y + dy, 0, 1 - r.h) };
}

/** False once the document has fewer pages than the box's page (a reload that changed the file). */
export function regionStillValid(r: Region, pageCount: number): boolean {
  return Number.isInteger(r.page) && r.page >= 1 && r.page <= pageCount;
}

/** The box in words, for a screen reader. */
export function describeRegion(r: Region): string {
  const pct = (v: number) => `${Math.round(v * 100)}%`;
  return `Box ${pct(r.w)} wide, ${pct(r.h)} high, ${pct(r.x)} across, ${pct(r.y)} down, page ${r.page}`;
}

/** "report.pdf, page 3" for a PDF, "chart.png" for an image (page null). */
export function regionLabel(name: string, page: number | null): string {
  return page === null ? name : `${name}, page ${page}`;
}

/** The reference the agent is given: the label, then the comment with every run of whitespace collapsed; 2000 characters at most. */
export function regionText(label: string, comment: string): string {
  const note = comment.replace(/\s+/g, " ").trim();
  return (note ? `${label}: ${note}` : label).slice(0, MAX_TEXT);
}
