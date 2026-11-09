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
 * - While a program holds synchronized output (DEC mode 2026), it buffers instead of drawing, so `privateRedraw` asks the
 *   renderer itself then, a second private name. The frame may show a half-finished update; the user chose that over an
 *   empty canvas (decision 10, with the live check's numbers in docs/research/2026-11-09-resize-fixes-live-check.md).
 *
 * Delete this file once the app is on an xterm release that carries PR #5529.
 */

/** Draws the rows from `start` to `end` now. */
export type Redraw = (start: number, end: number) => void;

type Renderer = { renderRows?: unknown };
type Private = {
  modes?: { synchronizedOutputMode?: boolean };
  _core?: { _renderService?: { _renderRows?: unknown; _renderer?: { value?: Renderer } } };
};

/**
 * xterm's private redraw for this terminal, or null when this xterm has no such function.
 *
 * While a program holds synchronized output (DEC mode 2026), `_renderRows` only notes the rows for later, which would leave
 * the cleared canvas empty until the program ends its update. The picture that mode asks the terminal to keep showing is
 * gone with the old canvas, so the redraw asks the renderer itself, one level down, to draw the screen as it stands.
 */
export function privateRedraw(term: unknown): Redraw | null {
  const t = term as Private | null;
  const service = t?._core?._renderService;
  const draw = service?._renderRows;
  if (typeof draw !== "function") return null;
  return (start, end) => {
    const renderer = service?._renderer?.value;
    if (t?.modes?.synchronizedOutputMode && typeof renderer?.renderRows === "function") (renderer.renderRows as Redraw).call(renderer, start, end);
    else (draw as Redraw).call(service, start, end);
  };
}

export type SyncRedraw = {
  /**
   * Fits the terminal, and draws every row at once when the fit changed the grid. Every fit goes through here, not only the
   * resize observer's: a fit this doesn't see leaves `deviceResized` comparing against a size the canvas no longer has.
   */
  fit(): void;
  /** For a device-pixel ResizeObserver on the canvas, made after xterm's own: draws when xterm set the canvas's size again. */
  deviceResized(): void;
};

/**
 * The two moments xterm clears its canvas during a resize, each answered with a draw in the same frame:
 * - a changed grid makes the renderer set the canvas's size, from the fit;
 * - at a fractional display scale the canvas's true size in device pixels can differ from the computed one, and the WebGL
 *   addon's own observer then sets it again, after the first draw. PR #5529 draws there too.
 *
 * `canvas` gives the renderer's canvas, or null when there is none (the DOM renderer).
 */
export function syncRedraw(
  term: { readonly cols: number; readonly rows: number },
  fit: () => void,
  redraw: Redraw,
  canvas: () => { readonly width: number; readonly height: number } | null,
): SyncRedraw {
  // The canvas's size at the last draw; null until a resize has drawn, so xterm's first paint stays its own.
  let drawnAt: { width: number; height: number } | null = null;
  const draw = () => {
    redraw(0, term.rows - 1);
    const c = canvas();
    drawnAt = c && { width: c.width, height: c.height };
  };
  return {
    fit() {
      const { cols, rows } = term;
      fit();
      if (term.cols !== cols || term.rows !== rows) draw();
    },
    deviceResized() {
      const c = canvas();
      if (c && drawnAt && (c.width !== drawnAt.width || c.height !== drawnAt.height)) draw();
    },
  };
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
