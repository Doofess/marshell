import { invoke } from "@tauri-apps/api/core";

/** Opens the native folder picker, starting in `start` when it is a folder. Null when cancelled. */
export async function pickFolder(start: string | null): Promise<string | null> {
  return invoke<string | null>("pick_folder", { start });
}
