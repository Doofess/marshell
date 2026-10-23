import { invoke } from "@tauri-apps/api/core";
import type { Conversation } from "../../generated/Conversation";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";
import type { HistoryEntry } from "../../generated/HistoryEntry";
import type { HistoryPatch } from "../../generated/HistoryPatch";
import type { HistoryQuery } from "../../generated/HistoryQuery";
import type { SearchHit } from "../../generated/SearchHit";
import type { ExportFormat } from "../exportName";
import { ApiError, api, coreFetch } from "./client";
import { reasonOf, refusable } from "./sessions";

const conversation = (agent: string, id: string) =>
  `/v1/history/${encodeURIComponent(agent)}/${encodeURIComponent(id)}`;

/** What starting a conversation again comes to: the new tab, or the core's reason for refusing. */
export type Resumed = { ok: true; tabId: string } | { ok: false; reason: string };

/** The filters as a query string; unset fields and false booleans are left out. */
function filterParams(q: HistoryQuery): URLSearchParams {
  const params = new URLSearchParams();
  for (const key of ["agent", "project", "branch", "model", "since_ms", "tag"] as const) {
    const v = q[key];
    if (v !== undefined) params.set(key, String(v));
  }
  if (q.bookmarked) params.set("bookmarked", "true");
  if (q.archived) params.set("archived", "true");
  return params;
}

const withQuery = (path: string, params: URLSearchParams) => (params.size > 0 ? `${path}?${params.toString()}` : path);

/** Runs a change the core may refuse with a 400 or 409 in its own words; anything else rejects. */
async function refused<T extends object>(request: () => Promise<T>): Promise<({ ok: true } & T) | { ok: false; reason: string }> {
  try {
    return { ok: true, ...(await request()) };
  } catch (e) {
    if (e instanceof ApiError && (e.status === 400 || e.status === 409)) return { ok: false, reason: reasonOf(e) };
    throw e;
  }
}

type Size = { cols: number; rows: number };

async function startAgain(ep: Endpoint, path: string, size: Size): Promise<Resumed> {
  let created: CreateSessionResponse | undefined;
  const started = await refusable(async () => {
    created = await api<CreateSessionResponse>(ep, "POST", path, size);
  });
  return started.ok && created ? { ok: true, tabId: created.tab_id } : started.ok ? { ok: false, reason: "The core did not say which tab it started." } : started;
}

export const history = {
  /** The archived conversations, newest first. Unset filters are left out of the query string. */
  list: (ep: Endpoint, q: HistoryQuery) => api<HistoryEntry[]>(ep, "GET", withQuery("/v1/history", filterParams(q))),
  /** `archived` includes archived conversations in the results. */
  search: (ep: Endpoint, q: string, archived = false) => {
    const params = new URLSearchParams({ q });
    if (archived) params.set("archived", "true");
    return api<SearchHit[]>(ep, "GET", `/v1/history/search?${params.toString()}`);
  },
  /** One row per bookmark, as a search hit, with the same filters as `list`. */
  bookmarks: (ep: Endpoint, q: HistoryQuery) => api<SearchHit[]>(ep, "GET", withQuery("/v1/history/bookmarks", filterParams(q))),
  /** Rejects with a 404 ApiError ("No such conversation.") when there is none. */
  get: (ep: Endpoint, agent: string, id: string) => api<Conversation>(ep, "GET", conversation(agent, id)),
  /** Changes the supplied marks. A 400 (a tag the core refuses) comes back as `{ ok: false, reason }` in the core's words. */
  patch: (ep: Endpoint, agent: string, id: string, p: HistoryPatch) =>
    refused(async () => ({ entry: await api<HistoryEntry>(ep, "PATCH", conversation(agent, id), p) })),
  /** Marks or unmarks a turn (0-based, as in a search hit). A 409 (a turn the core hasn't indexed yet) comes back as `{ ok: false, reason }` in the core's words. */
  bookmarkTurn: (ep: Endpoint, agent: string, id: string, turn: number, on: boolean) =>
    refused(async () => {
      await api<void>(ep, on ? "PUT" : "DELETE", `${conversation(agent, id)}/bookmarks/${turn}`);
      return {};
    }),
  /** Deletes Marshell's copy only. A 409 (running in a tab, copy in progress) comes back as `{ ok: false, reason }`. */
  remove: (ep: Endpoint, agent: string, id: string) =>
    refused(async () => {
      await api<void>(ep, "DELETE", conversation(agent, id));
      return {};
    }),
  /** Starts the conversation again in a new tab. A 409 comes back as `{ ok: false, reason }`, as for `sessions.resume`. */
  resume: (ep: Endpoint, agent: string, id: string, size: Size): Promise<Resumed> =>
    startAgain(ep, `${conversation(agent, id)}/resume`, size),
  /** Like `resume`, but Claude starts a new conversation from this one. */
  fork: (ep: Endpoint, agent: string, id: string, size: Size): Promise<Resumed> =>
    startAgain(ep, `${conversation(agent, id)}/fork`, size),
  /** The conversation as text (or, for "asciicast", its recording), with secrets and personal paths hidden when `redact`. */
  exportText: async (ep: Endpoint, agent: string, id: string, format: ExportFormat, redact: boolean): Promise<string> => {
    const params = new URLSearchParams({ format, redact: String(redact) });
    const res = await coreFetch(ep, "GET", `${conversation(agent, id)}/export?${params.toString()}`);
    if (!res.ok) throw new ApiError(res.status, await res.text());
    return res.text();
  },
};

/** Saves an export through the native Save dialog. The path it was saved to, or null when the person cancelled. `name` is `exportFileName`'s. */
export const saveConversation = (text: string, name: string): Promise<string | null> =>
  invoke<string | null>("save_conversation", new TextEncoder().encode(text), { headers: { "x-file-name": name } });
