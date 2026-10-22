type CloseChoiceDeps = {
  save?: (keep: boolean) => Promise<void>;
  isOpen: () => boolean;
  closeWindow: () => Promise<void>;
  closeOnce: (keep: boolean) => Promise<void>;
};

/** A failed save still carries out this close; Escape during the save cancels either path. */
export async function chooseClose(keepInTray: boolean, deps: CloseChoiceDeps): Promise<"closed" | "closed-once" | "cancelled"> {
  let saved = false;
  if (deps.save) {
    try {
      await deps.save(keepInTray);
      saved = true;
    } catch {
      // The shell can carry out the choice once without persisting it.
    }
  }
  if (!deps.isOpen()) return "cancelled";
  if (saved) {
    await deps.closeWindow();
    return "closed";
  }
  await deps.closeOnce(keepInTray);
  return "closed-once";
}
