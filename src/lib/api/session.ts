import { invoke } from "@tauri-apps/api/core";
import type { CreateSessionRequest } from "../../generated/CreateSessionRequest";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";

export type StartResult =
  | { kind: "ready"; endpoint: Endpoint; tabId: string }
  | { kind: "error"; message: string };

let pending: Promise<StartResult> | null = null;

/** Creates the phase 1 tab once, even if React runs effects twice in development. */
export function startSession(): Promise<StartResult> {
  pending ??= create();
  return pending;
}

async function create(): Promise<StartResult> {
  let endpoint: Endpoint;
  try {
    endpoint = await invoke<Endpoint>("core_endpoint");
  } catch (e) {
    return { kind: "error", message: `Couldn't reach the core: ${String(e)}` };
  }
  const body: CreateSessionRequest = { cols: 80, rows: 24 };
  try {
    const res = await fetch(`http://127.0.0.1:${endpoint.port}/v1/sessions`, {
      method: "POST",
      headers: { Authorization: `Bearer ${endpoint.token}`, "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) return { kind: "error", message: `Couldn't start a shell. ${await res.text()}` };
    const { tab_id } = (await res.json()) as CreateSessionResponse;
    return { kind: "ready", endpoint, tabId: tab_id };
  } catch (e) {
    return { kind: "error", message: `Couldn't start a shell. ${String(e)}` };
  }
}
