import type { Endpoint } from "../../generated/Endpoint";
import type { FolderInfo } from "../../generated/FolderInfo";
import { api } from "./client";

export const folders = {
  /** Asks the core whether `path` is a full path to an existing folder. Throws `ApiError` 400 (not a full path) or 404 (no folder there). */
  check: (ep: Endpoint, path: string) => api<FolderInfo>(ep, "GET", `/v1/folders/check?path=${encodeURIComponent(path)}`),
};
