import type { Conversation } from "../../generated/Conversation";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";
import type { HistoryEntry } from "../../generated/HistoryEntry";
import type { HistoryQuery } from "../../generated/HistoryQuery";
import type { SearchHit } from "../../generated/SearchHit";
import { api } from "./client";
import { refusable } from "./sessions";

const conversation = (agent: string, id: string) =>
  `/v1/history/${encodeURIComponent(agent)}/${encodeURIComponent(id)}`;

/** What starting a conversation again comes to: the new tab, or the core's reason for refusing. */
export type Resumed = { ok: true; tabId: string } | { ok: false; reason: string };

export const history = {
  /** The archived conversations, newest first. Unset filters are left out of the query string. */
  list: (ep: Endpoint, q: HistoryQuery) => {
    const params = new URLSearchParams();
    for (const key of ["agent", "project", "branch", "model", "since_ms"] as const) {
      const v = q[key];
      if (v !== undefined) params.set(key, String(v));
    }
    const qs = params.toString();
    return api<HistoryEntry[]>(ep, "GET", qs ? `/v1/history?${qs}` : "/v1/history");
  },
  search: (ep: Endpoint, q: string) =>
    api<SearchHit[]>(ep, "GET", `/v1/history/search?${new URLSearchParams({ q }).toString()}`),
  /** Rejects with a 404 ApiError ("No such conversation.") when there is none. */
  get: (ep: Endpoint, agent: string, id: string) => api<Conversation>(ep, "GET", conversation(agent, id)),
  /** Starts the conversation again in a new tab. A 409 comes back as `{ ok: false, reason }`, as for `sessions.resume`. */
  resume: async (ep: Endpoint, agent: string, id: string, size: { cols: number; rows: number }): Promise<Resumed> => {
    let created: CreateSessionResponse | undefined;
    const started = await refusable(async () => {
      created = await api<CreateSessionResponse>(ep, "POST", `${conversation(agent, id)}/resume`, size);
    });
    return started.ok && created ? { ok: true, tabId: created.tab_id } : started.ok ? { ok: false, reason: "The core did not say which tab it started." } : started;
  },
};
