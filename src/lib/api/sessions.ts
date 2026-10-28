import type { Bookmarked } from "../../generated/Bookmarked";
import type { CreateSessionRequest } from "../../generated/CreateSessionRequest";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";
import type { InputRequest } from "../../generated/InputRequest";
import type { RenameRequest } from "../../generated/RenameRequest";
import type { ReopenRequest } from "../../generated/ReopenRequest";
import type { ReorderRequest } from "../../generated/ReorderRequest";
import type { RestartRequest } from "../../generated/RestartRequest";
import type { ResumeCommand } from "../../generated/ResumeCommand";
import type { ResumeError } from "../../generated/ResumeError";
import type { SessionMode } from "../../generated/SessionMode";
import type { SessionsSnapshot } from "../../generated/SessionsSnapshot";
import { ApiError, api } from "./client";

const tab = (tabId: string) => `/v1/sessions/${encodeURIComponent(tabId)}`;

export type Started = { ok: true } | { ok: false; reason: string };

/** The core's reason in a refusal: the `reason` of its JSON body, or the plain text. */
export function reasonOf(e: ApiError): string {
  try {
    const body = JSON.parse(e.message) as Partial<ResumeError>;
    if (typeof body.reason === "string") return body.reason;
  } catch {
    // Not our JSON body: fall through to the plain text.
  }
  return e.message;
}

/** Runs a start request: a 409 becomes `{ ok: false, reason }` in the core's words, anything else rejects. */
export async function refusable(request: () => Promise<unknown>): Promise<Started> {
  try {
    await request();
    return { ok: true };
  } catch (e) {
    if (e instanceof ApiError && e.status === 409) return { ok: false, reason: reasonOf(e) };
    throw e;
  }
}

/** What asking for a resume command comes to: the command and its form, or the core's reason for refusing. */
export type ResumeCommandResult = ({ ok: true } & ResumeCommand) | { ok: false; reason: string };

/** Runs a resume-command request: a 409 becomes `{ ok: false, reason }` in the core's words, anything else rejects. */
export async function resumeCommandOf(request: () => Promise<ResumeCommand>): Promise<ResumeCommandResult> {
  try {
    return { ok: true, ...(await request()) };
  } catch (e) {
    if (e instanceof ApiError && e.status === 409) return { ok: false, reason: reasonOf(e) };
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
  /** Bookmarks the current point of a live Claude Code session. A 409 (nothing yet, another agent) comes back as `{ ok: false, reason }`. */
  bookmark: async (ep: Endpoint, tabId: string): Promise<{ ok: true; title: string } | { ok: false; reason: string }> => {
    try {
      return { ok: true, title: (await api<Bookmarked>(ep, "POST", `${tab(tabId)}/bookmark`)).title };
    } catch (e) {
      if (e instanceof ApiError && e.status === 409) return { ok: false, reason: reasonOf(e) };
      throw e;
    }
  },
  /**
   * Starts an Ended or Exited tab again in place. A 409 comes back as `{ ok: false, reason }`
   * with the core's own words; any other failure rejects with an ApiError.
   */
  resume: (ep: Endpoint, tabId: string, size: { cols: number; rows: number }): Promise<Started> =>
    refusable(() => api<CreateSessionResponse>(ep, "POST", `${tab(tabId)}/resume`, size)),
  /** The line that starts the session's CLI again, to paste into a terminal. A 409 comes back as `{ ok: false, reason }`; an unknown tab rejects with a 404 ApiError. */
  resumeCommand: (ep: Endpoint, tabId: string): Promise<ResumeCommandResult> =>
    resumeCommandOf(() => api<ResumeCommand>(ep, "GET", `${tab(tabId)}/resume-command`)),
  /** Starts the resume command in a system terminal window. A 409 ("End the session first.") comes back as `{ ok: false, reason }`, as for `resume`. */
  openOutside: (ep: Endpoint, tabId: string): Promise<Started> => refusable(() => api<void>(ep, "POST", `${tab(tabId)}/open-outside`)),
  /** Freezes a running session's whole process tree. A 409 comes back as `{ ok: false, reason }`, as for `resume`. */
  pause: (ep: Endpoint, tabId: string): Promise<Started> => refusable(() => api<void>(ep, "POST", `${tab(tabId)}/pause`)),
  /** Thaws a paused session. Answers like `pause`. */
  cont: (ep: Endpoint, tabId: string): Promise<Started> => refusable(() => api<void>(ep, "POST", `${tab(tabId)}/continue`)),
  /** Mutes or unmutes a session's banners and sounds. Rejects with a 404 ApiError for a tab the core doesn't know. */
  setMuted: (ep: Endpoint, tabId: string, muted: boolean): Promise<void> => api<void>(ep, "PUT", `${tab(tabId)}/muted`, { muted }),
  /** Ends an agent tab's process and starts it again in `mode`, in the same row. Answers like `resume`. */
  restart: (ep: Endpoint, tabId: string, mode: SessionMode, size: { cols: number; rows: number }): Promise<Started> =>
    refusable(() => api<CreateSessionResponse>(ep, "POST", `${tab(tabId)}/restart`, { mode, ...size } satisfies RestartRequest)),
};
