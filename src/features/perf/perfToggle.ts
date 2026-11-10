import type { KeyEventLike } from "../../lib/bindings";

/** The chord that shows or hides the terminal's perf overlay (Ctrl or Cmd, Shift and Alt, P). A key-down only. */
export function isPerfToggle(e: KeyEventLike & { type: string }): boolean {
  return e.type === "keydown" && e.shiftKey && e.altKey && (e.ctrlKey || e.metaKey) && e.code === "KeyP";
}
