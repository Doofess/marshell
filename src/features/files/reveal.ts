import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { Os } from "../../lib/keymap";

/** The menu row that shows a file in the system's file manager. */
export function revealLabel(os: Os): string {
  if (os === "mac") return "Show in Finder";
  if (os === "linux") return "Show in file manager";
  return "Show in File Explorer";
}

/**
 * A path on a network share. The opener plugin checks no scope and touches the path, so a hand-edited layout.json could
 * make Explorer reach a share; the tree never shows one, so none is ever revealed.
 */
export function revealRefused(path: string): boolean {
  return /^[\\/]{2}/.test(path);
}

/** Selects the file in the system's file manager. Says why when it can't. */
export async function revealFile(path: string, announce: (t: string) => void, reveal: (p: string) => Promise<void> = revealItemInDir): Promise<void> {
  if (revealRefused(path)) {
    announce("Couldn't show the file: it's on a network share.");
    return;
  }
  try {
    await reveal(path);
  } catch (e) {
    announce(`Couldn't show the file: ${e instanceof Error ? e.message : String(e)}`);
  }
}
