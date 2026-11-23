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

/** The text that asks the session's agent to review the changes. A 404 or 409 throws an `ApiError` whose message is the core's sentence. */
export async function askReview(tab: string, base: string | null): Promise<ReviewReply> {
  return api<ReviewReply>(await getEndpoint(), "POST", `${session(tab)}/review`, { base });
}

/** Turns the typed comments message into the form to hand the agent. A 404 or 409 throws an `ApiError` whose message is the core's sentence. */
export async function postComments(tab: string, text: string): Promise<CommentsReply> {
  return api<CommentsReply>(await getEndpoint(), "POST", `${session(tab)}/comments`, { text });
}
