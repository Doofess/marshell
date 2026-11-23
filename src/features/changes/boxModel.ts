import type { DraftComment } from "../../generated/DraftComment";
import { place } from "./comments";
import type { Item, Section } from "./model";

export type BoxAnchor = Omit<DraftComment, "id" | "text">;

/** The comment box: the rows it is about and, when it edits a comment, that comment's id. */
export type BoxState = { anchor: BoxAnchor; id: string | null };

/** The box's React key: path, side, start and end. A reply that moves lines never changes it, so the box keeps its text and caret. */
export const anchorKey = (a: BoxAnchor): string => `${a.path}\u0000${a.side}\u0000${a.start}\u0000${a.end}`;

/** The index of the item the box is drawn under: the comment it edits, else the range's last row as `place` finds it now; -1 when the file is gone. */
export function boxAfter(items: Item[], sections: Section[], box: BoxState): number {
  if (box.id !== null) {
    const at = items.findIndex((i) => i.key === `comment:${box.id}`);
    if (at >= 0) return at;
  }
  const file = sections.find((s) => s.path === box.anchor.path);
  if (file === undefined) return -1;
  const placed = place({ ...box.anchor, id: "", text: "" }, file);
  if (placed.at !== null) return items.findIndex((i) => i.kind === "row" && i.file.path === file.path && i.at === placed.at);
  // Above the first row: under the file's header (and its note).
  const head = items.findIndex((i) => i.kind === "file" && i.file.path === file.path);
  return head >= 0 && items[head + 1]?.kind === "note" ? head + 1 : head;
}
