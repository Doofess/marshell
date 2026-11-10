import type { MenuItem } from "../../components/ContextMenu/ContextMenu";
import { splitPath } from "../preview/kinds";

const OPEN_FILE = "open-file";
const BROWSE = "browse";
const RECENT = "recent:";

/** What the + menu offers: "Open file…", "Browse…", then one row per recent file, named by the file and its folder. */
export function plusMenuItems(recent: readonly string[]): MenuItem[] {
  return [
    { id: OPEN_FILE, label: "Open file…" },
    { id: BROWSE, label: "Browse…" },
    ...recent.map((path): MenuItem => {
      const { name, folder } = splitPath(path);
      return { id: `${RECENT}${path}`, label: folder ? `${name} — ${folder}` : name };
    }),
  ];
}

export type PlusChoice = { kind: "openFile" } | { kind: "browse" } | { kind: "recent"; path: string } | null;

/** What a chosen menu row stands for; null for an id the menu never had (a recent file that is no longer listed included). */
export function plusChoice(id: string, recent: readonly string[]): PlusChoice {
  if (id === OPEN_FILE) return { kind: "openFile" };
  if (id === BROWSE) return { kind: "browse" };
  if (id.startsWith(RECENT)) {
    const path = id.slice(RECENT.length);
    return recent.includes(path) ? { kind: "recent", path } : null;
  }
  return null;
}
