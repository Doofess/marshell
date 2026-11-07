import { MOVED_COMMANDS, type Command } from "../../lib/keymap";
import { RECENT_LIMIT } from "../window/viewState";

/** The five session modes, which sit behind "Change mode…" in the empty box. */
export const MODE_IDS: readonly string[] = ["mode-manual", "mode-plan", "mode-auto-edit", "mode-full-auto", "mode-bypass"];

const GROUP_ORDER: Command["group"][] = ["Session", "Terminal", "Window", "App"];

/**
 * The empty box's commands: `recent` is the saved ids that are offered and not moved, in order, no duplicates, at most RECENT_LIMIT;
 * `rest` is everything else but the moved items, the mode rows and what `recent` shows, stable-sorted by group.
 */
export function emptyBox(commands: readonly Command[], recent: readonly string[], moved: readonly string[]): { recent: Command[]; rest: Command[] } {
  const byId = new Map(commands.map((c) => [c.id, c]));
  const shown: Command[] = [];
  for (const id of recent) {
    const c = byId.get(id);
    if (!c || moved.includes(id) || shown.includes(c)) continue;
    shown.push(c);
    if (shown.length >= RECENT_LIMIT) break;
  }
  const rank = (c: Command) => GROUP_ORDER.indexOf(c.group);
  const rest = commands
    .filter((c) => !moved.includes(c.id) && !MODE_IDS.includes(c.id) && !shown.includes(c))
    .map((c, i) => ({ c, i }))
    .sort((a, b) => rank(a.c) - rank(b.c) || a.i - b.i)
    .map((x) => x.c);
  return { recent: shown, rest };
}

/** "Change mode…" is offered only while some mode row is. */
export function withModeStep(commands: readonly Command[]): Command[] {
  return commands.some((c) => MODE_IDS.includes(c.id)) ? [...commands] : commands.filter((c) => c.id !== "change-mode");
}

/** Whether running this id from the palette goes in Recent. */
export function recordsRecent(id: string): boolean {
  return !id.startsWith("session:") && !id.startsWith("file:") && id !== "palette" && id !== "change-mode" && !MOVED_COMMANDS.includes(id);
}

/** Esc in the mode list goes back to the commands; in the commands it closes. */
export function paletteEscape(list: "commands" | "modes"): "back" | "close" {
  return list === "modes" ? "back" : "close";
}
