import type { Endpoint } from "../../generated/Endpoint";
import type { MergeOutcome } from "../../generated/MergeOutcome";
import type { MergePlan } from "../../generated/MergePlan";
import type { MergeRequest } from "../../generated/MergeRequest";
import type { RemovePlan } from "../../generated/RemovePlan";
import type { RemoveRequest } from "../../generated/RemoveRequest";
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
  remove(req: RemoveRequest): Promise<void>;
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

/** The request that asks for the commands: no commands yet, because none have been shown. */
export const mergePlanRequest = (message: string, remove: boolean): MergeRequest => ({ message, remove, commands: [] });

/**
 * The request to merge, built from the plan on screen and nothing else (what is shown is what runs). It is null unless that plan
 * is for this very message and checkbox and has no blocker.
 */
export function mergeRequest(message: string, remove: boolean, shown: { key: string; plan: MergePlan } | null): MergeRequest | null {
  if (!shown || shown.key !== planKey(message, remove) || shown.plan.blocker) return null;
  return { message, remove, commands: shown.plan.commands };
}

/** The request to remove, built from the plan on screen: its commands and its warnings, as shown. */
export const removeRequest = (shown: RemovePlan | null): RemoveRequest | null => (shown ? { commands: shown.commands, warnings: shown.warnings } : null);

export const conflictHeading = (files: number, base: string): string =>
  `Merging stopped: ${files} ${files === 1 ? "file conflicts" : "files conflict"} with ${base}. Nothing on ${base} was changed.`;

/** One line, because the core refuses text with a line break, and Marshell never sends Enter: the user reads it and presses Enter in the agent. */
export function conflictPrompt(task: Task, files: string[]): string {
  const names = files.map((f) => f.replace(/[\r\n]+/g, " ")).join(", ");
  return `Merging ${task.branch} into ${task.base_branch} conflicts in ${names}. Rebase this branch on ${task.base_branch} and resolve those conflicts, then commit.`;
}

export const mergedToast = (task: Task, removed: boolean): string => `Merged ${task.branch} into ${task.base_branch}.${removed ? " Removed the worktree." : ""}`;
export const removedToast = (task: Task): string => `Removed ${task.branch}.`;
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

export async function runRemove(api: ReviewApi, req: RemoveRequest): Promise<{ ok: true } | { ok: false; reason: string }> {
  try {
    await api.remove(req);
    return { ok: true };
  } catch (e) {
    return { ok: false, reason: e instanceof ApiError && e.status === 409 ? e.message : `Couldn't remove: ${detail(e)}` };
  }
}
