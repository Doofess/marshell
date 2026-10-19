import { invoke } from "@tauri-apps/api/core";

/** The taskbar count is every entry; an alert needs a new entry on a tab that isn't muted while Marshell isn't focused. */
export function attentionStep(
  prev: string[],
  next: { id: string; tab: string }[],
  focused: boolean,
  muted: Set<string>,
): { count: number; alert: boolean } {
  const seen = new Set(prev);
  const alert = !focused && next.some((n) => !seen.has(n.id) && !muted.has(n.tab));
  return { count: next.length, alert };
}

/** Calls a Tauri command, ignoring a failure: tests and a plain browser have no Tauri to answer. */
async function tell(command: string, args: Record<string, unknown>): Promise<void> {
  try {
    await invoke(command, args);
  } catch {
    // No Tauri here, or the command failed; the badge and the flash are only a courtesy.
  }
}

export const setBadge = (count: number) => tell("set_badge", { count });
export const alertNeedsYou = (sound: boolean) => tell("alert_needs_you", { sound });
