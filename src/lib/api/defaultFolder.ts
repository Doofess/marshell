import type { DefaultFolder } from "../../generated/DefaultFolder";
import type { Endpoint } from "../../generated/Endpoint";
import { api } from "./client";

/** The folder sessions and the shell panel start in when nothing names one: the path in force, whether it is there, and whether the person chose it. */
export const getDefaultFolder = (ep: Endpoint): Promise<DefaultFolder> => api<DefaultFolder>(ep, "GET", "/v1/default-folder");
