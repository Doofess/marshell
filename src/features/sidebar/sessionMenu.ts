import type { MenuItem } from "../../components/ContextMenu/menuKeys";
import { MODES } from "../modes/sessionModes";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { modeLabel } from "../window/SessionHeader";
import type { Mode } from "./types";

export type SessionMenuFacts = {
  /** An agent session has a mode to restart in; a plain shell has none. */
  agent: boolean;
  /** Restarting in another mode will not replace an ended agent's live shell. */
  modeChangeable: boolean;
  resumable: boolean;
  task: boolean;
  inShownPane: boolean;
  paneAvailable: boolean;
  /** Running and not paused. */
  pausable: boolean;
  paused: boolean;
  /** The session's banners and sounds are muted. */
  muted: boolean;
  /** A Claude Code session: the only agent whose points can be bookmarked for now. */
  bookmarkable: boolean;
};

/** The items a session row's menu shows. Items that don't apply are left out, never disabled. */
export function sessionMenuItems(facts: SessionMenuFacts, os: Os): MenuItem[] {
  const shortcut = (id: string) => shortcutLabel(commandById(id).binding!, os);
  const items: MenuItem[] = [{ id: "rename", label: "Rename", shortcut: shortcut("rename") }];
  if (!facts.inShownPane && facts.paneAvailable) items.push({ id: "split", label: "Split into a pane" });
  if (facts.resumable) items.push({ id: "resume", label: "Resume" });
  if (facts.agent && facts.modeChangeable) items.push({ id: "mode", label: "Mode ▸" });
  if (facts.pausable) items.push({ id: "pause", label: "Pause" });
  if (facts.paused) items.push({ id: "continue", label: "Continue" });
  if (facts.bookmarkable) items.push({ id: "bookmark", label: "Bookmark", shortcut: shortcut("bookmark") });
  items.push(facts.muted ? { id: "unmute", label: "Unmute notifications" } : { id: "mute", label: "Mute notifications" });
  if (facts.task) items.push({ id: "review-task", label: "Review changes", shortcut: shortcut("review-task") });
  items.push({ separator: true }, { id: "close", label: "Close", shortcut: shortcut("close") });
  return items;
}

/**
 * The second menu Mode ▸ opens: the modes the agent offers (the five for an adapter), the one the session started in marked
 * "(current)". The current one stays in the list (the places of the others don't move) and the window leaves it out of the run.
 */
export function modeMenuItems(current: Mode | undefined, offered: readonly Mode[] = MODES): MenuItem[] {
  return MODES.filter((m) => offered.includes(m)).map((m) => ({ id: `mode-${m}`, label: m === current ? `${modeLabel(m)} (current)` : modeLabel(m) }));
}
