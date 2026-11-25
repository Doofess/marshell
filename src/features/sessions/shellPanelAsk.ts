/** What the shell panel asks the core for: a session's folder, the default folder (null), or nothing yet. */
export function shellPanelAsk(i: {
  shown: boolean;
  sessions: number;
  focusedCwd: string | undefined;
}): { cwd: string | null } | null {
  if (!i.shown) return null;
  if (i.focusedCwd !== undefined) return { cwd: i.focusedCwd };
  // Sessions exist but none is focused yet: wait for its folder rather than jump to the default folder.
  if (i.sessions === 0) return { cwd: null };
  return null;
}
