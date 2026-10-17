/** What one stat poll saw: the file's size and modification time, or that it is gone. */
export type StatResult = { kind: "ok"; size: number; modified_ms: number } | { kind: "missing" };

export type ReloadState = {
  /** The last size and mtime acted on. */
  seen: { size: number; modified_ms: number } | null;
  status: "none" | "loaded" | "tooLarge" | "deleted";
};

export type ReloadAction = "keep" | "refetch" | "deleted" | "tooLarge";

export const START: ReloadState = { seen: null, status: "none" };

/**
 * What to do with one poll's result. `keep` means the view already shows the truth. `deleted` and `tooLarge` are said once
 * per change, not once per second; a file that comes back, or shrinks to the cap, is `refetch` even with an unchanged stat.
 */
export function next(state: ReloadState, stat: StatResult, cap: number): { action: ReloadAction; state: ReloadState } {
  if (stat.kind === "missing") {
    return state.status === "deleted" ? { action: "keep", state } : { action: "deleted", state: { seen: null, status: "deleted" } };
  }
  const unchanged = state.seen !== null && state.seen.size === stat.size && state.seen.modified_ms === stat.modified_ms;
  const seen = { size: stat.size, modified_ms: stat.modified_ms };
  if (stat.size > cap) {
    return state.status === "tooLarge" && unchanged ? { action: "keep", state } : { action: "tooLarge", state: { seen, status: "tooLarge" } };
  }
  return state.status === "loaded" && unchanged ? { action: "keep", state } : { action: "refetch", state: { seen, status: "loaded" } };
}
