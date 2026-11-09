/**
 * A full-screen program's text stays where it is when the terminal loses rows.
 *
 * When rows go, xterm 6.0 keeps the cursor's row on the screen: it drops empty rows below the cursor, and once the cursor is
 * on the last row it drops rows from the top instead. For a shell that is right, the top rows scroll into the history. A
 * full-screen program (the alternate screen: Codex, vim, less) has no history and draws from the top, and many park the
 * cursor on the last row, so everything moves up a row and comes back when the program repaints, about three frames later:
 * the text jumps while a window edge moves. Before a fit that loses rows, the terminal moves that screen's cursor onto the
 * new last row, where xterm puts it anyway, so xterm drops the rows from the bottom (PLAN §9, 6q-0 decision 10;
 * docs/research/2026-11-09-resize-fixes-live-check.md).
 *
 * The cursor is xterm's private `_core._bufferService.buffers.alt.y`, which any xterm release can rename.
 * `keepFullScreenTop` answers false then, the terminal fits as xterm does alone, and topAnchor.test.ts, which runs the
 * installed xterm, fails on the upgrade.
 */

type Size = { cols: number; rows: number };
type Private = {
  rows?: number;
  buffer?: { active?: { type?: string } };
  _core?: { _bufferService?: { buffers?: { alt?: { y?: unknown } } } };
};

/**
 * Call just before a resize to `next`. Answers false only when the anchor was needed and xterm's private cursor is gone.
 * A shell's screen, a resize that loses no rows and a size that can't be measured are left alone.
 */
export function keepFullScreenTop(term: unknown, next: Size | undefined): boolean {
  const t = term as Private | null;
  if (!next || !Number.isFinite(next.cols) || !(next.rows >= 1) || typeof t?.rows !== "number" || next.rows >= t.rows) return true;
  if (t.buffer?.active?.type !== "alternate") return true;
  const screen = t._core?._bufferService?.buffers?.alt;
  if (!screen || typeof screen.y !== "number") return false;
  if (screen.y > next.rows - 1) screen.y = next.rows - 1;
  return true;
}

/**
 * A fit that anchors a full-screen program's text first, for the size the fit addon is about to set. The addon measures again
 * inside its own fit, so only a full-screen program pays for the second measurement; a shell is fitted as before.
 */
export function topAnchoredFit(term: unknown, fit: { proposeDimensions(): Size | undefined; fit(): void }, missing: () => void = reportMissingAnchor): () => void {
  return () => {
    const fullScreen = (term as Private | null)?.buffer?.active?.type === "alternate";
    if (fullScreen && !keepFullScreenTop(term, fit.proposeDimensions())) missing();
    fit.fit();
  };
}

let reported = false;

/** Says, once per page load, that the private cursor is gone. */
export function reportMissingAnchor(log: (message: string) => void = console.error): void {
  if (reported) return;
  reported = true;
  log("Marshell: xterm no longer has BufferService.buffers.alt.y; a full-screen program's text may jump while the terminal loses rows.");
}

/** For tests: forget that the missing cursor was reported. */
export function resetMissingAnchorReport(): void {
  reported = false;
}
