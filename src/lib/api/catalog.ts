import type { AgentCatalog } from "../../generated/AgentCatalog";
import type { Endpoint } from "../../generated/Endpoint";
import type { OwnAgents } from "../../generated/OwnAgents";
import type { OwnAgentsSave } from "../../generated/OwnAgentsSave";
import type { OwnTools } from "../../generated/OwnTools";
import type { OwnToolsSave } from "../../generated/OwnToolsSave";
import { api } from "./client";

export const catalogApi = {
  /** Every agent the launcher can offer (adapters, shipped manifests, the person's own) as this machine has it, and the presets. */
  get: (ep: Endpoint) => api<AgentCatalog>(ep, "GET", "/v1/agents/catalog"),
};

export const ownAgents = {
  /** The agents in `agents.json`, the entries or file the core refused, and the version a save must name. */
  get: (ep: Endpoint) => api<OwnAgents>(ep, "GET", "/v1/agents/own"),
  /** Replaces the list. Throws `ApiError` 409 when the file changed since it was read, 422 with the refused entries as JSON. */
  save: (ep: Endpoint, body: OwnAgentsSave) => api<OwnAgents>(ep, "PUT", "/v1/agents/own", body),
};

export const ownTools = {
  /** The tools in `tools.json`, the entries or file the core refused, and the version a save must name. */
  get: (ep: Endpoint) => api<OwnTools>(ep, "GET", "/v1/tools/own"),
  /** Replaces the list. Throws `ApiError` 409 when the file changed since it was read, 422 with the refused entries as JSON. */
  save: (ep: Endpoint, body: OwnToolsSave) => api<OwnTools>(ep, "PUT", "/v1/tools/own", body),
};
