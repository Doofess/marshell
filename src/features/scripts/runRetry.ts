import type { RepoScripts } from "../../generated/RepoScripts";

/** What the header's Run does after it re-reads a file. */
export type RereadAction = { kind: "ask"; scripts: RepoScripts } | { kind: "start"; scripts: RepoScripts } | { kind: "close" };

/**
 * Whether a refused run asks again: a 409 (the file changed since it was shown) or a 403 (trust for the repo was replaced since)
 * re-reads the file and shows the card, once. A retried call that is refused again announces the error and stops.
 */
export function askOnRunFailure(status: number | undefined, retried: boolean): boolean {
  return !retried && (status === 409 || status === 403);
}

/**
 * What to do with the re-read file: an untrusted one asks, one that is trusted now starts (the person already allowed it), and
 * one with no run script, a problem or no hash closes the card.
 */
export function afterReread(again: RepoScripts | null): RereadAction {
  if (again === null || again.run === null || again.problem !== null || again.hash === null) return { kind: "close" };
  return again.trusted ? { kind: "start", scripts: again } : { kind: "ask", scripts: again };
}
