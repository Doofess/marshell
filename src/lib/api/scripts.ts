import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";
import type { RepoScripts } from "../../generated/RepoScripts";
import type { RunScriptRequest } from "../../generated/RunScriptRequest";
import { api } from "./client";

/**
 * A repo's `marshell.json`. A refusal (409 the file changed, 403 not allowed, 422 no such script) rejects with an ApiError whose
 * message is the core's own sentence.
 */

/** The scripts of the checkout holding `cwd`, and whether the person allowed exactly this file. */
export const getScripts = (ep: Endpoint, cwd: string) => api<RepoScripts>(ep, "GET", `/v1/scripts?cwd=${encodeURIComponent(cwd)}`);

/** Allows the file whose hash the person was shown. Rejects with a 409 ApiError when the file has changed since. */
export const allowScripts = (ep: Endpoint, cwd: string, hash: string) =>
  api<RepoScripts>(ep, "POST", "/v1/scripts/allow", { cwd, hash });

/** Starts `setup` or `run` as a session in the checkout's root. */
export const runScript = (ep: Endpoint, req: RunScriptRequest) => api<CreateSessionResponse>(ep, "POST", "/v1/scripts/run", req);
