import { invoke } from "@tauri-apps/api/core";
import type { CreateSessionRequest } from "../../generated/CreateSessionRequest";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { PermissionDecision } from "../../generated/PermissionDecision";
import type { RestartRequest } from "../../generated/RestartRequest";
import type { SessionMode } from "../../generated/SessionMode";
import { ApiError } from "./client";

/** Tauri's `invoke`, injectable so tests run without a window. */
export type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

/**
 * An administrator session takes input, resize and approval answers only from the app's own window: the token API refuses
 * them for that row (decision 8). These reach the in-process core over Tauri commands instead. A refusal rejects with an
 * Error carrying the core's own sentence, which Tauri hands back as a bare string.
 */
async function callFor<T>(command: string, args: Record<string, unknown>, ask: Invoke): Promise<T> {
  try {
    return (await ask(command, args)) as T;
  } catch (e) {
    throw new Error(sentenceOf(e));
  }
}

const sentenceOf = (e: unknown): string => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

const call = async (command: string, args: Record<string, unknown>, ask: Invoke): Promise<void> => void (await callFor<unknown>(command, args, ask));

export const sendInput = (tabId: string, text: string, ask: Invoke = invoke): Promise<void> => call("elevated_input", { tabId, text }, ask);

export const resize = (tabId: string, cols: number, rows: number, ask: Invoke = invoke): Promise<void> =>
  call("elevated_resize", { tabId, cols, rows }, ask);

export const decide = (rid: string, decision: PermissionDecision, ask: Invoke = invoke): Promise<void> =>
  call("elevated_decide", { rid, decision }, ask);

/** Where a terminal's keystrokes and size go instead of the pty socket. */
export interface TrustedSink {
  input(text: string): Promise<void>;
  resize(cols: number, rows: number): Promise<void>;
}

/** The sink for one administrator session. */
export const trustedSink = (tabId: string, ask: Invoke = invoke): TrustedSink => ({
  input: (text) => sendInput(tabId, text, ask),
  resize: (cols, rows) => resize(tabId, cols, rows, ask),
});

/**
 * Types `text` into a session with no Enter, as `POST /v1/sessions/{tab}/input` does for every other row: an
 * administrator row takes it through the window's own path, with the same refusal of a line break.
 */
export async function typeText(
  tabId: string,
  text: string,
  elevated: boolean,
  viaApi: () => Promise<void>,
  ask: Invoke = invoke,
): Promise<void> {
  if (!elevated) return viaApi();
  if (/[\r\n]/.test(text)) throw new Error("The text can't contain a line break.");
  return sendInput(tabId, text, ask);
}

/** What starting a row again comes to: it worked, or the core's own sentence for why not (as `sessions.resume` answers). */
type Started = { ok: true } | { ok: false; reason: string };

const startedOf = async (run: () => Promise<unknown>): Promise<Started> => {
  try {
    await run();
    return { ok: true };
  } catch (e) {
    return { ok: false, reason: sentenceOf(e) };
  }
};

type Size = { cols: number; rows: number };

/**
 * Starting an administrator session is the window's alone (decision 10): the token API refuses a create with `elevated`, and the
 * restart, resume and reopen of an administrator row. These route each start to the Tauri command that reaches the in-process
 * core, and every other start to the API as before. A refused create rejects with an ApiError, so the launcher shows the sentence.
 */
export async function createSession(
  req: CreateSessionRequest,
  viaApi: () => Promise<CreateSessionResponse>,
  ask: Invoke = invoke,
): Promise<CreateSessionResponse> {
  if (!req.elevated) return viaApi();
  try {
    return await callFor<CreateSessionResponse>("elevated_create", { req }, ask);
  } catch (e) {
    throw new ApiError(409, sentenceOf(e));
  }
}

export async function resumeSession(elevated: boolean, tabId: string, size: Size, viaApi: () => Promise<Started>, ask: Invoke = invoke): Promise<Started> {
  if (!elevated) return viaApi();
  return startedOf(() => callFor("elevated_resume", { tabId, ...size }, ask));
}

export async function restartSession(
  elevated: boolean,
  tabId: string,
  mode: SessionMode,
  size: Size,
  viaApi: () => Promise<Started>,
  ask: Invoke = invoke,
): Promise<Started> {
  if (!elevated) return viaApi();
  return startedOf(() => callFor("elevated_restart", { tabId, req: { mode, ...size } satisfies RestartRequest }, ask));
}

/**
 * Reopens a closed session. A closed row is no longer listed, so nothing here knows whether it ran as administrator: the API
 * goes first, and its 409 (the row ran as administrator) sends the same reopen through the window. With no tab id it is the
 * most recently closed one, on either path.
 */
export async function reopenSession(
  tabId: string | undefined,
  viaApi: () => Promise<CreateSessionResponse>,
  ask: Invoke = invoke,
): Promise<CreateSessionResponse> {
  try {
    return await viaApi();
  } catch (e) {
    if (!(e instanceof ApiError && e.status === 409)) throw e;
    try {
      return await callFor<CreateSessionResponse>("elevated_reopen", { tabId: tabId ?? null }, ask);
    } catch (inner) {
      throw new ApiError(409, sentenceOf(inner));
    }
  }
}
