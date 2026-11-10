/**
 * What covers the window while a file is dropped on it, in the announcement's words; null when nothing does. `modal` is any
 * other modal layer on screen (the scripts question, the first-run setup), so a new dialog needs no line here.
 */
export function dropCover(s: { launcherOpen: boolean; overlay: string | null; review: boolean; closeAsk: boolean; modal: boolean }): string | null {
  if (s.launcherOpen) return "the launcher";
  if (s.overlay === "palette") return "the command palette";
  if (s.overlay === "settings") return "Settings";
  if (s.overlay !== null || s.review || s.closeAsk || s.modal) return "the dialog";
  return null;
}

/** Whether a modal layer is on screen: a dialog that says so, or the first-run setup. */
export const modalOnScreen = (doc: Pick<Document, "querySelector">): boolean => doc.querySelector('[aria-modal="true"], .live-app__onboarding') !== null;
