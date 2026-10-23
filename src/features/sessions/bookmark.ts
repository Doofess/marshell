import type { Endpoint } from "../../generated/Endpoint";
import { UNTITLED } from "../history/historyActions";
import { sessions } from "../../lib/api/sessions";

/** Ctrl+Shift+B, the palette's "Bookmark this point" and the row menu's Bookmark: marks the current point of a Claude Code session and says how it went, in the core's words when it refuses. */
export async function bookmarkSession(endpoint: Endpoint, tabId: string, announce: (text: string) => void): Promise<void> {
  try {
    const r = await sessions.bookmark(endpoint, tabId);
    announce(r.ok ? `Bookmarked this point in “${r.title || UNTITLED}”.` : r.reason);
  } catch (e) {
    announce(`Couldn't bookmark: ${e instanceof Error ? e.message : String(e)}`);
  }
}
