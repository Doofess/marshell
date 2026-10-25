import type { AgentCatalog } from "../../generated/AgentCatalog";
import type { Endpoint } from "../../generated/Endpoint";
import { api } from "./client";

export const catalogApi = {
  /** Every agent the launcher can offer (adapters, shipped manifests, the person's own) as this machine has it, and the presets. */
  get: (ep: Endpoint) => api<AgentCatalog>(ep, "GET", "/v1/agents/catalog"),
};
