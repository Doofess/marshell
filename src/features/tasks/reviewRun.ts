import type { Endpoint } from "../../generated/Endpoint";
import type { MergeOutcome } from "../../generated/MergeOutcome";
import type { MergePlan } from "../../generated/MergePlan";
import type { MergeRequest } from "../../generated/MergeRequest";
import type { RemovePlan } from "../../generated/RemovePlan";
import type { RemoveRequest } from "../../generated/RemoveRequest";
import type { RemoveResult } from "../../generated/RemoveResult";
import type { Task } from "../../generated/Task";
import { ApiError } from "../../lib/api/client";
import { tasksApi } from "../../lib/api/tasks";

/**
 * What the Review sheet needs from the core, for one task. The sheet gets this instead of an endpoint, so a test (or a story)
 * can stand in for the core.
 */
export type ReviewApi = {
  mergePlan(req: MergeRequest): Promise<MergePlan>;
  merge(req: MergeRequest): Promise<MergeOutcome>;
  removePlan(): Promise<RemovePlan>;
  remove(req: RemoveRequest): Promise<RemoveResult>;
};

export const reviewApi = (ep: Endpoint, id: string): ReviewApi => ({
  mergePlan: (req) => tasksApi.mergePlan(ep, id, req),
  merge: (req) => tasksApi.merge(ep, id, req),
  removePlan: () => tasksApi.removePlan(ep, id),
  remove: (req) => tasksApi.remove(ep, id, req),
});

/** What the merge step shows and does, for the view to draw. `ready`: the commands on screen are the plan for the message and checkbox now shown, and no refetch is out. */
export type MergeView = {
  message: string;
  remove: boolean;
  plan: MergePlan | null;
  ready: boolean;
  error: string | null;
  onMessage: (message: string) => void;
  onRemoveChange: (remove: boolean) => void;
  onConfirm: () => void;
};

/** What the remove step shows and does. `ready`: the commands and warnings on screen are the latest plan, and no refetch is out. */
export type RemoveView = {
  plan: RemovePlan | null;
  ready: boolean;
  error: string | null;
  onConfirm: () => void;
};

/** How long the message and the checkbox sit still before the commands are asked for again. */
export const PLAN_DELAY_MS = 250;

/** "fix-login" becomes "Fix login": hyphens are spaces and the first letter is a capital. */
export function readableMessage(name: string): string {
  const spaced = name.replace(/-+/g, " ").trim();
  const [first = ""] = spaced;
  return first.toLocaleUpperCase() + spaced.slice(first.length);
}

/** What a plan is for. A plan whose key differs from the message and checkbox now on screen is stale. */
export const planKey = (message: string, remove: boolean): string => JSON.stringify([message, remove]);

/** The request that asks for the commands: no commands or warnings yet, because none have been shown. */
export const mergePlanRequest = (message: string, remove: boolean): MergeRequest => ({ message, remove, commands: [], warnings: [] });

/**
 * The request to merge, built from the plan on screen and nothing else (what is shown is what runs). It is null unless that plan
 * is for this very message and checkbox and has no blocker.
 */
export function mergeRequest(message: string, remove: boolean, shown: { key: string; plan: MergePlan } | null): MergeRequest | null {
  if (!shown || shown.key !== planKey(message, remove) || shown.plan.blocker) return null;
  const { teardown } = shown.plan;
  return { message, remove, commands: shown.plan.commands, warnings: shown.plan.warnings, ...(teardown === undefined ? {} : { teardown }) };
}

/** The request to remove, built from the plan on screen: its commands and its warnings, as shown. */
export const removeRequest = (shown: RemovePlan | null): RemoveRequest | null =>
  shown ? { commands: shown.commands, warnings: shown.warnings, ...(shown.teardown === undefined ? {} : { teardown: shown.teardown }) } : null;

export const conflictHeading = (files: number, base: string): string =>
  `Merging stopped: ${files} ${files === 1 ? "file conflicts" : "files conflict"} with ${base}. Nothing on ${base} was changed.`;

/** One line, because the core refuses text with a line break, and Marshell never sends Enter: the user reads it and presses Enter in the agent. */
export function conflictPrompt(task: Task, files: string[]): string {
  const names = files.map((f) => f.replace(/[\r\n]+/g, " ")).join(", ");
  return `Merging ${task.branch} into ${task.base_branch} conflicts in ${names}. Rebase this branch on ${task.base_branch} and resolve those conflicts, then commit.`;
}

/** The merge, then the removal: done, or the core's one sentence on why the worktree was kept. */
export const mergedToast = (task: Task, removed: boolean, removalNote: string | null, notes: string[] = []): string =>
  `Merged ${task.branch} into ${task.base_branch}.${removed ? " Removed the worktree." : ""}${removalNote ? ` ${removalNote}` : ""}${notes.map((n) => ` ${n}`).join("")}`;
export const removedToast = (task: Task, notes: string[] = []): string => [`Removed ${task.branch}.`, ...notes].join(" ");
export const removeLabel = (warnings: string[]): string => (warnings.length > 0 ? "Remove anyway" : "Remove");

const detail = (e: unknown): string => (e instanceof ApiError ? e.message : String(e));
export const planFailure = (e: unknown): string => `Couldn't load the commands: ${detail(e)}`;

/**
 * Merges with the request as built from the plan on screen. A refusal (409, such as "The task changed while you were reading the
 * commands. Review them again.") and any other error come back as a failure with one sentence, so the step shows it.
 */
export async function runMerge(api: ReviewApi, req: MergeRequest): Promise<MergeOutcome> {
  try {
    return await api.merge(req);
  } catch (e) {
    return { kind: "failed", reason: e instanceof ApiError && e.status === 409 ? e.message : `Couldn't merge: ${detail(e)}` };
  }
}

export async function runRemove(api: ReviewApi, req: RemoveRequest): Promise<{ ok: true; notes: string[] } | { ok: false; reason: string }> {
  try {
    const result = await api.remove(req);
    return { ok: true, notes: result?.notes ?? [] };
  } catch (e) {
    return { ok: false, reason: e instanceof ApiError && e.status === 409 ? e.message : `Couldn't remove: ${detail(e)}` };
  }
}

/** What the core said about a plan, for which message and checkbox (`key`) and which round of asking (`round`). */
export type PlanLoad<P> = { key: string; round: number; plan: P | null; error: string | null };

/** The remove plan depends on nothing the user types, so every remove load has this one key. */
export const REMOVE_KEY = "remove";

/**
 * The load, if it is for what is on screen now: the same message and checkbox, and the same round (a round passes when a step
 * opens and when a run fails, because the task may have changed). Anything else is stale, and so is no load at all (the debounce
 * is still waiting, or the fetch is out).
 */
export function currentLoad<P>(load: PlanLoad<P> | null, key: string, round: number): PlanLoad<P> | null {
  return load !== null && load.key === key && load.round === round ? load : null;
}

/** Whether Merge or Remove can act: its step is open, its plan is the current one, nothing blocks, and no run has started. */
export function canConfirm({ step, ready, blocker, running }: { step: string; ready: boolean; blocker: string | null; running: boolean }): boolean {
  return (step === "merge" || step === "remove") && ready && !blocker && !running;
}

/** The merge request for a press of Merge: from the current plan only, and null whenever Merge could not have acted. */
export function confirmMergeRequest(a: { step: string; running: boolean; message: string; remove: boolean; key: string; round: number; load: PlanLoad<MergePlan> | null }): MergeRequest | null {
  if (a.step !== "merge" || a.running) return null;
  const current = currentLoad(a.load, a.key, a.round);
  return mergeRequest(a.message, a.remove, current?.plan ? { key: current.key, plan: current.plan } : null);
}

/** The remove request for a press of Remove: the commands and warnings of the current plan, as shown. */
export function confirmRemoveRequest(a: { step: string; running: boolean; round: number; load: PlanLoad<RemovePlan> | null }): RemoveRequest | null {
  if (a.step !== "remove" || a.running) return null;
  return removeRequest(currentLoad(a.load, REMOVE_KEY, a.round)?.plan ?? null);
}

/** Commands are on screen, but they are not the current plan: they are dimmed and say so. */
export const isStale = (shown: unknown | null, ready: boolean): boolean => shown !== null && !ready;

/** The round of asking for plans, and the reason the last run was refused. Every transition is here so it can be tested. */
export type RunState = { round: number; failure: string | null };
export const initialRun: RunState = { round: 0, failure: null };
/** A step opened: every plan from before is stale. */
export const runOpened = (s: RunState): RunState => ({ round: s.round + 1, failure: null });
/** A run was refused or failed (a 409 included): show why, and make every plan stale so the commands are asked for again. */
export const runFailed = (s: RunState, reason: string): RunState => ({ round: s.round + 1, failure: reason });
/** The person changed the message or the checkbox, or went back: the old reason no longer applies. */
export const runEdited = (s: RunState): RunState => (s.failure === null ? s : { ...s, failure: null });
