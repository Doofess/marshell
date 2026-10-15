import type { RowModel } from "../sidebar/types";
import type { PaletteSession } from "./fixtures";

/** The sidebar's rows as the palette lists them: number, name, folder, what runs, and what it is doing. The number is never reassigned here. */
export function paletteSessions(rows: RowModel[]): PaletteSession[] {
  return rows.map((r) => ({ id: r.id, number: r.number, name: r.name, project: r.project, branch: r.branch, kind: r.agent, status: r.status }));
}
