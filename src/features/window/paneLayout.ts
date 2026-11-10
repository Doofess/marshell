/** The side pane's numbers: its narrowest and widest saved width, the divider's keyboard step, and the room under which it covers the terminal. */
export const PANE = { min: 320, max: 4096, step: 16, coverBelow: 720 } as const;

export type PaneMode = "hidden" | "beside" | "cover";

export type PaneGeometry = {
  mode: PaneMode;
  terminal: number;
  pane: number;
  /** The widest the pane may be dragged, for aria-valuemax. */
  max: number;
};

function whole(n: number): number {
  return Number.isFinite(n) ? Math.max(0, Math.round(n)) : 0;
}

function clamp(n: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, n));
}

/** The widest the pane may be: what leaves the terminal its 80 columns, and never under the pane's minimum. */
function widest(room: number, terminalMin: number): number {
  return Math.max(PANE.min, room - 1 - whole(terminalMin));
}

/**
 * The window's geometry as pixels. `room` is the main area's width and `terminalMin` the width of 80 columns; the divider is 1 px
 * in "beside". Under 720 px the pane covers the terminal. Beside, the pane keeps its saved width (half the room when nothing is
 * saved) held between 320 px and the 80-column stop; only at 320 px can the terminal go under 80 columns.
 */
export function paneGeometry(i: { room: number; shown: boolean; saved: number | null; terminalMin: number }): PaneGeometry {
  const room = whole(i.room);
  const max = widest(room, i.terminalMin);
  if (room === 0) return { mode: "hidden", terminal: 0, pane: 0, max };
  if (!i.shown) return { mode: "hidden", terminal: room, pane: 0, max };
  if (room < PANE.coverBelow) return { mode: "cover", terminal: 0, pane: room, max };
  const wanted = i.saved !== null && Number.isFinite(i.saved) ? Math.round(i.saved) : Math.round((room - 1) / 2);
  const pane = clamp(wanted, PANE.min, max);
  return { mode: "beside", terminal: room - 1 - pane, pane, max };
}

/**
 * The key of the pane's layout motion (`useLayoutMotion`): a change of key moves for 200 ms, anything else is followed at once.
 * Showing, hiding and covering change it. So does a sidebar switch beside an open pane: the room changes with the sidebar, and
 * a pane at half the room (or at the 80-column stop) changes width with it, which must ease with the sidebar instead of jumping.
 * A hidden pane has nothing to move, and a covering pane is the room itself, so the sidebar leaves their keys alone.
 */
export function paneMotionKey(mode: PaneMode, sidebar: string): string {
  return mode === "beside" ? `beside/${sidebar}` : mode;
}

/** The width to save when the divider is dragged to `fromEnd` px from the main area's end edge, or nudged. */
export function draggedWidth(i: { room: number; terminalMin: number; fromEnd: number }): number {
  const max = widest(whole(i.room), i.terminalMin);
  const from = Number.isFinite(i.fromEnd) ? i.fromEnd : PANE.min;
  return clamp(Math.round(from), PANE.min, max);
}
