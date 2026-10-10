/**
 * Split view: up to four terminals on screen at once (any number can run). The arrangement is a plain grid of
 * pane tracks with a 1 px hairline track between neighbours, so CSS grid does the sizing and each handle is one grid item.
 * The handle's 8 px hit area overhangs the neighbouring panes, so a divider costs one pixel of terminal, not eight.
 * `chooseArrangement` picks the layout that keeps the smallest pane closest to a comfortable 80 by 24.
 */
export const PANE_LIMIT = 4;
export const HANDLE = 1;
export const HIT = 8;
/** Terminal cell metrics at 13 px mono, the pane header, and the terminal's own padding. */
export const CELL = { w: 8, h: 17, header: 28, padX: 16, padY: 16 } as const;
const COMFORT = { cols: 80, rows: 24 } as const;
/** A layout this close to the best wins if it comes first in the preference order, so the choice does not flicker between near-equals. */
const TOLERANCE = 0.1;
export const RATIO = { min: 0.2, max: 0.8, default: 0.5 } as const;

export type PaneCount = 1 | 2 | 3 | 4;
export type Arrangement = "single" | "columns" | "rows" | "grid" | "main-left" | "main-top";
/** In order of preference; the first is the default. */
export const ARRANGEMENTS: Record<PaneCount, Arrangement[]> = {
  1: ["single"],
  2: ["columns", "rows"],
  3: ["main-left", "main-top", "columns", "rows"],
  4: ["grid", "columns", "rows"],
};

export type Size = { w: number; h: number };
export type Ratios = { col?: number; row?: number };
/** Grid lines, 1-based and end-exclusive, as CSS `grid-column` / `grid-row` take them. */
export type Area = { colStart: number; colEnd: number; rowStart: number; rowEnd: number };
export type Handle = Area & { orientation: "vertical" | "horizontal" };
export type Plan = { count: PaneCount; arrangement: Arrangement; colTracks: number; rowTracks: number; cells: Area[]; handles: Handle[] };

const area = (colStart: number, colEnd: number, rowStart: number, rowEnd: number): Area => ({ colStart, colEnd, rowStart, rowEnd });
/** Pane track i is grid line 2i+1; the handle track after it is 2i+2. */
const pane = (i: number): [number, number] => [2 * i + 1, 2 * i + 2];
const handle = (i: number): [number, number] => [2 * i + 2, 2 * i + 3];

export function planFor(count: PaneCount, arrangement: Arrangement): Plan {
  if (!ARRANGEMENTS[count]?.includes(arrangement)) throw new Error(`${arrangement} does not suit ${count} panes`);
  const make = (colTracks: number, rowTracks: number, cells: Area[], handles: Handle[]): Plan => ({ count, arrangement, colTracks, rowTracks, cells, handles });
  const range = (n: number) => Array.from({ length: n }, (_, i) => i);
  switch (arrangement) {
    case "single":
      return make(1, 1, [area(1, 2, 1, 2)], []);
    case "columns":
      return make(
        count,
        1,
        range(count).map((i) => area(...pane(i), 1, 2)),
        range(count - 1).map((i) => ({ orientation: "vertical" as const, ...area(...handle(i), 1, 2) })),
      );
    case "rows":
      return make(
        1,
        count,
        range(count).map((i) => area(1, 2, ...pane(i))),
        range(count - 1).map((i) => ({ orientation: "horizontal" as const, ...area(1, 2, ...handle(i)) })),
      );
    case "grid":
      return make(
        2,
        2,
        [area(1, 2, 1, 2), area(3, 4, 1, 2), area(1, 2, 3, 4), area(3, 4, 3, 4)],
        [
          { orientation: "vertical", ...area(2, 3, 1, 4) },
          { orientation: "horizontal", ...area(1, 4, 2, 3) },
        ],
      );
    case "main-left":
      return make(
        2,
        2,
        [area(1, 2, 1, 4), area(3, 4, 1, 2), area(3, 4, 3, 4)],
        [
          { orientation: "vertical", ...area(2, 3, 1, 4) },
          { orientation: "horizontal", ...area(3, 4, 2, 3) },
        ],
      );
    case "main-top":
      return make(
        2,
        2,
        [area(1, 4, 1, 2), area(1, 2, 3, 4), area(3, 4, 3, 4)],
        [
          { orientation: "horizontal", ...area(1, 4, 2, 3) },
          { orientation: "vertical", ...area(2, 3, 3, 4) },
        ],
      );
  }
}

const clampRatio = (r: number | undefined) => Math.min(RATIO.max, Math.max(RATIO.min, Number.isFinite(r) ? (r as number) : RATIO.default));
const round = (n: number) => Number(n.toFixed(3));

/** The CSS `grid-template-columns` / `-rows` value for `n` pane tracks. Two tracks follow the ratio; more are equal. */
export function tracks(n: number, ratio: number = RATIO.default): string {
  if (n === 1) return "minmax(0, 1fr)";
  if (n === 2) {
    const r = clampRatio(ratio);
    return `minmax(0, ${round(r)}fr) ${HANDLE}px minmax(0, ${round(1 - r)}fr)`;
  }
  return Array.from({ length: n }, () => "minmax(0, 1fr)").join(` ${HANDLE}px `);
}

/** Pixel size of each pane track along one axis. */
function trackSizes(total: number, n: number, ratio: number | undefined): number[] {
  const avail = Math.max(0, total - HANDLE * (n - 1));
  if (n === 1) return [avail];
  if (n === 2) {
    const r = clampRatio(ratio);
    return [avail * r, avail * (1 - r)];
  }
  return Array.from({ length: n }, () => avail / n);
}

/** Size along an axis of the lines [start, end): odd lines are pane tracks, even lines are handle tracks. */
function spanSize(sizes: number[], start: number, end: number): number {
  let sum = 0;
  for (let line = start; line < end; line++) sum += line % 2 === 1 ? sizes[(line - 1) / 2]! : HANDLE;
  return sum;
}

/** How many terminal columns and rows each pane really gets, after padding, its header and the handles. */
export function paneCells(plan: Plan, size: Size, ratios: Ratios = {}): { cols: number; rows: number }[] {
  const widths = trackSizes(size.w, plan.colTracks, ratios.col);
  const heights = trackSizes(size.h, plan.rowTracks, ratios.row);
  return plan.cells.map((c) => ({
    cols: Math.max(0, Math.floor((spanSize(widths, c.colStart, c.colEnd) - CELL.padX) / CELL.w)),
    rows: Math.max(0, Math.floor((spanSize(heights, c.rowStart, c.rowEnd) - CELL.header - CELL.padY) / CELL.h)),
  }));
}

/** 1 when every pane is at least 80 by 24; otherwise the share the smallest pane reaches. */
export function scoreArrangement(plan: Plan, size: Size, ratios: Ratios = {}): number {
  return Math.min(...paneCells(plan, size, ratios).map((p) => (Math.min(p.cols, COMFORT.cols) / COMFORT.cols) * (Math.min(p.rows, COMFORT.rows) / COMFORT.rows)));
}

export function chooseArrangement(count: PaneCount, size: Size, ratios: Ratios = {}): Arrangement {
  const scored = ARRANGEMENTS[count].map((a) => ({ a, score: scoreArrangement(planFor(count, a), size, ratios) }));
  const best = Math.max(...scored.map((s) => s.score));
  return scored.find((s) => s.score >= best - TOLERANCE)!.a;
}
