import type { ChangesReply } from "../../generated/ChangesReply";
import type { ChangesRequest } from "../../generated/ChangesRequest";
import type { CommentsReply } from "../../generated/CommentsReply";
import type { ReviewReply } from "../../generated/ReviewReply";
import { ApiError, api, getEndpoint } from "../../lib/api/client";

const session = (tab: string) => `/v1/sessions/${encodeURIComponent(tab)}`;

/** The changes of the session's project folder. A 404 means the session has no folder: a value, not an error. */
export async function readChanges(tab: string, req: ChangesRequest): Promise<ChangesReply | "no-folder"> {
  const ep = await getEndpoint();
  try {
    return await api<ChangesReply>(ep, "POST", `${session(tab)}/changes`, req);
  } catch (e) {
    if (e instanceof ApiError && e.status === 404) return "no-folder";
    throw e;
  }
}

export type Refusal = "ended" | "elevated" | "paused" | "waiting" | "no_agent";
const REFUSALS: readonly string[] = ["ended", "elevated", "paused", "waiting", "no_agent"];

/** Why the core refused to type into the session (a 409 with `{ reason }`), or null for any other failure. */
export function refusalOf(e: unknown): Refusal | null {
  if (!(e instanceof ApiError) || e.status !== 409) return null;
  try {
    const reason = (JSON.parse(e.message) as { reason?: unknown } | null)?.reason;
    return typeof reason === "string" && REFUSALS.includes(reason) ? (reason as Refusal) : null;
  } catch {
    return null;
  }
}

/** The text that asks the session's agent to review the changes. A 404 or 409 throws an `ApiError` whose message is the core's sentence. */
export async function askReview(tab: string, base: string | null): Promise<ReviewReply> {
  return api<ReviewReply>(await getEndpoint(), "POST", `${session(tab)}/review`, { base });
}

/** Turns the typed comments message into the form to hand the agent. A 404 or 409 throws an `ApiError` whose message is the core's sentence. */
export async function postComments(tab: string, text: string): Promise<CommentsReply> {
  return api<CommentsReply>(await getEndpoint(), "POST", `${session(tab)}/comments`, { text });
}
