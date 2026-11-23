import type { AskReply } from "../../generated/AskReply";
import type { AskRequest } from "../../generated/AskRequest";
import type { CreatePull } from "../../generated/CreatePull";
import type { CreatedPull } from "../../generated/CreatedPull";
import type { GithubReply } from "../../generated/GithubReply";
import type { PullAction } from "../../generated/PullAction";
import type { PullMergeRequest } from "../../generated/PullMergeRequest";
import type { PushPlan } from "../../generated/PushPlan";
import type { PushProgress } from "../../generated/PushProgress";
import type { PullThreads } from "../../generated/PullThreads";
import type { PushRequest } from "../../generated/PushRequest";
import type { PushStarted } from "../../generated/PushStarted";
import type { ThreadAction } from "../../generated/ThreadAction";
import type { ThreadsRequest } from "../../generated/ThreadsRequest";
import { ApiError, api, coreFetch, getEndpoint } from "../../lib/api/client";

const session = (tab: string) => `/v1/sessions/${encodeURIComponent(tab)}`;
const push = (id: string) => `/v1/pushes/${encodeURIComponent(id)}`;

/**
 * Why a call failed, as a sentence: the `{ reason }` of a 409 or 503 body, else the plain-text sentence of a 400 or 404,
 * else the error's own message. Every call below throws an `ApiError`; hand it to this.
 */
export function failureOf(e: unknown): { reason: string } {
  if (e instanceof ApiError) {
    if (e.status === 409 || e.status === 503) {
      try {
        const reason = (JSON.parse(e.message) as { reason?: unknown } | null)?.reason;
        if (typeof reason === "string") return { reason };
      } catch {
        // A plain-text body: its text is the reason.
      }
    }
    return { reason: e.message };
  }
  return { reason: e instanceof Error ? e.message : String(e) };
}

/** A call whose answer, if any, isn't read. */
async function send(method: "GET" | "POST", path: string, body?: unknown): Promise<void> {
  const res = await coreFetch(await getEndpoint(), method, path, body);
  if (!res.ok) throw new ApiError(res.status, await res.text());
}

/** What GitHub says about the session's checkout. `fresh: false` recounts locally and doesn't ask GitHub again. */
export async function readGithub(tab: string, fresh: boolean): Promise<GithubReply> {
  return api<GithubReply>(await getEndpoint(), "POST", `${session(tab)}/github`, { fresh });
}

/** What a push (or a pull request's creation) would send, and the pull request's first title and description. */
export async function readPlan(tab: string, create: boolean): Promise<PushPlan> {
  return api<PushPlan>(await getEndpoint(), "POST", `${session(tab)}/github/plan`, { create });
}

export async function startPush(tab: string, req: PushRequest): Promise<PushStarted> {
  return api<PushStarted>(await getEndpoint(), "POST", `${session(tab)}/push`, req);
}

export async function readPush(id: string, from: number): Promise<PushProgress> {
  return api<PushProgress>(await getEndpoint(), "GET", `${push(id)}?from=${from}`);
}

export async function cancelPush(id: string): Promise<void> {
  await send("POST", `${push(id)}/cancel`);
}

/** Stops every push Marshell runs. */
export async function stopPushes(): Promise<void> {
  await send("POST", "/v1/pushes/stop");
}

export async function createPull(tab: string, req: CreatePull): Promise<CreatedPull> {
  return api<CreatedPull>(await getEndpoint(), "POST", `${session(tab)}/github/create`, req);
}

export async function mergePull(tab: string, req: PullMergeRequest): Promise<void> {
  await send("POST", `${session(tab)}/github/merge`, req);
}

export async function pullAction(tab: string, req: PullAction): Promise<void> {
  await send("POST", `${session(tab)}/github/pull`, req);
}

/** The review threads, reviews and comments of the pull request the bar names. */
export async function readThreads(tab: string, req: ThreadsRequest): Promise<PullThreads> {
  return api<PullThreads>(await getEndpoint(), "POST", `${session(tab)}/github/threads`, req);
}

/** Resolves, unresolves or replies to a thread of the bar's pull request. */
export async function threadAction(tab: string, req: ThreadAction): Promise<void> {
  await send("POST", `${session(tab)}/github/thread`, req);
}

/** The text that asks the session's agent about the pull request. A 409 for a session that takes no text reads as `/review`'s does (`refusalOf`). */
export async function ask(tab: string, req: AskRequest): Promise<AskReply> {
  return api<AskReply>(await getEndpoint(), "POST", `${session(tab)}/github/ask`, req);
}
