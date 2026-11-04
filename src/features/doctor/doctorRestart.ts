import type { SessionInfo } from "../../generated/SessionInfo";
import type { SessionMode } from "../../generated/SessionMode";
import { listNumbers } from "./report";

export type RestartGo = { tabId: string; number: number; mode: SessionMode; elevated: boolean };

/**
 * Which of the fix's tab ids "Restart them" restarts, each in its current mode. A tab that is no longer open is dropped, silently:
 * its session is gone. A session that is working, one whose restart needs the administrator acknowledgement, and one with no mode
 * to come back in are skipped, and named by number for the receipt.
 */
export function planRestarts(tabIds: readonly string[], infos: readonly SessionInfo[], working: (tabId: string) => boolean, needsAck: (info: SessionInfo) => boolean): { go: RestartGo[]; skipped: number[] } {
  const go: RestartGo[] = [];
  const skipped: number[] = [];
  for (const tabId of tabIds) {
    const info = infos.find((i) => i.tab_id === tabId);
    if (!info) continue;
    if (info.agent === null || info.mode === null || working(tabId) || needsAck(info)) skipped.push(info.number);
    else go.push({ tabId, number: info.number, mode: info.mode, elevated: info.elevated === true });
  }
  return { go, skipped };
}

/** What the receipt says about sessions that were left for the person to restart. */
export function skippedReceipt(numbers: readonly number[]): string | null {
  if (numbers.length === 0) return null;
  return numbers.length === 1 ? `Session ${numbers[0]} wasn't restarted. Restart it from its tab.` : `Sessions ${listNumbers(numbers)} weren't restarted. Restart them from their tabs.`;
}
