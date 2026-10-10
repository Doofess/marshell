import type { ApprovalRequest } from "../approval/types";

/** docs/PLAN.md "Layout": header always 28 px; the targeted card expands (up to 256 px, the measured height of the real card), the others are 36 px one-liners; at most 40% of the sidebar, in whole rows (it never scrolls). */
export const LANE = { header: 28, expanded: 256, collapsed: 36, maxShare: 0.4 } as const;

export const pending = (items: ApprovalRequest[]): ApprovalRequest[] => items.filter((r) => r.state.phase === "pending");

/** Oldest waiting first (docs/PLAN.md section 7: first in, first out). Ties break by id. */
export function orderLane(items: ApprovalRequest[]): ApprovalRequest[] {
  return [...items].sort((a, b) => b.waitingMs - a.waitingMs || a.id.localeCompare(b.id));
}

export function laneHeading(count: number): string {
  return count > 0 ? `Needs you · ${count}` : "All clear";
}

export type LaneLayout = {
  height: number;
  /** True when the lane shows less than its natural layout; never a scroll, always a "+N more" button. */
  clipped: boolean;
  hiddenCount: number;
  /** Whether the oldest request is the full card (it is not when the 40% cap is shorter than the card). */
  expanded: boolean;
  /** How many requests are on screen. */
  visible: number;
};

/**
 * The lane never scrolls. It shows whole rows up to 40% of the sidebar and leaves the rest behind a "+N more" button.
 * When even one card would not fit in 40%, it shows one-liners only.
 */
export function laneLayout(count: number, sidebarHeight: number): LaneLayout {
  const n = Number.isFinite(count) ? Math.max(0, Math.floor(count)) : 0;
  if (n === 0) return { height: LANE.header, clipped: false, hiddenCount: 0, expanded: false, visible: 0 };
  const max = Number.isFinite(sidebarHeight) && sidebarHeight > 0 ? Math.floor(sidebarHeight * LANE.maxShare) : 0;
  const natural = LANE.header + LANE.expanded + (n - 1) * LANE.collapsed;
  if (natural <= max) return { height: natural, clipped: false, hiddenCount: 0, expanded: true, visible: n };
  if (max >= LANE.header + LANE.expanded) {
    const fits = Math.floor((max - LANE.header - LANE.expanded) / LANE.collapsed);
    const visible = 1 + fits;
    return { height: LANE.header + LANE.expanded + fits * LANE.collapsed, clipped: true, hiddenCount: n - visible, expanded: true, visible };
  }
  const visible = Math.min(n, Math.max(0, Math.floor((max - LANE.header) / LANE.collapsed)));
  return { height: LANE.header + visible * LANE.collapsed, clipped: true, hiddenCount: n - visible, expanded: false, visible };
}
