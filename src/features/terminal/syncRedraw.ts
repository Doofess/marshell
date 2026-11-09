/**
 * The terminal draws in the same frame it resizes.
 *
 * xterm 6.0's WebGL renderer clears its canvas when the grid changes and draws again on the next animation frame, so the
 * text flickers while a window edge or a divider moves. xterm fixed it in PR #5529, which is in 6.1 and later; 6.0 is the
 * newest stable release, so until then the terminal calls xterm's private `RenderService._renderRows` itself, right after
 * the fit (PLAN §9, 6q-0 decisions 8 and 10; docs/spikes/S12-browser-pane.md finding 23).
 *
 * What the private call costs:
 * - It can be renamed by any xterm release. `privateRedraw` answers null then, the terminal falls back to the next-frame
 *   fit, and syncRedraw.test.ts fails on the upgrade.
 * - It goes around xterm's debouncer, so the refresh the resize queued still runs a frame later: each step draws twice,
 *   as upstream's fix does.
 * - While a program holds synchronized output (DEC mode 2026), it buffers instead of drawing.
 *
 * Delete this file once the app is on an xterm release that carries PR #5529.
 */

/** Draws the rows from `start` to `end` now. */
export type Redraw = (start: number, end: number) => void;

type Private = { _core?: { _renderService?: { _renderRows?: unknown } } };

/** xterm's private redraw for this terminal, or null when this xterm has no such function. */
export function privateRedraw(term: unknown): Redraw | null {
  const service = (term as Private | null)?._core?._renderService;
  const draw = service?._renderRows;
  if (typeof draw !== "function") return null;
  return (start, end) => (draw as Redraw).call(service, start, end);
}

/** Fits the terminal, and draws every row at once when the fit changed the grid (a changed grid is what clears the canvas). */
export function fitAndRedraw(term: { readonly cols: number; readonly rows: number }, fit: () => void, redraw: Redraw): void {
  const { cols, rows } = term;
  fit();
  if (term.cols === cols && term.rows === rows) return;
  redraw(0, term.rows - 1);
}

let reported = false;

/** Says, once per page load, that the private redraw is gone. */
export function reportMissingRedraw(log: (message: string) => void = console.error): void {
  if (reported) return;
  reported = true;
  log("Marshell: xterm no longer has RenderService._renderRows; the terminal falls back to the next-frame fit and may flicker while resizing.");
}

/** For tests: forget that the missing redraw was reported. */
export function resetMissingRedrawReport(): void {
  reported = false;
}
