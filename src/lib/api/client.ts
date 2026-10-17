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
  if (endpoint) return endpoint;
  const p: Promise<Endpoint> = invoke<Endpoint>("core_endpoint").catch((e: unknown) => {
    // A failed lookup must not stick, but a newer lookup (after resetEndpoint) must survive.
    if (endpoint === p) endpoint = null;
    throw e;
  });
  endpoint = p;
  return p;
}

export function resetEndpoint(): void {
  endpoint = null;
}

/** The core's address for a path, and the bearer header every request carries. */
export const coreUrl = (ep: Endpoint, path: string) => `http://127.0.0.1:${ep.port}${path}`;
export const bearer = (ep: Endpoint): Record<string, string> => ({ Authorization: `Bearer ${ep.token}` });

/** One authenticated request to the core, answered as it came, for a caller that reads headers or raw bytes. `api()` is built on it. */
export async function coreFetch(
  ep: Endpoint,
  method: "GET" | "POST" | "PATCH" | "PUT" | "DELETE",
  path: string,
  body?: unknown,
): Promise<Response> {
  const headers = bearer(ep);
  if (body !== undefined) headers["Content-Type"] = "application/json";
  return fetch(coreUrl(ep, path), {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
}

/** One request to the core's local API. A 204 resolves to `undefined`. */
export async function api<T>(
  ep: Endpoint,
  method: "GET" | "POST" | "PATCH" | "PUT" | "DELETE",
  path: string,
  body?: unknown,
): Promise<T> {
  const res = await coreFetch(ep, method, path, body);
  if (!res.ok) throw new ApiError(res.status, await res.text());
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}
