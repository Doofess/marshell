/** What a dialog does with a key: move focus inside it, close, stay put, or nothing (the key belongs to the control). */
export type TrapAction = { kind: "focus"; index: number } | { kind: "close" } | { kind: "stay" } | null;

/**
 * The keys of a modal dialog. Tab and Shift+Tab walk its controls and wrap, so focus never leaves; Escape closes it. `current` is
 * the index of the focused control, or -1 when focus is on the dialog's title or on nothing inside it.
 */
export function trapKey(e: { key: string; shiftKey?: boolean }, count: number, current: number): TrapAction {
  if (e.key === "Escape") return { kind: "close" };
  if (e.key !== "Tab") return null;
  if (count === 0) return { kind: "stay" };
  if (e.shiftKey) return { kind: "focus", index: current <= 0 ? count - 1 : current - 1 };
  return { kind: "focus", index: current < 0 || current >= count - 1 ? 0 : current + 1 };
}
