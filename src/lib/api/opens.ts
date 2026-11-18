import type { Endpoint } from "../../generated/Endpoint";
import type { PaneOpenDone } from "../../generated/PaneOpenDone";
import { coreFetch } from "./client";

/** The `marshell open` requests: the core queues them and sends the list on the event stream; the app answers each one here. */
export const opensApi = {
  /** Reports each request taken: added, marked, or refused. The core ignores an id it doesn't have, so an answer may be sent again. */
  async done(ep: Endpoint, done: PaneOpenDone[]): Promise<void> {
    const res = await coreFetch(ep, "POST", "/v1/pane/opens/done", { done });
    if (!res.ok) throw new Error(`The core answered ${res.status}.`);
  },
};
