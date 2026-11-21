import type { Endpoint } from "../../generated/Endpoint";
import type { ScreenshotSaved } from "../../generated/ScreenshotSaved";
import { ApiError, bearer, coreUrl } from "./client";

/**
 * Saves a page's PNG to the Downloads folder. `host` is the page's host and port, `at` is `YYYY-MM-DD HHMMSS`;
 * the core makes the file name from them and never takes one. Rejects with an ApiError (400, 413, 500) carrying the core's words.
 */
export async function saveScreenshot(ep: Endpoint, host: string, at: string, png: Blob): Promise<ScreenshotSaved> {
  const query = `host=${encodeURIComponent(host)}&at=${encodeURIComponent(at)}`;
  const res = await fetch(coreUrl(ep, `/v1/screenshots?${query}`), {
    method: "POST",
    headers: { ...bearer(ep), "Content-Type": "image/png" },
    body: png,
  });
  if (!res.ok) throw new ApiError(res.status, await res.text());
  return (await res.json()) as ScreenshotSaved;
}
