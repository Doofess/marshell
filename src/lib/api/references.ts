import type { Endpoint } from "../../generated/Endpoint";
import type { ProjectReferences } from "../../generated/ProjectReferences";
import type { ReferencesUpdate } from "../../generated/ReferencesUpdate";
import { api } from "./client";

/**
 * A project's reference aliases. A refusal (422 an alias is not allowed, 400 the folder is not a full path or not a folder) rejects
 * with an ApiError whose message is the core's own sentence.
 */

/** The toggle and aliases of the project holding `cwd`. A linked worktree reads its main repo's. */
export const getReferences = (ep: Endpoint, cwd: string) =>
  api<ProjectReferences>(ep, "GET", `/v1/references?cwd=${encodeURIComponent(cwd)}`);

/** Replaces the project's toggle and aliases. */
export const putReferences = (ep: Endpoint, update: ReferencesUpdate) => api<void>(ep, "PUT", "/v1/references", update);
