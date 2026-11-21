import type { MenuItem } from "../../components/ContextMenu/menuKeys";
import type { Os } from "../../lib/keymap";
import { revealLabel } from "./reveal";
import type { TreeRow } from "./treeModel";

export type TreeChoice = "open" | "paste" | "copy-path" | "copy-relative" | "reveal";

/**
 * A row's menu: Open (a file or a link), Paste path into the session (what dropping the file on the terminal does),
 * the two copies and the file manager. The tree writes nothing, so there is no more. "more" and "empty" rows have none.
 */
export function treeMenuItems(row: TreeRow, o: { pasteTo: { name: string; refused?: string } | null; os: Os }): MenuItem[] {
  if (row.kind === "more" || row.kind === "empty") return [];
  const first: MenuItem[] = [];
  if (row.kind !== "folder") first.push({ id: "open" satisfies TreeChoice, label: "Open" });
  if (o.pasteTo) {
    first.push({
      id: "paste" satisfies TreeChoice,
      label: `Paste path into ${o.pasteTo.name}`,
      ...(o.pasteTo.refused ? { disabled: o.pasteTo.refused } : {}),
    });
  }
  const items: MenuItem[] = [];
  if (first.length > 0) items.push(...first, { separator: true });
  items.push(
    { id: "copy-path" satisfies TreeChoice, label: "Copy path" },
    { id: "copy-relative" satisfies TreeChoice, label: "Copy relative path" },
    { separator: true },
    { id: "reveal" satisfies TreeChoice, label: revealLabel(o.os) },
  );
  return items;
}
