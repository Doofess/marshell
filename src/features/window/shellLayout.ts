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
 * How tall the shell panel is. The user's choice to hide it always stands. Otherwise it folds to its header by itself
 * when an open panel would leave the list under two rows, so the sessions are never squeezed out by a convenience.
 * `forceOpen` is the user asking for it anyway (the shortcut on a folded panel): then it fits one row instead of two.
 */
export function shellLayout(sidebarHeight: number, laneHeight: number, collapsedPref = false, rowHeight = 56, forceOpen = false): ShellLayout {
  const collapsed = { height: SHELL.header, collapsed: true };
  if (collapsedPref) return { ...collapsed, auto: false };
  if (!Number.isFinite(sidebarHeight) || sidebarHeight <= 0) return { ...collapsed, auto: true };
  const body = Math.min(SHELL.maxBody, Math.max(SHELL.minBody, SHELL.share * sidebarHeight));
  const open = SHELL.header + Math.round(body);
  const lane = Math.max(0, number(laneHeight, 0));
  const room = sidebarHeight - lane - FOOTER - open;
  if (room < MIN_ROWS * rowHeight) {
    const forced = Math.min(Math.round(body), sidebarHeight - lane - FOOTER - rowHeight - SHELL.header);
    return forceOpen && forced >= MIN_FORCED_BODY ? { height: SHELL.header + forced, collapsed: false, auto: false } : { ...collapsed, auto: true };
  }
  return { height: open, collapsed: false, auto: false };
}
