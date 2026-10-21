import type { Endpoint } from "../../generated/Endpoint";
import { api } from "./client";

/** The core's "pause notifications" flag. It stops banners, sounds and the taskbar flash, never the needs-you count. */
export const notify = {
  paused: async (ep: Endpoint) => (await api<{ paused: boolean }>(ep, "GET", "/v1/notify/paused")).paused,
  setPaused: (ep: Endpoint, paused: boolean) => api<void>(ep, "PUT", "/v1/notify/paused", { paused }),
};
