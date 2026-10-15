/**
 * Rows never move while the pointer is over the board: keep every row where it was, append new ones, and keep a row
 * that disappeared in place, marked stale, until the pointer leaves.
 */
export function freezeOrder(current: string[], incoming: string[]): { order: string[]; stale: string[] } {
  const now = new Set(incoming);
  const fresh = incoming.filter((id) => !current.includes(id));
  return { order: [...current, ...fresh], stale: current.filter((id) => !now.has(id)) };
}
