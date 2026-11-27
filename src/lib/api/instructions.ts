import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { CopyFix } from "../../generated/CopyFix";
import type { DocRef } from "../../generated/DocRef";
import type { DocSave } from "../../generated/DocSave";
import type { DocSaved } from "../../generated/DocSaved";
import type { Endpoint } from "../../generated/Endpoint";
import type { LibraryReview } from "../../generated/LibraryReview";
import type { LibraryView } from "../../generated/LibraryView";
import { conflictBody } from "./agents";
import { api, ApiError } from "./client";

/** The Instructions page: the user docs and, with a project, that project's docs. */
export const libraryView = (ep: Endpoint, project: string | null) =>
  api<LibraryView>(ep, "GET", project === null ? "/v1/instructions" : `/v1/instructions?project=${encodeURIComponent(project)}`);

export type SaveOutcome = { ok: true; saved: DocSaved } | { ok: false; conflict: true } | { ok: false; conflict: false; message: string };

/**
 * Saves a doc. A 409 whose body names a reason or files is a change since the page read the doc (a conflict); any other 404, 409 or 422
 * carries a sentence that says why the save was refused.
 */
export async function saveDoc(ep: Endpoint, req: DocSave): Promise<SaveOutcome> {
  try {
    return { ok: true, saved: await api<DocSaved>(ep, "POST", "/v1/instructions/save", req) };
  } catch (e) {
    if (!(e instanceof ApiError) || (e.status !== 404 && e.status !== 409 && e.status !== 422)) throw e;
    if (e.status === 409) {
      const { reason, paths } = conflictBody(e.message);
      if (reason !== undefined || paths.length > 0) return { ok: false, conflict: true };
    }
    return { ok: false, conflict: false, message: e.message };
  }
}

/** The preview of Review: every out-of-date, missing and extra copy. */
export const reviewCopies = (ep: Endpoint, project: string | null) => {
  const body: LibraryReview = project === null ? {} : { project };
  return api<ConfigPreview>(ep, "POST", "/v1/instructions/review", body);
};

/** The preview of deleting a doc and its copies. */
export const previewDelete = (ep: Endpoint, ref: DocRef) => api<ConfigPreview>(ep, "POST", "/v1/instructions/delete", ref);

/** The preview of one fix for a copy changed outside Marshell. */
export const previewCopyFix = (ep: Endpoint, fix: CopyFix) => api<ConfigPreview>(ep, "POST", "/v1/instructions/copy", fix);
