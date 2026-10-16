import type { Endpoint } from "../../generated/Endpoint";
import type { IntegrationInfo } from "../../generated/IntegrationInfo";
import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import { api } from "./client";

export const profiles = {
  get: (ep: Endpoint) => api<ProfilesDoc>(ep, "GET", "/v1/profiles"),
  /** The shells detection found, as found. A profile the user edited is back to these on Reset. */
  detected: (ep: Endpoint) => api<Profile[]>(ep, "GET", "/v1/profiles/detected"),
  /** Rejects with a 400 ApiError (message from the core) when the document fails validation. */
  put: (ep: Endpoint, doc: ProfilesDoc) => api<ProfilesDoc>(ep, "PUT", "/v1/profiles", doc),
  /** What shell integration would do for this profile (saved or not): the script it loads, or why it doesn't apply. Writes nothing. */
  integration: (ep: Endpoint, profile: Profile) => api<IntegrationInfo>(ep, "POST", "/v1/profiles/integration", profile),
};
