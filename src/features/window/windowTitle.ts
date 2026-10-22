import type { RowModel } from "../sidebar/types";

const APP = "Marshell";

/**
 * What the OS shows in the title bar and the taskbar: the focused session first, like any window named for its document.
 * With sessions waiting, the needs-you count leads, so Alt+Tab and the taskbar's hover text show it.
 */
export function windowTitle(row: RowModel | null, waiting: number): string {
  const name = row?.name.trim();
  const base = name ? `${name} — ${APP}` : APP;
  return waiting > 0 ? `(${waiting}) ${base}` : base;
}
