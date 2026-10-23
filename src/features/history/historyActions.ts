import type { Endpoint } from "../../generated/Endpoint";
import type { HistoryPatch } from "../../generated/HistoryPatch";
import { history } from "../../lib/api/history";

type Target = { agent: string; session_id: string };

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** What a change the core accepted is announced as. */
export function actionAnnouncement(p: HistoryPatch): string {
  if (p.name !== undefined) {
    const name = p.name.trim();
    return name === "" ? "Name cleared." : `Renamed to “${name}”.`;
  }
  if (p.pinned !== undefined) return p.pinned ? "Pinned." : "Unpinned.";
  if (p.archived !== undefined) return p.archived ? "Archived." : "Unarchived.";
  return "Tags saved.";
}

/** Starts a new Claude conversation from this one in a new tab. A refusal or a failure goes to `refuse` in words. */
export async function forkConversation(
  endpoint: Endpoint,
  target: Target,
  size: { cols: number; rows: number },
  to: { onResumed: (tabId: string) => void; refuse: (text: string) => void },
): Promise<void> {
  try {
    const r = await history.fork(endpoint, target.agent, target.session_id, size);
    if (r.ok) to.onResumed(r.tabId);
    else to.refuse(r.reason);
  } catch (e) {
    to.refuse(`Couldn't fork: ${message(e)}`);
  }
}

/**
 * Deletes Marshell's copy. Once it is gone the conversation closes, focus goes to the list and the list is loaded again; a refusal
 * (running in a tab, copy in progress) or a failure leaves it open and goes to `refuse`.
 */
export async function deleteConversation(
  endpoint: Endpoint,
  target: Target,
  title: string,
  to: { announce: (text: string) => void; closed: () => void; focusList: () => void; refresh: () => void; refuse: (text: string) => void },
): Promise<void> {
  try {
    const r = await history.remove(endpoint, target.agent, target.session_id);
    if (!r.ok) return to.refuse(r.reason);
    to.closed();
    to.focusList();
    to.refresh();
    to.announce(`Deleted Marshell's copy of “${title}”.`);
  } catch (e) {
    to.refuse(`Couldn't delete: ${message(e)}`);
  }
}
