import { invoke } from "@tauri-apps/api/core";

/** Opens the native open-file dialog, starting in `start` when it is a folder; `kind` "html" filters to HTML files. Null when cancelled. */
export async function pickFile(start: string | null, kind?: "html"): Promise<string | null> {
  return invoke<string | null>("pick_file", { start, kind });
}
