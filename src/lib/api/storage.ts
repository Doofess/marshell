import type { RetentionLimits } from "../../generated/RetentionLimits";
import type { RetentionPreview } from "../../generated/RetentionPreview";
import type { StorageReport } from "../../generated/StorageReport";
import { api, getEndpoint } from "./client";

/** What Marshell keeps on disk, the saved limits, the last cleanup and the backup. Rejects with a 503 ApiError when history is off for this run. */
export const getStorage = async (): Promise<StorageReport> => api<StorageReport>(await getEndpoint(), "GET", "/v1/history/storage");

/** What a cleanup with these limits would remove now. Rejects with a 400 ApiError for a limit that isn't one of the choices. */
export const previewRetention = async (limits: RetentionLimits): Promise<RetentionPreview> =>
  api<RetentionPreview>(await getEndpoint(), "POST", "/v1/history/storage/preview", limits);
