/** Where the launcher's highlighted option is being moved: the folder field (a text input) or the program grid. */
export type OpenList = "folder" | "programs" | null;

/**
 * The keys that move the highlighted option. In the folder field Left and Right belong to the caret, so a typo in the
 * query can be fixed; only the program grid, which has no caret, uses all four arrows.
 */
export function listNavKeys(open: OpenList): readonly string[] {
  if (open === "programs") return ["ArrowDown", "ArrowUp", "ArrowRight", "ArrowLeft"];
  if (open === "folder") return ["ArrowDown", "ArrowUp"];
  return [];
}

/** The next highlighted index for an arrow key, wrapping, and recovering from a stale index. */
export function stepActive(index: number, key: string, size: number): number {
  if (size <= 0) return 0;
  const step = key === "ArrowDown" || key === "ArrowRight" ? 1 : -1;
  const from = Number.isFinite(index) ? Math.min(Math.max(0, Math.floor(index)), size - 1) : 0;
  return (from + step + size) % size;
}

/**
 * Closing the folder field swaps an input for a button, so the browser drops focus on the page and the sheet's keys (Esc, Enter)
 * stop arriving. True when the folder chip should take focus back: the field just closed and nothing real has focus.
 */
export function focusFellOut({ wasOpen, isOpen, activeIsBody }: { wasOpen: boolean; isOpen: boolean; activeIsBody: boolean }): boolean {
  return wasOpen && !isOpen && activeIsBody;
}
