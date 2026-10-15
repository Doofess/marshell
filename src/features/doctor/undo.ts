export const TOAST_MS = 10_000;
export type UndoState = { fixedAtMs: number; count: number; filesMatch: boolean };
export const toastVisible = (u: UndoState, nowMs: number) => nowMs - u.fixedAtMs < TOAST_MS;
/** The toast for 10 s, then Undo last fix in the Doctor header; once a file changed, undo is withdrawn and says why. */
export function undoOffer(u: UndoState, nowMs: number): { kind: "toast" | "header" | "stale"; text: string } {
  if (!u.filesMatch) return { kind: "stale", text: "A file changed since the fix, so undo would overwrite it. Show differences." };
  return toastVisible(u, nowMs) ? { kind: "toast", text: "Undo" } : { kind: "header", text: "Undo last fix" };
}
