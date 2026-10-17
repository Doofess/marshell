import type { CreateSessionRequest } from "../../generated/CreateSessionRequest";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";
import type { InputRequest } from "../../generated/InputRequest";
import type { RenameRequest } from "../../generated/RenameRequest";
import type { ReopenRequest } from "../../generated/ReopenRequest";
import type { ReorderRequest } from "../../generated/ReorderRequest";
import type { RestartRequest } from "../../generated/RestartRequest";
import type { ResumeError } from "../../generated/ResumeError";
import type { SessionMode } from "../../generated/SessionMode";
import type { SessionsSnapshot } from "../../generated/SessionsSnapshot";
import { ApiError, api } from "./client";

const tab = (tabId: string) => `/v1/sessions/${encodeURIComponent(tabId)}`;

export type Started = { ok: true } | { ok: false; reason: string };

/** Runs a start request: a 409 becomes `{ ok: false, reason }` in the core's words, anything else rejects. */
async function refusable(request: () => Promise<unknown>): Promise<Started> {
  try {
    await request();
    return { ok: true };
  } catch (e) {
    if (e instanceof ApiError && e.status === 409) {
      try {
        const body = JSON.parse(e.message) as Partial<ResumeError>;
        if (typeof body.reason === "string") return { ok: false, reason: body.reason };
      } catch {
        // Not our JSON body: fall through to the plain text.
      }
      return { ok: false, reason: e.message };
    }
    throw e;
  }
}

export const sessions = {
  list: (ep: Endpoint) => api<SessionsSnapshot>(ep, "GET", "/v1/sessions"),
  create: (ep: Endpoint, req: CreateSessionRequest) =>
    api<CreateSessionResponse>(ep, "POST", "/v1/sessions", req),
  rename: (ep: Endpoint, tabId: string, name: string) =>
    api<void>(ep, "PATCH", tab(tabId), { name } satisfies RenameRequest),
  reorder: (ep: Endpoint, tabIds: string[]) =>
    api<void>(ep, "POST", "/v1/sessions/reorder", { tab_ids: tabIds } satisfies ReorderRequest),
  close: (ep: Endpoint, tabId: string) => api<void>(ep, "DELETE", tab(tabId)),
  /**
   * Types `text` into the session as if the user had typed it, with no Enter: the user reads it and sends it. The core
   * refuses text with a line break (400) and an unknown tab (404); both reject with an ApiError.
   */
  input: (ep: Endpoint, tabId: string, text: string) => api<void>(ep, "POST", `${tab(tabId)}/input`, { text } satisfies InputRequest),
  /** Without a tab id, reopens the most recently closed session. Rejects with a 404 ApiError when none is left. */
  reopen: (ep: Endpoint, tabId?: string) =>
    api<CreateSessionResponse>(
      ep,
      "POST",
      "/v1/sessions/reopen",
      tabId === undefined ? undefined : ({ tab_id: tabId } satisfies ReopenRequest),
    ),
  /**
   * The sidebar shell panel's own pty: started in `cwd` on the first call, the same one after that
   * while it runs. It is not a tab. Returns the id to attach a terminal to.
   */
  shellPanel: async (ep: Endpoint, cwd: string, size: { cols: number; rows: number }): Promise<string> =>
    (await api<{ tab_id: string }>(ep, "POST", "/v1/shell-panel", { cwd, ...size })).tab_id,
  /** Turns the panel shell into a tab ("Open in a pane"). Rejects with a 404 ApiError when there is none. */
  adoptShellPanel: async (ep: Endpoint): Promise<string> =>
    (await api<{ tab_id: string }>(ep, "POST", "/v1/shell-panel/adopt")).tab_id,
  /** The session's asciicast file. Rejects with a 404 ApiError ("This session has no recording.") when there is none. */
  recordingPath: async (ep: Endpoint, tabId: string): Promise<string> =>
    (await api<{ path: string }>(ep, "GET", `${tab(tabId)}/recording`)).path,
  /**
   * Starts an Ended or Exited tab again in place. A 409 comes back as `{ ok: false, reason }`
   * with the core's own words; any other failure rejects with an ApiError.
   */
  resume: (ep: Endpoint, tabId: string, size: { cols: number; rows: number }): Promise<Started> =>
    refusable(() => api<CreateSessionResponse>(ep, "POST", `${tab(tabId)}/resume`, size)),
  /** Freezes a running session's whole process tree. A 409 comes back as `{ ok: false, reason }`, as for `resume`. */
  pause: (ep: Endpoint, tabId: string): Promise<Started> => refusable(() => api<void>(ep, "POST", `${tab(tabId)}/pause`)),
  /** Thaws a paused session. Answers like `pause`. */
  cont: (ep: Endpoint, tabId: string): Promise<Started> => refusable(() => api<void>(ep, "POST", `${tab(tabId)}/continue`)),
  /** Ends an agent tab's process and starts it again in `mode`, in the same row. Answers like `resume`. */
  restart: (ep: Endpoint, tabId: string, mode: SessionMode, size: { cols: number; rows: number }): Promise<Started> =>
    refusable(() => api<CreateSessionResponse>(ep, "POST", `${tab(tabId)}/restart`, { mode, ...size } satisfies RestartRequest)),
};
