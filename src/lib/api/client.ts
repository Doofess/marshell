import { invoke } from "@tauri-apps/api/core";
import type { Endpoint } from "../../generated/Endpoint";

/** A non-2xx answer from the core. `message` is the response text. */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

let endpoint: Promise<Endpoint> | null = null;

/** The core's port and token, fetched once. Call `resetEndpoint()` after the core restarts. */
export function getEndpoint(): Promise<Endpoint> {
  endpoint ??= invoke<Endpoint>("core_endpoint").catch((e: unknown) => {
    endpoint = null; // a failed lookup must not stick
    throw e;
  });
  return endpoint;
}

export function resetEndpoint(): void {
  endpoint = null;
}

/** One request to the core's local API. A 204 resolves to `undefined`. */
export async function api<T>(
  ep: Endpoint,
  method: "GET" | "POST" | "PATCH" | "PUT" | "DELETE",
  path: string,
  body?: unknown,
): Promise<T> {
  const headers: Record<string, string> = { Authorization: `Bearer ${ep.token}` };
  if (body !== undefined) headers["Content-Type"] = "application/json";
  const res = await fetch(`http://127.0.0.1:${ep.port}${path}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!res.ok) throw new ApiError(res.status, await res.text());
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}
