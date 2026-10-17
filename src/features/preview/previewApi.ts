import type { FileList } from "../../generated/FileList";
import type { FileStat } from "../../generated/FileStat";
import type { ResolveRequest } from "../../generated/ResolveRequest";
import type { ResolveResponse } from "../../generated/ResolveResponse";
import { ApiError, api, coreFetch, getEndpoint } from "../../lib/api/client";
import type { StatResult } from "./reload";

/** The largest file the core serves per kind. Mirrors the core's caps; a bigger file is a 413 there and "too large" here. */
export const TEXT_CAP = 2 * 1024 * 1024;
export const IMAGE_CAP = 20 * 1024 * 1024;

const session = (tab: string) => `/v1/sessions/${encodeURIComponent(tab)}`;

/** The size and modification time of a file, or `missing` on a 404. A refused path (400) throws an ApiError. */
export async function statFile(path: string): Promise<StatResult> {
  const ep = await getEndpoint();
  const res = await coreFetch(ep, "GET", `/v1/files/stat?path=${encodeURIComponent(path)}`);
  if (res.status === 404) return { kind: "missing" };
  if (!res.ok) throw new ApiError(res.status, await res.text());
  const s = (await res.json()) as FileStat;
  return { kind: "ok", size: s.size, modified_ms: s.modified_ms };
}

export type Content =
  | { kind: "ok"; bytes: ArrayBuffer; modifiedMs: number }
  | { kind: "tooLarge"; size: number; limit: number }
  | { kind: "missing" };

/** The file's bytes. `kind` is required by the core: it picks the cap (text or image). */
export async function fetchContent(path: string, kind: "text" | "image"): Promise<Content> {
  const ep = await getEndpoint();
  const res = await coreFetch(ep, "GET", `/v1/files/content?path=${encodeURIComponent(path)}&kind=${kind}`);
  if (res.status === 404) return { kind: "missing" };
  if (res.status === 413) {
    const body = (await res.json()) as { size: number; limit: number };
    return { kind: "tooLarge", size: body.size, limit: body.limit };
  }
  if (!res.ok) throw new ApiError(res.status, await res.text());
  return { kind: "ok", bytes: await res.arrayBuffer(), modifiedMs: Number(res.headers.get("X-Modified-Ms") ?? 0) };
}

/** Which of these texts name an existing file in the session's project: the absolute path for each, or null. */
export async function resolvePaths(tab: string, candidates: string[]): Promise<Array<string | null>> {
  const ep = await getEndpoint();
  const body: ResolveRequest = { candidates };
  return (await api<ResolveResponse>(ep, "POST", `${session(tab)}/resolve`, body)).paths;
}

/** The files of the session's project, for quick open. */
export async function listFiles(tab: string): Promise<FileList> {
  const ep = await getEndpoint();
  return api<FileList>(ep, "GET", `${session(tab)}/files`);
}
