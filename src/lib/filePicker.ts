import { invoke } from "@tauri-apps/api/core";

/** Opens the native open-file dialog, starting in `start` when it is a folder. Null when cancelled. */
export async function pickFile(start: string | null): Promise<string | null> {
  return invoke<string | null>("pick_file", { start });
}
