import type { Endpoint } from "../../generated/Endpoint";
import type { MergeOutcome } from "../../generated/MergeOutcome";
import type { MergePlan } from "../../generated/MergePlan";
import type { MergeRequest } from "../../generated/MergeRequest";
import type { RemovePlan } from "../../generated/RemovePlan";
import type { RemoveRequest } from "../../generated/RemoveRequest";
import type { ResumeError } from "../../generated/ResumeError";
import type { Task } from "../../generated/Task";
import type { TaskChanges } from "../../generated/TaskChanges";
import type { TaskPlan } from "../../generated/TaskPlan";
import type { TaskPlanRequest } from "../../generated/TaskPlanRequest";
import { ApiError, api } from "./client";

const task = (id: string) => `/v1/tasks/${encodeURIComponent(id)}`;

/**
 * The core answers a refusal with 409 and `{ reason }`. That is re-thrown as an ApiError whose message is the reason alone,
 * so `startFailure` and the sheets show the core's own sentence. Any other failure passes through untouched.
 */
async function refusal<T>(call: Promise<T>): Promise<T> {
  try {
    return await call;
  } catch (e) {
    if (e instanceof ApiError && e.status === 409) {
      try {
        const body = JSON.parse(e.message) as Partial<ResumeError>;
        if (typeof body.reason === "string") throw new ApiError(409, body.reason);
      } catch (inner) {
        if (inner instanceof ApiError) throw inner;
        // Not our JSON body: keep the plain text.
      }
    }
    throw e;
  }
}

/**
 * Task worktrees. Merge and remove requests carry the commands (and, for remove, the warnings) the user was shown; the core
 * refuses when a fresh plan differs. A 404 (an id the core does not know) rejects with an ApiError.
 */
export const tasksApi = {
  list: (ep: Endpoint) => api<Task[]>(ep, "GET", "/v1/tasks"),
  /** What creating the task would run. Rejects with a 409 ApiError (the reason) when it cannot be made. */
  plan: (ep: Endpoint, req: TaskPlanRequest) => refusal(api<TaskPlan>(ep, "POST", "/v1/tasks/plan", req)),
  create: (ep: Endpoint, req: TaskPlanRequest) => refusal(api<Task>(ep, "POST", "/v1/tasks", req)),
  changes: (ep: Endpoint, id: string) => refusal(api<TaskChanges>(ep, "GET", `${task(id)}/changes`)),
  mergePlan: (ep: Endpoint, id: string, req: MergeRequest) => api<MergePlan>(ep, "POST", `${task(id)}/merge/plan`, req),
  /** A conflict or a git failure comes back as the outcome; only a refusal to start rejects. */
  merge: (ep: Endpoint, id: string, req: MergeRequest) => refusal(api<MergeOutcome>(ep, "POST", `${task(id)}/merge`, req)),
  removePlan: (ep: Endpoint, id: string) => api<RemovePlan>(ep, "POST", `${task(id)}/remove/plan`),
  remove: (ep: Endpoint, id: string, req: RemoveRequest) => refusal(api<void>(ep, "POST", `${task(id)}/remove`, req)),
};
