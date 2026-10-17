import type { Os } from "../../lib/keymap";
import { inRenameBox } from "./rowMenu";

type Mods = { ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean };

/**
 * A click that adds or removes the session's pane: Cmd alone on macOS (Ctrl+click is a right-click there), Ctrl alone elsewhere.
 * A click in the rename text box stays with the box.
 */
export function togglesPane(e: Mods & { target?: object | null }, os: Os): boolean {
  if (e.altKey || e.shiftKey || inRenameBox(e.target ?? null)) return false;
  return os === "mac" ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
}

/** Ctrl+Space on a focused row does the same on every OS (Cmd+Space is Spotlight), unless the key is meant for the rename text box. */
export function togglesPaneKey(e: Mods & { key: string; target: object | null }): boolean {
  if (inRenameBox(e.target)) return false;
  return e.key === " " && e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey;
}
