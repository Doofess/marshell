import { invoke } from "@tauri-apps/api/core";

/** The needs-you count: distinct sessions that need you. The taskbar badge, the window title and the tray all show it. By tab, not by id: a waiting row that its held request replaces is the same session. */
export function needsYouCount(entries: { id: string; tab: string }[]): number {
  return new Set(entries.map((e) => e.tab)).size;
}

/** Calls a Tauri command, ignoring a failure: tests and a plain browser have no Tauri to answer. */
async function tell(command: string, args: Record<string, unknown>): Promise<void> {
  try {
    await invoke(command, args);
  } catch {
    // No Tauri here, or the command failed; the badge is only a courtesy.
  }
}

export const setBadge = (count: number) => tell("set_badge", { count });
