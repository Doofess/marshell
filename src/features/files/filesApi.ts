import type { SearchReply } from "../../generated/SearchReply";
import type { TreeAsk } from "../../generated/TreeAsk";
import type { TreeReply } from "../../generated/TreeReply";
import { ApiError, api, getEndpoint } from "../../lib/api/client";

const session = (tab: string) => `/v1/sessions/${encodeURIComponent(tab)}`;

/** A 404 means the session has no project folder: a value, not an error. */
async function orNoFolder<T>(run: () => Promise<T>): Promise<T | "no-folder"> {
  try {
    return await run();
  } catch (e) {
    if (e instanceof ApiError && e.status === 404) return "no-folder";
    throw e;
  }
}

/** The asked folders of the session's project, one level each. */
export async function listTree(tab: string, asks: TreeAsk[]): Promise<TreeReply | "no-folder"> {
  const ep = await getEndpoint();
  return orNoFolder(() => api<TreeReply>(ep, "POST", `${session(tab)}/tree`, { dirs: asks }));
}

/** A plain-text search of the session's project. `q` is the text to find (the text after "?" in the box). `signal` cancels it. */
export async function searchFiles(tab: string, q: string, signal: AbortSignal): Promise<SearchReply | "no-folder"> {
  const ep = await getEndpoint();
  return orNoFolder(() => api<SearchReply>(ep, "POST", `${session(tab)}/search`, { q }, signal));
}
