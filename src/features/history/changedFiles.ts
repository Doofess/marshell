import type { Conversation } from "../../generated/Conversation";
import type { ToolCard } from "../../generated/ToolCard";

export type ChangedFile = {
  path: string;
  /** Total lines added over every edit; null when any edit's count is unknown. */
  added: number | null;
  removed: number | null;
  /** The first edit created the file. */
  created: boolean;
  /** `turn` is the turn's index, the way `Turn.index` counts (from 0). */
  edits: { turn: number; card: ToolCard }[];
};

const norm = (path: string) => path.replace(/\\/g, "/");

/** The files Claude Code edited, grouped by path (either separator), in the order each was first edited. */
export function filesChanged(c: Conversation): ChangedFile[] {
  const byPath = new Map<string, ChangedFile>();
  for (const turn of c.turns) {
    for (const item of turn.items) {
      if (item.kind !== "tool" || !item.card.file) continue;
      const { file } = item.card;
      const path = norm(file.path);
      let entry = byPath.get(path);
      if (!entry) {
        entry = { path, added: 0, removed: 0, created: file.created, edits: [] };
        byPath.set(path, entry);
      }
      entry.edits.push({ turn: turn.index, card: item.card });
      entry.added = entry.added === null || file.added === undefined ? null : entry.added + file.added;
      entry.removed = entry.removed === null || file.removed === undefined ? null : entry.removed + file.removed;
    }
  }
  return [...byPath.values()];
}

/** One edit as a unified diff `DiffView` can read: file headers, one hunk header with the counts, then the card's lines. */
export function editDiff(path: string, card: ToolCard): string {
  const name = norm(path);
  const head = [`--- a/${name}`, `+++ b/${name}`];
  if (!card.detail) return head.join("\n");
  const created = card.file?.created === true;
  const lines = card.detail.split("\n").map((l) => (created ? `+${l}` : l));
  const added = lines.filter((l) => l.startsWith("+")).length;
  const removed = lines.filter((l) => l.startsWith("-")).length;
  const old = created ? 0 : lines.length - added;
  const hunk = `@@ -${created ? 0 : 1},${old} +1,${lines.length - removed} @@`;
  return [...head, hunk, ...lines].join("\n");
}
