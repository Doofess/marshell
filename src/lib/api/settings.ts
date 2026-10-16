import type { Endpoint } from "../../generated/Endpoint";
import type { Settings } from "../../generated/Settings";
import { api } from "./client";

export const settings = {
  get: (ep: Endpoint) => api<Settings>(ep, "GET", "/v1/settings"),
  /** Rejects with a 400 ApiError (message from the core) when the settings fail validation. */
  put: (ep: Endpoint, s: Settings) => api<Settings>(ep, "PUT", "/v1/settings", s),
};
