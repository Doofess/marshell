/** What the shell panel asks the core for: a session's folder, the default folder (null), or nothing yet. `ready` is the window settled and loaded; the panel being shown or not does not matter. */
export function shellPanelAsk(i: {
  ready: boolean;
  sessions: number;
  focusedCwd: string | undefined;
}): { cwd: string | null } | null {
  if (!i.ready) return null;
  if (i.focusedCwd !== undefined) return { cwd: i.focusedCwd };
  // Sessions exist but none is focused yet: wait for its folder rather than jump to the default folder.
  if (i.sessions === 0) return { cwd: null };
  return null;
}
