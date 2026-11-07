import { inRenameBox } from "./rowMenu";

type Mods = { ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean };

/** Ctrl+Space on a focused row selects it like Enter, on every OS (Cmd+Space is Spotlight), unless the key is meant for the rename text box. */
export function selectsByCtrlSpace(e: Mods & { key: string; target: object | null }): boolean {
  if (inRenameBox(e.target)) return false;
  return e.key === " " && e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey;
}
