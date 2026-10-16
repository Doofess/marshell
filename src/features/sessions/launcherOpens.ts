/**
 * Whether the window opens the launcher by itself: only when the core has sent its list and the list is empty (a first run, or
 * everything closed). A core that restarted seeds the saved tabs as Ended, so a restart with tabs to show leaves it closed.
 */
export function launcherOpens(loaded: boolean, rowCount: number): boolean {
  return loaded && rowCount === 0;
}
