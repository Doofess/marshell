import type { RowModel } from "../sidebar/types";

const APP = "Marshell";

/** What the OS shows in the title bar and the taskbar: the focused session first, like any window named for its document. */
export function windowTitle(row: RowModel | null): string {
  const name = row?.name.trim();
  return name ? `${name} — ${APP}` : APP;
}
