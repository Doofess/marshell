import { invoke } from "@tauri-apps/api/core";
import type { PermissionDecision } from "../../generated/PermissionDecision";

/** Tauri's `invoke`, injectable so tests run without a window. */
export type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

/**
 * An administrator session takes input, resize and approval answers only from the app's own window: the token API refuses
 * them for that row (decision 8). These reach the in-process core over Tauri commands instead. A refusal rejects with an
 * Error carrying the core's own sentence, which Tauri hands back as a bare string.
 */
async function call(command: string, args: Record<string, unknown>, ask: Invoke): Promise<void> {
  try {
    await ask(command, args);
  } catch (e) {
    throw new Error(typeof e === "string" ? e : e instanceof Error ? e.message : String(e));
  }
}

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
