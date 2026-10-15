import type { Endpoint } from "../../generated/Endpoint";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import { api } from "./client";

export const projects = {
  list: (ep: Endpoint) => api<ProjectInfo[]>(ep, "GET", "/v1/projects"),
};
