import type { Endpoint } from "../../generated/Endpoint";
import type { LayoutDoc } from "../../generated/LayoutDoc";
import type { ViewLayout } from "../../generated/ViewLayout";
import { api } from "./client";

export const layout = {
  get: (ep: Endpoint) => api<LayoutDoc>(ep, "GET", "/v1/layout"),
  /** Saves how the window is laid out. Rejects with a 400 ApiError naming the field when the core refuses a value. */
  putView: (ep: Endpoint, view: ViewLayout) => api<void>(ep, "PUT", "/v1/layout/view", view),
};
