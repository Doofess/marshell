import { moveBy } from "./reorder";
import type { RowModel } from "./types";

/** The sessions of one project. `start` is the group's first index in the displayed list (the groups one after another). */
export type Group = { project: string; rows: RowModel[]; start: number };

/** Rows grouped by project. Groups are ordered by their first row's position, and rows keep the core's order inside a group. */
export function groupRows(rows: RowModel[]): Group[] {
  const byProject = new Map<string, RowModel[]>();
  for (const row of rows) {
    const group = byProject.get(row.projectKey);
    if (group) group.push(row);
    else byProject.set(row.projectKey, [row]);
  }
  let start = 0;
  return [...byProject.values()].map((rs) => {
    const group = { project: rs[0]?.project ?? "", rows: rs, start };
    start += rs.length;
    return group;
  });
}

/** The displayed order while grouped: the groups one after another. */
export const groupedOrder = (groups: Group[]): RowModel[] => groups.flatMap((g) => g.rows);

/** The bounds of the group holding `id`, in displayed indices: `end` is one past its last row. */
export function groupBounds(groups: Group[], id: string): { start: number; end: number } | undefined {
  const g = groups.find((x) => x.rows.some((r) => r.id === id));
  return g && { start: g.start, end: g.start + g.rows.length };
}

/** The displayed order after moving `id` one place inside its project group, or null at the group's edge (or for an unknown id). */
export function moveWithinGroup(rows: RowModel[], id: string, by: -1 | 1): string[] | null {
  const groups = groupRows(rows);
  const bounds = groupBounds(groups, id);
  return bounds ? moveBy(groupedOrder(groups).map((r) => r.id), id, by, bounds) : null;
}
