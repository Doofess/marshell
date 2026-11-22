/** How long after the last key an auto save runs. */
export const AUTO_SAVE_MS = 1000;

/** What decides whether an auto save may run now. */
export type AutoSaveFacts = {
  /** The setting is on. */
  on: boolean;
  dirty: boolean;
  /** The changed-on-disk bar is up. */
  conflict: boolean;
  /** The text still holds conflict markers (`countConflicts(text) > 0`): a merge that left some is saved only by Ctrl+S. */
  markers: boolean;
  readOnly: boolean;
  /** A save or an undo of one is in flight. */
  saving: boolean;
};

/** Whether an auto save may run now. It never runs in a conflict, on text with conflict markers, or on a read-only tab. */
export function autoSaveDue(f: AutoSaveFacts): boolean {
  return f.on && f.dirty && !f.saving && !f.conflict && !f.markers && !f.readOnly;
}

/**
 * Whether the 1,000 ms timer runs: the tab is idle and an auto save could run. `lastSent` is the text of the last save sent: a tab
 * that is dirty with that same text came back from a refused save, and trying it again each second would say the failure each
 * second, so it waits for the next change.
 */
export function autoSaveTimer(f: AutoSaveFacts & { text: string; lastSent: string | null }): boolean {
  return autoSaveDue(f) && f.text !== f.lastSent;
}
