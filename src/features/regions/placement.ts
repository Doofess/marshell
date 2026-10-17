export type Rect = { left: number; top: number; width: number; height: number };

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(hi, lo), Math.max(lo, v));

/** Below the box if the popover fits there, else above, else the side with more room; left-aligned with the box and kept inside `bounds`. */
export function placePopover(box: Rect, pop: { w: number; h: number }, bounds: Rect, gap = 8): { left: number; top: number; side: "below" | "above" } {
  const below = box.top + box.height + gap;
  const above = box.top - gap - pop.h;
  const roomBelow = bounds.top + bounds.height - below;
  const roomAbove = box.top - gap - bounds.top;
  const side: "below" | "above" = roomBelow >= pop.h ? "below" : roomAbove >= pop.h ? "above" : roomBelow >= roomAbove ? "below" : "above";
  const top = clamp(side === "below" ? below : above, bounds.top, bounds.top + bounds.height - pop.h);
  const left = clamp(box.left, bounds.left, bounds.left + bounds.width - pop.w);
  return { left, top, side };
}
