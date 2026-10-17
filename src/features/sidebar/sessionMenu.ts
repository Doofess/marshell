import type { MenuItem } from "../../components/ContextMenu/menuKeys";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";

export type SessionMenuFacts = {
  resumable: boolean;
  task: boolean;
  inShownPane: boolean;
  paneAvailable: boolean;
};

/** The items a session row's menu shows. Items that don't apply are left out, never disabled. */
export function sessionMenuItems(facts: SessionMenuFacts, os: Os): MenuItem[] {
  const shortcut = (id: string) => shortcutLabel(commandById(id).binding!, os);
  const items: MenuItem[] = [{ id: "rename", label: "Rename", shortcut: shortcut("rename") }];
  if (!facts.inShownPane && facts.paneAvailable) items.push({ id: "split", label: "Split into a pane" });
  if (facts.resumable) items.push({ id: "resume", label: "Resume" });
  if (facts.task) items.push({ id: "review-task", label: "Review changes", shortcut: shortcut("review-task") });
  items.push({ separator: true }, { id: "close", label: "Close", shortcut: shortcut("close") });
  return items;
}
