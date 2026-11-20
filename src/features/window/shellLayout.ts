import type { RowCell } from "../terminal/rowCell";

/** The shell panel under the session list: a 32 px header over a body that is 30% of the sidebar, within 120 to 280 px. */
export const SHELL = { header: 32, share: 0.3, minBody: 120, maxBody: 280 } as const;
const FOOTER = 40;
/** How many session rows the list must keep once the panel is open. */
const MIN_ROWS = 2;
/** When the user opens a folded panel anyway, it keeps one row and takes what is left; under this much terminal it stays a header. */
const MIN_FORCED_BODY = 60;

export type ShellLayout = { height: number; collapsed: boolean; auto: boolean };

const number = (n: number, fallback: number) => (Number.isFinite(n) ? n : fallback);

/**
 * A body cut down to the terminal rows that fit in it, so no part of a row shows under the last line. A fractional total is
 * rounded up, never down, or the last row would not fit. Without a measured row, or with room for none, the body is left as it is.
 */
export function wholeRows(body: number, cell: RowCell | null): number {
  if (!cell || !(cell.height > 0)) return Math.round(body);
  const rows = Math.floor((body - cell.pad) / cell.height + 1e-6);
  return rows < 1 ? Math.round(body) : Math.ceil(cell.pad + rows * cell.height - 1e-6);
}

/**
 * How tall the shell panel is. The user's choice to hide it always stands. Otherwise it folds to its header by itself
 * when an open panel would leave the list under two rows, so the sessions are never squeezed out by a convenience.
 * `forceOpen` is the user asking for it anyway (the shortcut on a folded panel): then it fits one row instead of two.
 * `cell` is the shell terminal's measured row: with it, an open body is as tall as whole rows (`wholeRows`).
 */
export function shellLayout(sidebarHeight: number, laneHeight: number, collapsedPref = false, rowHeight = 56, forceOpen = false, cell: RowCell | null = null): ShellLayout {
  const collapsed = { height: SHELL.header, collapsed: true };
  if (collapsedPref) return { ...collapsed, auto: false };
  if (!Number.isFinite(sidebarHeight) || sidebarHeight <= 0) return { ...collapsed, auto: true };
  const body = wholeRows(Math.min(SHELL.maxBody, Math.max(SHELL.minBody, SHELL.share * sidebarHeight)), cell);
  const open = SHELL.header + body;
  const lane = Math.max(0, number(laneHeight, 0));
  const room = sidebarHeight - lane - FOOTER - open;
  if (room < MIN_ROWS * rowHeight) {
    const forced = wholeRows(Math.min(body, sidebarHeight - lane - FOOTER - rowHeight - SHELL.header), cell);
    return forceOpen && forced >= MIN_FORCED_BODY ? { height: SHELL.header + forced, collapsed: false, auto: false } : { ...collapsed, auto: true };
  }
  return { height: open, collapsed: false, auto: false };
}
