import type { Endpoint } from "../../generated/Endpoint";
import type { ToolCatalog } from "../../generated/ToolCatalog";
import { api } from "./client";

export const toolsApi = {
  /** The shipped and the person's own tool profiles with their presets, and why own entries were refused. */
  get: (ep: Endpoint) => api<ToolCatalog>(ep, "GET", "/v1/tools"),
};
