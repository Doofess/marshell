import type { RepoScripts } from "../../generated/RepoScripts";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { SetupOutcome } from "../../generated/SetupOutcome";

/** What the announcer says when the new worktree's `marshell.json` has no setup, though the confirm view showed one. */
export const NO_WORKTREE_SETUP = "The new worktree's marshell.json has no setup script, so none ran.";
/** When the worktree's file could not be read at all. */
export const WORKTREE_UNREADABLE = "Couldn't read the new worktree's marshell.json, so no setup ran.";

/**
 * How a setup tab stands: null while it runs; Passed for exit 0; Failed{code} for any other exit; Closed when the tab is gone
 * (the person closed it) or its session Ended.
 */
export function setupOutcome(info: SessionInfo | undefined): SetupOutcome | null {
  if (info === undefined) return { kind: "closed" };
  switch (info.state.kind) {
    case "running":
      return null;
    case "exited":
      return info.state.code === 0 ? { kind: "passed" } : { kind: "failed", code: info.state.code };
    case "ended":
      return { kind: "closed" };
  }
}

/** A setup tab whose agent is waiting to start. `seen` is set once the session list has listed the tab. */
export type PendingSetup = { seen: boolean };

/**
 * Which waiting agents can start now. `runScript` answers before the next session list names the new tab, so a tab missing from the
 * list counts as closed only after the list has shown it once. `seen` lists tabs the list shows now (the caller marks them).
 */
export function readyPending(pending: ReadonlyMap<string, PendingSetup>, infos: SessionInfo[]): { seen: string[]; ready: { tabId: string; outcome: SetupOutcome }[] } {
  const seen: string[] = [];
  const ready: { tabId: string; outcome: SetupOutcome }[] = [];
  for (const [tabId, entry] of pending) {
    const info = infos.find((i) => i.tab_id === tabId);
    if (info === undefined && !entry.seen) continue;
    if (info !== undefined) seen.push(tabId);
    const outcome = setupOutcome(info);
    if (outcome !== null) ready.push({ tabId, outcome });
  }
  return { seen, ready };
}

/** What to do once a task's worktree exists and a setup was going to run (the confirm view showed one, trusted or just allowed). */
export type AfterCreate = { kind: "run"; hash: string } | { kind: "ask"; scripts: RepoScripts } | { kind: "none"; announce: string };

/**
 * The confirm view read the main checkout's `marshell.json`, but the worktree holds its own committed copy. `shownHash` is the
 * hash the person saw; `fresh` is the worktree's file read after the task was made (null when it could not be read).
 * - the same file, or one already allowed, with a setup: run it;
 * - a different file with a setup: ask again for that file;
 * - no setup (or a broken file): no setup runs, and the announcer says so.
 */
export function afterCreate(shownHash: string | null, fresh: RepoScripts | null): AfterCreate {
  if (fresh === null) return { kind: "none", announce: WORKTREE_UNREADABLE };
  if (fresh.problem !== null) return { kind: "none", announce: fresh.problem };
  if (fresh.setup === null || fresh.hash === null) return { kind: "none", announce: NO_WORKTREE_SETUP };
  if (fresh.hash === shownHash || fresh.trusted) return { kind: "run", hash: fresh.hash };
  return { kind: "ask", scripts: fresh };
}
