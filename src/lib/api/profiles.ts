import type { Endpoint } from "../../generated/Endpoint";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import { api } from "./client";

export const profiles = {
  get: (ep: Endpoint) => api<ProfilesDoc>(ep, "GET", "/v1/profiles"),
  /** Rejects with a 400 ApiError (message from the core) when the document fails validation. */
  put: (ep: Endpoint, doc: ProfilesDoc) => api<ProfilesDoc>(ep, "PUT", "/v1/profiles", doc),
};
