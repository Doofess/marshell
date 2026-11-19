import type { Draft } from "../../generated/Draft";
import type { DraftInfo } from "../../generated/DraftInfo";
import type { DraftList } from "../../generated/DraftList";
import type { FileConflict } from "../../generated/FileConflict";
import type { FileMerge } from "../../generated/FileMerge";
import type { FileMerged } from "../../generated/FileMerged";
import type { FileSaved } from "../../generated/FileSaved";
import type { FileUnsave } from "../../generated/FileUnsave";
import { api, bearer, coreFetch, coreUrl, getEndpoint } from "../../lib/api/client";

export type SaveResult =
  | { kind: "saved"; saved: FileSaved }
  | { kind: "conflict"; hash: string | null }
  | { kind: "tooLarge" }
  | { kind: "refused"; line: string };

export type UnsaveResult = { kind: "saved"; saved: FileSaved } | { kind: "conflict" } | { kind: "refused"; line: string };

/** What a refusal says when the core gave no line, or could not be reached. */
const GENERIC_LINE = "The request failed.";

/** The core's plain-text line for a refusal, or a generic one when the body is empty. */
async function lineOf(res: Response): Promise<string> {
  try {
    const text = (await res.text()).trim();
    return text === "" ? GENERIC_LINE : text;
  } catch {
    return GENERIC_LINE;
  }
}

/** Save the bytes to the file. `expect` is the hash the edits started from, or null for no file. */
export async function saveFile(path: string, bytes: Uint8Array, expect: string | null): Promise<SaveResult> {
  try {
    const ep = await getEndpoint();
    const query = `path=${encodeURIComponent(path)}&expect=${expect ?? "none"}`;
    // The body is raw bytes, so this does not go through `coreFetch`, which sends JSON.
    const res = await fetch(coreUrl(ep, `/v1/files/content?${query}`), {
      method: "PUT",
      headers: { ...bearer(ep), "Content-Type": "application/octet-stream" },
      body: bytes as BodyInit,
    });
    if (res.status === 200) return { kind: "saved", saved: (await res.json()) as FileSaved };
    if (res.status === 409) return { kind: "conflict", hash: ((await res.json()) as FileConflict).hash ?? null };
    if (res.status === 413) return { kind: "tooLarge" };
    return { kind: "refused", line: await lineOf(res) };
  } catch {
    return { kind: "refused", line: GENERIC_LINE };
  }
}

/** Put back what the save named by `snapshot` replaced. A 404 (no such save) is a conflict. */
export async function unsaveFile(path: string, snapshot: string): Promise<UnsaveResult> {
  try {
    const ep = await getEndpoint();
    const body: FileUnsave = { path, snapshot };
    const res = await coreFetch(ep, "POST", "/v1/files/unsave", body);
    if (res.status === 200) return { kind: "saved", saved: (await res.json()) as FileSaved };
    if (res.status === 409 || res.status === 404) return { kind: "conflict" };
    return { kind: "refused", line: await lineOf(res) };
  } catch {
    return { kind: "refused", line: GENERIC_LINE };
  }
}

export type MergeResult = { kind: "merged"; text: string; conflicts: number } | { kind: "refused"; line: string };

/** Merge the person's text and the disk's text, both of which grew from `base`. Each has `\n` line endings. */
export async function mergeTexts(base: string, mine: string, disk: string): Promise<MergeResult> {
  try {
    const ep = await getEndpoint();
    const body: FileMerge = { base, mine, disk };
    const res = await coreFetch(ep, "POST", "/v1/files/merge", body);
    if (res.status === 200) {
      const merged = (await res.json()) as FileMerged;
      return { kind: "merged", text: merged.text, conflicts: merged.conflicts };
    }
    if (res.status === 413) return { kind: "refused", line: "A text is over 3 MB." };
    return { kind: "refused", line: await lineOf(res) };
  } catch {
    return { kind: "refused", line: GENERIC_LINE };
  }
}

/** The files that have a draft, newest first. A failed request is an empty list. */
export async function listDrafts(): Promise<DraftInfo[]> {
  try {
    const ep = await getEndpoint();
    return (await api<DraftList>(ep, "GET", "/v1/drafts")).drafts;
  } catch {
    return [];
  }
}

/** The file's draft, or null when there is none or it can't be read. */
export async function getDraft(path: string): Promise<Draft | null> {
  try {
    const ep = await getEndpoint();
    const res = await coreFetch(ep, "GET", `/v1/drafts/one?path=${encodeURIComponent(path)}`);
    // 204 is the core's "no draft".
    if (res.status !== 200) return null;
    return (await res.json()) as Draft;
  } catch {
    return null;
  }
}

/** Keep the draft. False when the core refused it or could not be reached. */
export async function putDraft(draft: Draft): Promise<boolean> {
  try {
    const ep = await getEndpoint();
    const res = await coreFetch(ep, "PUT", "/v1/drafts/one", draft);
    return res.status === 204;
  } catch {
    return false;
  }
}

/** Drop the file's draft. A failure leaves the draft, which the core expires. */
export async function dropDraft(path: string): Promise<void> {
  try {
    const ep = await getEndpoint();
    await coreFetch(ep, "DELETE", `/v1/drafts/one?path=${encodeURIComponent(path)}`);
  } catch {
    // The draft stays until it expires or is dropped again.
  }
}
