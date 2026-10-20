import { invoke } from "@tauri-apps/api/core";
import type { Endpoint } from "../../generated/Endpoint";
import type { ExportPreview } from "../../generated/ExportPreview";
import type { LoggingState } from "../../generated/LoggingState";
import { ApiError, api, coreFetch } from "./client";

const base = "/v1/diagnostics";
const exportPath = (id: string) => `${base}/export/${encodeURIComponent(id)}`;

async function ok(res: Response): Promise<Response> {
  if (!res.ok) throw new ApiError(res.status, await res.text());
  return res;
}

/** The core's diagnostics routes. An unknown or expired export id is a 404 ApiError. */
export const diagnostics = {
  logging: (ep: Endpoint) => api<LoggingState>(ep, "GET", `${base}/logging`),
  /** A 409 ApiError when the core runs without a log. */
  setDetailed: (ep: Endpoint, detailed: boolean) => api<LoggingState>(ep, "PUT", `${base}/logging`, { detailed }),
  prepare: (ep: Endpoint) => api<ExportPreview>(ep, "POST", `${base}/export`),
  file: async (ep: Endpoint, id: string, name: string) => (await ok(await coreFetch(ep, "GET", `${exportPath(id)}/file?name=${encodeURIComponent(name)}`))).text(),
  zip: async (ep: Endpoint, id: string) => (await ok(await coreFetch(ep, "GET", `${exportPath(id)}/zip`))).arrayBuffer(),
};

/** Saves the zip through the native Save dialog. The path it was saved to, or null when the person cancelled. */
export const saveExport = (bytes: ArrayBuffer, name: string): Promise<string | null> =>
  invoke<string | null>("save_diagnostics", new Uint8Array(bytes), { headers: { "x-file-name": name } });

/** `marshell-diagnostics-YYYY-MM-DD.zip`, in local time. */
export function exportName(now: Date): string {
  const two = (n: number) => String(n).padStart(2, "0");
  return `marshell-diagnostics-${now.getFullYear()}-${two(now.getMonth() + 1)}-${two(now.getDate())}.zip`;
}

/** A file size: bytes, then KB and MB with one decimal. */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}
