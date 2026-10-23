import { invoke } from "@tauri-apps/api/core";

/** Whether the marshell:// link is registered to this Marshell. Null when it cannot be known: outside Tauri, a failed call, or a platform without the link. */
export async function readNotificationLink(): Promise<boolean | null> {
  try {
    return (await invoke<boolean | null>("notification_link")) ?? null;
  } catch {
    return null;
  }
}

/** Registers or removes the link and returns the state it ended in. A failure is thrown for the dialog to show. */
export function setNotificationLink(on: boolean): Promise<boolean> {
  return invoke<boolean>("set_notification_link", { on });
}

/** The tab a notification-centre click should select: the one it names while that session is still open, else null (show the window and leave the selection alone). */
export function focusTarget(tabId: string | null, tabIds: readonly string[]): string | null {
  return tabId !== null && tabIds.includes(tabId) ? tabId : null;
}
