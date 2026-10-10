import type { AnnounceKind } from "../../components/a11y/announce-policy";
import { changeSummary } from "./edit";

export type SettingsState = {
  readOnly?: boolean;
  unparsable?: boolean;
  /** The file changed on disk after it was opened. */
  conflict?: boolean;
  /** There are unapplied changes. */
  changed?: boolean;
  /** The exact diff is showing, with Apply and Discard. */
  reviewing?: boolean;
  /** The changes were just applied. */
  applied?: boolean;
};

/** What the window's announcer says after Apply: the result of something the user did, and that a backup exists. Null otherwise. */
export function settingsAnnouncement(state: SettingsState, changes: Record<string, unknown>, label = "Claude Code"): { text: string; kind: AnnounceKind } | null {
  if (!state.applied) return null;
  const n = Object.keys(changes).length;
  return { text: `Applied ${changeSummary(n).toLowerCase()} to ${label} settings. A backup was saved.`, kind: "action-result" };
}

/** The file a category edits. Phase 0 shows Claude Code's; Marshell's own settings live in ~/.marshell/settings.json. */
export const settingsPath = (category: string): string => (category === "claude" ? "C:\\Users\\Home Office\\.claude\\settings.json" : "~/.marshell/settings.json");
