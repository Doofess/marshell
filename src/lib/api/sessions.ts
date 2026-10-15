import type { CreateSessionRequest } from "../../generated/CreateSessionRequest";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";
import type { RenameRequest } from "../../generated/RenameRequest";
import type { ReopenRequest } from "../../generated/ReopenRequest";
import type { ReorderRequest } from "../../generated/ReorderRequest";
import type { SessionsSnapshot } from "../../generated/SessionsSnapshot";
import { api } from "./client";

const tab = (tabId: string) => `/v1/sessions/${encodeURIComponent(tabId)}`;

export const sessions = {
  list: (ep: Endpoint) => api<SessionsSnapshot>(ep, "GET", "/v1/sessions"),
  create: (ep: Endpoint, req: CreateSessionRequest) =>
    api<CreateSessionResponse>(ep, "POST", "/v1/sessions", req),
  rename: (ep: Endpoint, tabId: string, name: string) =>
    api<void>(ep, "PATCH", tab(tabId), { name } satisfies RenameRequest),
  reorder: (ep: Endpoint, tabIds: string[]) =>
    api<void>(ep, "POST", "/v1/sessions/reorder", { tab_ids: tabIds } satisfies ReorderRequest),
  close: (ep: Endpoint, tabId: string) => api<void>(ep, "DELETE", tab(tabId)),
  /** Without a tab id, reopens the most recently closed session. Rejects with a 404 ApiError when none is left. */
  reopen: (ep: Endpoint, tabId?: string) =>
    api<CreateSessionResponse>(
      ep,
      "POST",
      "/v1/sessions/reopen",
      tabId === undefined ? undefined : ({ tab_id: tabId } satisfies ReopenRequest),
    ),
};
