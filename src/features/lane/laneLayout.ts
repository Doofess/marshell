import type { ApprovalRequest } from "../approval/types";

/** docs/PLAN.md "Layout": header always 28 px; the targeted card expands (up to 176 px), the others are 36 px one-liners; at most 40% of the sidebar. */
export const LANE = { header: 28, expanded: 176, collapsed: 36, maxShare: 0.4 } as const;

export const pending = (items: ApprovalRequest[]): ApprovalRequest[] => items.filter((r) => r.state.phase === "pending");

/** Oldest waiting first (docs/PLAN.md section 7: first in, first out). Ties break by id. */
export function orderLane(items: ApprovalRequest[]): ApprovalRequest[] {
  return [...items].sort((a, b) => b.waitingMs - a.waitingMs || a.id.localeCompare(b.id));
}

export function laneHeading(count: number): string {
  return count > 0 ? `Needs you · ${count}` : "All clear";
}

export function laneLayout(count: number, sidebarHeight: number): { height: number; scrolls: boolean; hiddenCount: number } {
  const n = Number.isFinite(count) ? Math.max(0, Math.floor(count)) : 0;
  if (n === 0) return { height: LANE.header, scrolls: false, hiddenCount: 0 };
  const max = Number.isFinite(sidebarHeight) && sidebarHeight > 0 ? Math.floor(sidebarHeight * LANE.maxShare) : 0;
  const natural = LANE.header + LANE.expanded + (n - 1) * LANE.collapsed;
  if (natural <= max) return { height: natural, scrolls: false, hiddenCount: 0 };
  const fits = Math.max(0, Math.floor((max - LANE.header - LANE.expanded) / LANE.collapsed));
  return { height: Math.max(LANE.header, max), scrolls: true, hiddenCount: Math.max(0, n - 1 - fits) };
}
