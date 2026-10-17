import type { AttachmentSaved } from "../../generated/AttachmentSaved";
import type { Endpoint } from "../../generated/Endpoint";
import type { PasteRequest } from "../../generated/PasteRequest";
import { ApiError, api, bearer, coreUrl } from "../../lib/api/client";

const tab = (tabId: string) => `/v1/sessions/${encodeURIComponent(tabId)}`;

/** Saves a region's PNG as the session's attachment. Rejects with an ApiError (400, 404, 413) carrying the core's words. */
export async function saveAttachment(ep: Endpoint, tabId: string, png: Blob): Promise<AttachmentSaved> {
  const res = await fetch(coreUrl(ep, `${tab(tabId)}/attachments`), {
    method: "POST",
    headers: { ...bearer(ep), "Content-Type": "image/png" },
    body: png,
  });
  if (!res.ok) throw new ApiError(res.status, await res.text());
  return (await res.json()) as AttachmentSaved;
}

/** Pastes a saved attachment's reference and a comment into the session, never with Enter. A paused session answers 409. */
export const pasteAttachment = (ep: Endpoint, tabId: string, req: PasteRequest): Promise<void> =>
  api<void>(ep, "POST", `${tab(tabId)}/paste`, req);
