/** What covers the window while a file is dropped on it, in the announcement's words; null when nothing does. */
export function dropCover(s: { launcherOpen: boolean; overlay: string | null; review: boolean; closeAsk: boolean }): string | null {
  if (s.launcherOpen) return "the launcher";
  if (s.overlay === "palette") return "the command palette";
  if (s.overlay === "settings") return "Settings";
  if (s.overlay !== null || s.review || s.closeAsk) return "the dialog";
  return null;
}
