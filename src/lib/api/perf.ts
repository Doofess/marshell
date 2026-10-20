import type { Endpoint } from "../../generated/Endpoint";
import type { PerfSnapshot } from "../../generated/PerfSnapshot";
import { api } from "./client";

/** The core's performance route: startup time and resource use. */
export const perf = {
  snapshot: (ep: Endpoint) => api<PerfSnapshot>(ep, "GET", "/v1/perf"),
};
