import type { TreePosition } from "../../generated/TreePosition";

/** The tree's sizes in px: 240 at first, 160 at least, 2,048 at most, the file keeps 240; narrow below 480 px of panel. */
export const TREE = { min: 160, start: 240, max: 2048, fileMin: 240, step: 16, narrow: 480 } as const;

/** Automatic puts the tree at the window's edge: right in a left-to-right layout, left in a right-to-left one. */
export function treeSide(position: TreePosition, rtl: boolean): "left" | "right" {
  if (position === "automatic") return rtl ? "left" : "right";
  return position;
}

/** The tree's width: the saved one or the first, clamped so the file keeps its room, and never under the minimum. */
export function treeWidth(saved: number | null | undefined, panelWidth: number): number {
  const w = saved ?? TREE.start;
  return Math.max(TREE.min, Math.min(w, TREE.max, panelWidth - TREE.fileMin));
}

/** The width after a key on the divider, or null for a key it doesn't use. Toward the file grows the tree. */
export function dividerKey(key: string, width: number, panelWidth: number, side: "left" | "right"): number | null {
  const grow = side === "left" ? "ArrowRight" : "ArrowLeft";
  const shrink = side === "left" ? "ArrowLeft" : "ArrowRight";
  if (key === grow) return treeWidth(width + TREE.step, panelWidth);
  if (key === shrink) return treeWidth(width - TREE.step, panelWidth);
  if (key === "Home") return TREE.min;
  if (key === "End") return treeWidth(TREE.max, panelWidth);
  return null;
}
