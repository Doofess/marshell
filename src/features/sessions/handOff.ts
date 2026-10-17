import type { SessionInfo } from "../../generated/SessionInfo";

/**
 * The shell panel's tab id once the core has handed it off to the session list (it ran a known agent), else null. Null too while
 * an "Open in a pane" adopt of that tab is already pending, so the two paths never both act.
 */
export function handedOff(panelTabId: string | undefined, adoptedTabId: string | undefined, infos: SessionInfo[]): string | null {
  if (panelTabId === undefined || panelTabId === adoptedTabId) return null;
  return infos.some((i) => i.tab_id === panelTabId) ? panelTabId : null;
}
