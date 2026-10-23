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

/** Where keyboard focus goes when a link or a banner brings Marshell forward: a session's row, one key from its terminal
 * but never in it, so typing meant for another window cannot answer an agent. The linked session's row, else the selected
 * session's, else the main area. */
export function linkFocusSelector(tabId: string | null, selectedId: string | null, rows: readonly { id: string; number: number }[]): string {
  const row = rows.find((r) => r.id === (tabId ?? selectedId));
  return row ? `[data-nav-row="${row.number}"]` : ".window__main";
}
